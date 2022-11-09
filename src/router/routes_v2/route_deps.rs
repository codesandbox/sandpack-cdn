use std::collections::HashSet;

use warp::{Filter, Rejection, Reply};

use crate::app_error::ServerError;
use crate::npm::dep_tree_builder::{DepRange, DepRequest, DepTreeBuilder};
use crate::package::process::parse_package_specifier_no_validation;
use crate::router::utils::decode_base64;

use super::super::custom_reply::CustomReply;
use super::super::error_reply::ErrorReply;
use super::super::routes::with_data;
use crate::npm::package_data::PackageDataFetcher;

fn parse_query(query: String) -> Result<HashSet<DepRequest>, ServerError> {
    let parts = query.split(";");
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
    pkg_fetcher: PackageDataFetcher,
    is_json: bool,
) -> Result<CustomReply, ServerError> {
    let decoded_query = decode_base64(&path)?;
    let dep_requests = parse_query(decoded_query)?;
    let mut tree_builder = DepTreeBuilder::new(pkg_fetcher);
    tree_builder.push(dep_requests).await?;
    let mut reply = match is_json {
        true => CustomReply::json(&tree_builder.resolutions)?,
        false => CustomReply::msgpack(&tree_builder.resolutions)?,
    };
    reply.add_header(
        "cache-control",
        format!("public, max-age={}", 3600).as_str(),
    );
    Ok(reply)
}

async fn deps_route_handler(
    path: String,
    pkg_fetcher: PackageDataFetcher,
    is_json: bool,
) -> Result<impl Reply, Rejection> {
    match get_reply(path, pkg_fetcher, is_json).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(3600).unwrap()),
    }
}

fn json_route(
    pkg_fetcher: PackageDataFetcher,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "json" / "deps" / String)
        .and(warp::get())
        .and(with_data(pkg_fetcher))
        .and(with_data(true))
        .and_then(deps_route_handler)
}

fn msgpack_route(
    pkg_fetcher: PackageDataFetcher,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "deps" / String)
        .and(warp::get())
        .and(with_data(pkg_fetcher))
        .and(with_data(false))
        .and_then(deps_route_handler)
}

pub fn deps_route(
    pkg_fetcher: PackageDataFetcher,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    json_route(pkg_fetcher.clone()).or(msgpack_route(pkg_fetcher))
}
