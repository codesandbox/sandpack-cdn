use warp::{Filter, Rejection, Reply};

use crate::cache::layered::LayeredCache;
use crate::custom_reply::CustomReply;
use crate::package::process::transform_module_cached;
use crate::AppData;
use crate::{decode_req_part, process_dep_tree};

pub fn routes(
    app_data: AppData,
    cache: LayeredCache,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    package_route(app_data.clone(), cache.clone())
        .or(dep_tree_route(app_data.clone(), cache.clone()))
}

fn with_app_data(
    app_data: AppData,
) -> impl Filter<Extract = (AppData,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || app_data.clone())
}

fn with_layered_cache(
    layered_cache: LayeredCache,
) -> impl Filter<Extract = (LayeredCache,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || layered_cache.clone())
}


// TODO: Handle failure, with cache of couple hours?
pub async fn package_route_handler(
    path: String,
    data: AppData,
    cache: LayeredCache,
) -> Result<impl Reply, Rejection> {
    let decoded_specifier = decode_req_part(path.as_str())?;
    let package_content =
        transform_module_cached(decoded_specifier.as_str(), data.data_dir.as_str(), &cache).await?;

    let mut reply = CustomReply::json(&package_content);
    reply.add_header(
        "cache-control",
        format!("public, max-age={}", 365 * 24 * 3600).as_str(),
    );
    Ok(reply)
}

pub fn package_route(
    app_data: AppData,
    cache: LayeredCache,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("package" / String)
        .and(warp::get())
        .and(with_app_data(app_data))
        .and(with_layered_cache(cache))
        .and_then(package_route_handler)
}

// TODO: Handle failure, with cache of 5 minutes?
pub async fn dep_tree_route_handler(
    path: String,
    data: AppData,
    cache: LayeredCache,
) -> Result<impl Reply, Rejection> {
    let tree = process_dep_tree(path.as_str(), data.data_dir.as_str(), &cache).await?;

    let mut reply = CustomReply::json(&tree);
    reply.add_header(
        "cache-control",
        format!("public, max-age={}", 15 * 60).as_str(),
    );
    Ok(reply)
}

pub fn dep_tree_route(
    app_data: AppData,
    cache: LayeredCache,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("dep_tree" / String)
        .and(warp::get())
        .and(with_app_data(app_data))
        .and(with_layered_cache(cache))
        .and_then(dep_tree_route_handler)
}
