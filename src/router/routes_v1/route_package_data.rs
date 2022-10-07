use warp::{Filter, Rejection, Reply};

use crate::app_error::ServerError;
use crate::package::cached::CachedPackageProcessor;
use crate::package::process::parse_package_specifier;

use super::super::custom_reply::CustomReply;
use super::super::error_reply::ErrorReply;
use super::super::routes::with_data;
use super::super::utils::decode_req_part;

pub async fn get_package_data_reply(
    path: String,
    pkg_processor: &CachedPackageProcessor,
) -> Result<CustomReply, ServerError> {
    let decoded_specifier = decode_req_part(path.as_str())?;
    let (pkg_name, pkg_version) = parse_package_specifier(&decoded_specifier)?;
    let package_content = pkg_processor.get(&pkg_name, &pkg_version).await?;
    let mut reply = CustomReply::json(&package_content.0)?;
    reply.add_header(
        "cache-control",
        format!("public, max-age={}", 365 * 24 * 3600).as_str(),
    );
    Ok(reply)
}

pub async fn package_data_handler(
    path: String,
    pkg_processor: CachedPackageProcessor,
) -> Result<impl Reply, Rejection> {
    match get_package_data_reply(path, &pkg_processor).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(3600).unwrap()),
    }
}

pub fn package_data_route(
    pkg_processor: CachedPackageProcessor,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("package" / String)
        .and(warp::get())
        .and(with_data(pkg_processor))
        .and_then(package_data_handler)
}
