use std::collections::HashMap;

use warp::{Filter, Rejection, Reply};

use crate::app_error::ServerError;
use crate::cache::Cache;
use crate::npm::package_content::PackageContentFetcher;
use crate::npm::package_data::PackageDataFetcher;
use crate::package::collect_dep_tree::{collect_dep_tree, process_dep_map, DependencyList};
use crate::router::routes::with_data;
use crate::AppConfig;

use super::super::custom_reply::CustomReply;
use super::super::error_reply::ErrorReply;
use super::super::utils::decode_req_part;

async fn process_dep_tree(
    raw_deps_str: &str,
    temp_dir: &str,
    cache: &Cache,
    pkg_data_fetcher: &PackageDataFetcher,
    pkg_content_fetcher: &PackageContentFetcher,
) -> Result<DependencyList, ServerError> {
    let decoded_deps_str = decode_req_part(raw_deps_str)?;
    let dep_map: HashMap<String, String> = serde_json::from_str(decoded_deps_str.as_str())?;
    let dep_requests = process_dep_map(dep_map, 0)?;
    return collect_dep_tree(dep_requests, temp_dir, cache, pkg_data_fetcher, pkg_content_fetcher).await;
}

pub async fn get_dep_tree_reply(
    path: String,
    data: AppConfig,
    pkg_data_fetcher: PackageDataFetcher,
    pkg_content_fetcher: PackageContentFetcher,
) -> Result<CustomReply, ServerError> {
    let tree = process_dep_tree(
        path.as_str(),
        data.temp_dir.as_str(),
        &data.cache,
        &pkg_data_fetcher,
        &pkg_content_fetcher,
    )
    .await?;

    let mut reply = CustomReply::json(&tree)?;
    reply.add_header(
        "cache-control",
        format!("public, max-age={}", 15 * 60).as_str(),
    );
    Ok(reply)
}

pub async fn dep_tree_route_handler(
    path: String,
    data: AppConfig,
    pkg_data_fetcher: PackageDataFetcher,
    pkg_content_fetcher: PackageContentFetcher,
) -> Result<impl Reply, Rejection> {
    match get_dep_tree_reply(path, data, pkg_data_fetcher, pkg_content_fetcher).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(15 * 60).unwrap()),
    }
}

pub fn dep_tree_route(
    app_data: AppConfig,
    pkg_data_fetcher: PackageDataFetcher,
    pkg_content_fetcher: PackageContentFetcher,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("dep_tree" / String)
        .and(warp::get())
        .and(with_data(app_data))
        .and(with_data(pkg_data_fetcher))
        .and(with_data(pkg_content_fetcher))
        .and_then(dep_tree_route_handler)
}
