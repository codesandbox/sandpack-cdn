use warp::{Filter, Rejection, Reply};

use crate::npm::package_content::PackageContentFetcher;
use crate::npm_replicator::registry::NpmRocksDB;

use super::error_reply::ErrorReply;
use super::health::health_route;
use super::routes_v2::route_deps::deps_route;
use super::routes_v2::route_mod::mod_route;
use super::routes_v2::route_npm_status::npm_sync_status_route;

pub fn routes(
    npm_db: NpmRocksDB,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    // 15 minutes refresh interval and 1 day ttl
    let pkg_content_fetcher = PackageContentFetcher::new();

    mod_route(npm_db.clone(), pkg_content_fetcher)
        .or(deps_route(npm_db.clone()))
        .or(npm_sync_status_route(npm_db))
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
            .as_reply(300)
            .unwrap(),
    )
}

pub fn not_found_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone
{
    warp::any().and_then(not_found_handler)
}
