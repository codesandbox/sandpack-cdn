use warp::{Filter, Rejection, Reply};

use crate::app_error::ServerError;
use crate::npm::package_content::PackageContentFetcher;
use crate::npm::package_data::PackageDataFetcher;
use crate::package::process::transform_module_cached;
use crate::AppConfig;

use super::super::custom_reply::CustomReply;
use super::super::error_reply::ErrorReply;
use super::super::routes::with_data;
use super::super::utils::decode_req_part;

pub async fn get_package_data_reply(
    path: String,
    data: AppConfig,
    pkg_data_fetcher: PackageDataFetcher,
    pkg_content_fetcher: PackageContentFetcher,
) -> Result<CustomReply, ServerError> {
    let decoded_specifier = decode_req_part(path.as_str())?;
    let mut cache = data.cache.clone();
    let package_content = transform_module_cached(
        decoded_specifier.as_str(),
        data.temp_dir.as_str(),
        &mut cache,
        &pkg_data_fetcher,
        &pkg_content_fetcher,
    )
    .await?;
    let mut reply = CustomReply::json(&package_content)?;
    reply.add_header(
        "cache-control",
        format!("public, max-age={}", 365 * 24 * 3600).as_str(),
    );
    Ok(reply)
}

pub async fn package_data_handler(
    path: String,
    data: AppConfig,
    pkg_data_fetcher: PackageDataFetcher,
    pkg_content_fetcher: PackageContentFetcher,
) -> Result<impl Reply, Rejection> {
    match get_package_data_reply(path, data, pkg_data_fetcher, pkg_content_fetcher).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(3600).unwrap()),
    }
}

pub fn package_data_route(
    app_data: AppConfig,
    pkg_data_fetcher: PackageDataFetcher,
    pkg_content_fetcher: PackageContentFetcher,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("package" / String)
        .and(warp::get())
        .and(with_data(app_data))
        .and(with_data(pkg_data_fetcher))
        .and(with_data(pkg_content_fetcher))
        .and_then(package_data_handler)
}
