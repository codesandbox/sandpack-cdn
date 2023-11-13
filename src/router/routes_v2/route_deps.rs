use std::collections::HashSet;

use warp::{Filter, Rejection, Reply};

use crate::app_error::{AppResult, ServerError};
use crate::npm::dep_tree_builder::{DepRequest, DepTreeBuilder, ResolutionsMap};
use crate::npm_replicator::registry::NpmRocksDB;
use crate::package::process::parse_package_specifier_no_validation;
use crate::router::utils::decode_base64;

use super::super::custom_reply::CustomReply;
use super::super::error_reply::ErrorReply;
use super::super::routes::with_data;

fn parse_query(query: String) -> Result<HashSet<DepRequest>, ServerError> {
    let parts = query.split(';');
    let mut dep_requests: HashSet<DepRequest> = HashSet::new();
    for part in parts {
        let (name, version) = parse_package_specifier_no_validation(part)?;
        dep_requests.insert(DepRequest::from_name_version(name, version)?);
    }
    Ok(dep_requests)
}

async fn get_reply(
    path: String,
    npm_db: NpmRocksDB,
    is_json: bool,
) -> Result<CustomReply, ServerError> {
    let decoded_query = decode_base64(&path)?;
    let dep_requests = parse_query(decoded_query)?;

    let mut res_map: Option<ResolutionsMap> = None;
    let mut last_failed_pkg_name: Option<String> = None;
    for _idx in 0..100 {
        let cloned_dep_requests = dep_requests.clone();
        let cloned_npm_db = npm_db.clone();
        let result: AppResult<ResolutionsMap> = tokio::task::spawn_blocking(move || {
            let mut tree_builder = DepTreeBuilder::new(cloned_npm_db);
            tree_builder.resolve_tree(cloned_dep_requests)?;
            for (alias_key, alias_value) in tree_builder.aliases {
                if let Some(resolved_version) = tree_builder.resolutions.get(&alias_value) {
                    tree_builder
                        .resolutions
                        .insert(alias_key, resolved_version.clone());
                }
            }
            Ok(tree_builder.resolutions)
        })
        .await?;

        match result {
            Ok(data) => {
                res_map = Some(data);
            }

            Err(err) => {
                let mut cloned_npm_db = npm_db.clone();
                let new_pkg_name;
                match err {
                    ServerError::PackageVersionNotFound(pkg_name, _) => {
                        new_pkg_name = pkg_name;
                    }
                    ServerError::PackageNotFound(pkg_name) => {
                        new_pkg_name = pkg_name;
                    }
                    err => {
                        return Err(err);
                    }
                }

                if new_pkg_name.len() > 0 {
                    if Some(new_pkg_name.clone()) == last_failed_pkg_name {
                        return Err(ServerError::PackageNotFound(new_pkg_name));
                    }
                    last_failed_pkg_name = Some(new_pkg_name.clone());
                    cloned_npm_db.fetch_missing_pkg(&new_pkg_name).await?;
                }
            }
        }
    }

    if res_map == None {
        return Err(ServerError::PackageNotFound(
            last_failed_pkg_name.unwrap_or("unknown".to_string()),
        ));
    }

    let mut reply = match is_json {
        true => CustomReply::json(&res_map)?,
        false => CustomReply::msgpack(&res_map)?,
    };
    let cache_ttl = 3600;
    reply.add_header(
        "Cache-Control",
        format!("public, max-age={}", cache_ttl).as_str(),
    );
    reply.add_header(
        "CDN-Cache-Control",
        format!("max-age={}", cache_ttl).as_str(),
    );
    Ok(reply)
}

async fn deps_route_handler(
    path: String,
    npm_db: NpmRocksDB,
    is_json: bool,
) -> Result<impl Reply, Rejection> {
    match get_reply(path, npm_db, is_json).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(300).unwrap()),
    }
}

fn json_route(
    npm_db: NpmRocksDB,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "json" / "deps" / String)
        .and(warp::get())
        .and(with_data(npm_db))
        .and(with_data(true))
        .and_then(deps_route_handler)
}

fn msgpack_route(
    npm_db: NpmRocksDB,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "deps" / String)
        .and(warp::get())
        .and(with_data(npm_db))
        .and(with_data(false))
        .and_then(deps_route_handler)
}

pub fn deps_route(
    npm_db: NpmRocksDB,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    json_route(npm_db.clone()).or(msgpack_route(npm_db))
}
