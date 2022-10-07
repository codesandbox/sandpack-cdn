use std::time::Duration;
use warp::{Filter, Rejection, Reply};

use crate::npm::package_content::PackageContentFetcher;
use crate::npm::package_data::PackageDataFetcher;
use crate::AppConfig;

use super::error_reply::ErrorReply;
use super::health::health_route;
use super::routes_v1::route_dep_tree::dep_tree_route;
use super::routes_v1::route_package_data::package_data_route;
use super::routes_v2::route_deps::deps_route;
use super::routes_v2::route_mod::mod_route;

pub fn routes(
    app_data: AppConfig,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    // 15 minutes refresh interval and 1 day ttl
    let pkg_data_fetcher =
        PackageDataFetcher::new(Duration::from_secs(900), Duration::from_secs(86400), 250);
    let pkg_content_fetcher =
        PackageContentFetcher::new(Duration::from_secs(604800), Duration::from_secs(86400), 50);

    package_data_route(app_data.clone(), pkg_data_fetcher.clone(), pkg_content_fetcher.clone())
        .or(dep_tree_route(app_data.clone(), pkg_data_fetcher.clone(), pkg_content_fetcher.clone()))
        .or(mod_route(app_data.clone(), pkg_data_fetcher.clone(), pkg_content_fetcher.clone()))
        .or(deps_route(pkg_data_fetcher.clone()))
        .or(health_route())
        .or(not_found_route())
}

pub fn with_data<T>(
    data: T,
) -> impl Filter<Extract = (T,), Error = std::convert::Infallible> + Clone
where
    T: Clone + std::marker::Send,
{
    warp::any().map(move || data.clone())
}

pub async fn not_found_handler() -> Result<impl Reply, Rejection> {
    Ok(
        ErrorReply::new(404, "Not found".to_string(), "Not found".to_string())
            .as_reply(5)
            .unwrap(),
    )
}

pub fn not_found_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone
{
    warp::any().and_then(not_found_handler)
}
