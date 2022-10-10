use std::collections::HashMap;

use warp::{Filter, Rejection, Reply};

use crate::app_error::ServerError;
use crate::npm::package_data::PackageDataFetcher;
use crate::package::cached::CachedPackageProcessor;
use crate::package::collect_dep_tree::{collect_dep_tree, process_dep_map, DependencyList};
use crate::router::routes::with_data;

use super::super::custom_reply::CustomReply;
use super::super::error_reply::ErrorReply;
use super::super::utils::decode_req_part;

async fn process_dep_tree(
    decoded_deps_str: &str,
    pkg_data_fetcher: &PackageDataFetcher,
    pkg_processor: &CachedPackageProcessor,
) -> Result<DependencyList, ServerError> {
    let dep_map: HashMap<String, String> = serde_json::from_str(decoded_deps_str)?;
    let dep_requests = process_dep_map(dep_map, 0)?;
    return collect_dep_tree(dep_requests, pkg_data_fetcher, pkg_processor).await;
}

pub async fn get_dep_tree_reply(
    path: String,
    pkg_data_fetcher: PackageDataFetcher,
    pkg_processor: CachedPackageProcessor,
) -> Result<CustomReply, ServerError> {
    let (version, decoded_deps_str) = decode_req_part(&path)?;

    let tree = process_dep_tree(&decoded_deps_str, &pkg_data_fetcher, &pkg_processor).await?;

    let mut reply = match version {
        0..=4 => CustomReply::json(&tree),
        _ => CustomReply::msgpack(&tree),
    }?;
    reply.add_header(
        "cache-control",
        format!("public, max-age={}", 15 * 60).as_str(),
    );
    Ok(reply)
}

pub async fn dep_tree_route_handler(
    path: String,
    pkg_data_fetcher: PackageDataFetcher,
    pkg_processor: CachedPackageProcessor,
) -> Result<impl Reply, Rejection> {
    match get_dep_tree_reply(path, pkg_data_fetcher, pkg_processor).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(15 * 60).unwrap()),
    }
}

pub fn dep_tree_route(
    pkg_data_fetcher: PackageDataFetcher,
    pkg_processor: CachedPackageProcessor,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("dep_tree" / String)
        .and(warp::get())
        .and(with_data(pkg_data_fetcher))
        .and(with_data(pkg_processor))
        .and_then(dep_tree_route_handler)
}
