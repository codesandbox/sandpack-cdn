use warp::Filter;

use crate::AppData;

use super::route_dep_tree::dep_tree_route;
use super::route_package_data::package_data_route;

pub fn routes(
    app_data: AppData,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    package_data_route(app_data.clone()).or(dep_tree_route(app_data.clone()))
}

pub fn with_app_data(
    app_data: AppData,
) -> impl Filter<Extract = (AppData,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || app_data.clone())
}
