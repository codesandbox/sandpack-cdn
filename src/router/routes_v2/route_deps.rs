use warp::{Filter, Rejection, Reply};

use crate::app_error::ServerError;

use super::super::custom_reply::CustomReply;
use super::super::error_reply::ErrorReply;
use super::super::routes::with_data;
use super::super::utils::decode_req_part;
use crate::npm::package_data::PackageDataFetcher;

pub async fn get_deps_reply(
    path: String,
    pkg_fetcher: PackageDataFetcher,
) -> Result<CustomReply, ServerError> {
    let decoded_specifier = decode_req_part(path.as_str())?;
    let pkg_data = pkg_fetcher.get(&decoded_specifier).await?;
    let reply = CustomReply::msgpack(pkg_data.as_ref())?;
    Ok(reply)
}

pub async fn deps_route_handler(
    path: String,
    pkg_fetcher: PackageDataFetcher,
) -> Result<impl Reply, Rejection> {
    match get_deps_reply(path, pkg_fetcher).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(3600).unwrap()),
    }
}

pub fn deps_route(
    pkg_fetcher: PackageDataFetcher,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "deps" / String)
        .and(warp::get())
        .and(with_data(pkg_fetcher))
        .and_then(deps_route_handler)
}
