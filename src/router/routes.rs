use warp::{Filter, Rejection, Reply};

use crate::AppData;

use super::error_reply::ErrorReply;
use super::health::health_route;
use super::routes_v1::route_dep_tree::dep_tree_route;
use super::routes_v1::route_package_data::package_data_route;

pub fn routes(
    app_data: AppData,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    package_data_route(app_data.clone())
        .or(dep_tree_route(app_data))
        .or(health_route())
        .or(not_found_route())
}

pub fn with_app_data(
    app_data: AppData,
) -> impl Filter<Extract = (AppData,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || app_data.clone())
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
