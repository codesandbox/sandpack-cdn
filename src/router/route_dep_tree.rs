use std::collections::HashMap;

use warp::{Filter, Rejection, Reply};

use crate::app_error::ServerError;
use crate::cache::layered::LayeredCache;
use crate::package::collect_dep_tree::{collect_dep_tree, process_dep_map, DependencyList};
use crate::utils::decode_req_part;
use crate::AppData;

use super::custom_reply::CustomReply;
use super::error_reply::ErrorReply;
use super::routes::with_app_data;

async fn process_dep_tree(
    raw_deps_str: &str,
    data_dir: &str,
    cache: &LayeredCache,
) -> Result<DependencyList, ServerError> {
    let decoded_deps_str = decode_req_part(raw_deps_str)?;
    let dep_map: HashMap<String, String> = serde_json::from_str(decoded_deps_str.as_str())?;
    let dep_requests = process_dep_map(dep_map, 0)?;
    return collect_dep_tree(dep_requests, data_dir, cache).await;
}

pub async fn get_dep_tree_reply(path: String, data: AppData) -> Result<CustomReply, ServerError> {
    let tree = process_dep_tree(path.as_str(), data.data_dir.as_str(), &data.cache).await?;

    let mut reply = CustomReply::json(&tree)?;
    reply.add_header(
        "cache-control",
        format!("public, max-age={}", 15 * 60).as_str(),
    );
    Ok(reply)
}

// TODO: Handle failure, with cache of 5 minutes?
pub async fn dep_tree_route_handler(path: String, data: AppData) -> Result<impl Reply, Rejection> {
    match get_dep_tree_reply(path, data).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply().unwrap()),
    }
}

pub fn dep_tree_route(
    app_data: AppData,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("dep_tree" / String)
        .and(warp::get())
        .and(with_app_data(app_data))
        .and_then(dep_tree_route_handler)
}