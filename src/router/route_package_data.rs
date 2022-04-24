use warp::{Filter, Rejection, Reply};

use crate::app_error::ServerError;
use crate::package::process::transform_module_cached;
use crate::utils::decode_req_part;
use crate::AppData;

use super::custom_reply::CustomReply;
use super::error_reply::ErrorReply;
use super::routes::with_app_data;

pub async fn get_package_data_reply(
    path: String,
    data: AppData,
) -> Result<CustomReply, ServerError> {
    let decoded_specifier = decode_req_part(path.as_str())?;
    let package_content = transform_module_cached(
        decoded_specifier.as_str(),
        data.data_dir.as_str(),
        &data.cache,
    )
    .await?;
    let mut reply = CustomReply::json(&package_content)?;
    reply.add_header(
        "cache-control",
        format!("public, max-age={}", 365 * 24 * 3600).as_str(),
    );
    Ok(reply)
}

pub async fn package_data_handler(path: String, data: AppData) -> Result<impl Reply, Rejection> {
    match get_package_data_reply(path, data).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply().unwrap()),
    }
}

pub fn package_data_route(
    app_data: AppData,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("package" / String)
        .and(warp::get())
        .and(with_app_data(app_data))
        .and_then(package_data_handler)
}