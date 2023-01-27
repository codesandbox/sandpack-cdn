use std::collections::HashSet;

use warp::{Filter, Rejection, Reply};

use crate::app_error::{AppResult, ServerError};
use crate::npm::dep_tree_builder::{DepRange, DepRequest, DepTreeBuilder, ResolutionsMap};
use crate::npm_replicator::database::NpmDatabase;
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
        let parsed_range = DepRange::parse(version);
        dep_requests.insert(DepRequest::new(name, parsed_range));
    }
    Ok(dep_requests)
}

async fn get_reply(
    path: String,
    npm_db: NpmDatabase,
    is_json: bool,
) -> Result<CustomReply, ServerError> {
    let decoded_query = decode_base64(&path)?;
    let dep_requests = parse_query(decoded_query)?;

    let result: AppResult<ResolutionsMap> = tokio::task::spawn_blocking(move || {
        let mut tree_builder = DepTreeBuilder::new(npm_db.clone());
        tree_builder.resolve_tree(dep_requests)?;
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

    let mut reply = match is_json {
        true => CustomReply::json(&result?)?,
        false => CustomReply::msgpack(&result?)?,
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
    npm_db: NpmDatabase,
    is_json: bool,
) -> Result<impl Reply, Rejection> {
    match get_reply(path, npm_db, is_json).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(3600).unwrap()),
    }
}

fn json_route(
    npm_db: NpmDatabase,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "json" / "deps" / String)
        .and(warp::get())
        .and(with_data(npm_db))
        .and(with_data(true))
        .and_then(deps_route_handler)
}

fn msgpack_route(
    npm_db: NpmDatabase,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "deps" / String)
        .and(warp::get())
        .and(with_data(npm_db))
        .and(with_data(false))
        .and_then(deps_route_handler)
}

pub fn deps_route(
    npm_db: NpmDatabase,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    json_route(npm_db.clone()).or(msgpack_route(npm_db))
}
