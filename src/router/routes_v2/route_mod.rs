use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::sync::Arc;

use serde_bytes::ByteBuf;
use ::tar::EntryType;
use warp::hyper::body::Bytes;
use warp::{Filter, Rejection, Reply};

use crate::app_error::ServerError;
use crate::npm::package_content::{download_package_content, PackageContentFetcher};
use crate::npm::package_data::PackageDataFetcher;
use crate::package::process::parse_package_specifier;
use crate::router::utils::decode_base64;
use crate::utils::tar;

use super::super::custom_reply::CustomReply;
use super::super::error_reply::ErrorReply;
use super::super::routes::with_data;

type TarContent = Arc<Cursor<Bytes>>;

fn accumulate_files(tarball_content: TarContent) -> Result<HashMap<String, ByteBuf>, ServerError> {
    let mut collected: HashMap<String, ByteBuf> = HashMap::new();
    let mut archive = tar::open_tarball(tarball_content.as_ref().clone())?;
    for file in archive.entries()? {
        // Make sure there wasn't an I/O error
        let mut file = file?;

        if !EntryType::is_file(&file.header().entry_type()) {
            continue;
        }

        // Read file content
        let mut buf: Vec<u8> = Vec::new();
        file.read_to_end(&mut buf)?;

        // Read file path
        let header_path = file.header().path()?;
        let filepath_str = header_path.to_str().unwrap_or("package/unknown");
        let filepath = String::from(&filepath_str[7..]);

        // Insert into collection
        collected.insert(filepath, ByteBuf::from(buf));
    }

    Ok(collected)
}

#[tracing::instrument(name = "get_files", skip(content))]
async fn get_files(content: TarContent) -> Result<HashMap<String, ByteBuf>, ServerError> {
    let files = tokio::task::spawn_blocking(move || accumulate_files(content)).await??;

    Ok(files)
}

async fn create_reply(content: TarContent) -> Result<CustomReply, ServerError> {
    let files = get_files(content).await?;
    let mut reply = CustomReply::msgpack(&files)?;
    reply.add_header(
        "cache-control",
        format!("public, max-age={}", 365 * 24 * 3600).as_str(),
    );
    Ok(reply)
}

pub async fn get_mod_reply(
    path: String,
    pkg_data_fetcher: PackageDataFetcher,
    pkg_content_fetcher: PackageContentFetcher,
) -> Result<CustomReply, ServerError> {
    let decoded_specifier = decode_base64(&path)?;
    let (pkg_name, pkg_version) = parse_package_specifier(&decoded_specifier)?;

    let content = download_package_content(
        &pkg_name,
        &pkg_version,
        &pkg_data_fetcher,
        &pkg_content_fetcher,
    )
    .await?;

    let response = create_reply(content).await;

    response
}

pub async fn mod_route_handler(
    path: String,
    pkg_data_fetcher: PackageDataFetcher,
    pkg_content_fetcher: PackageContentFetcher,
) -> Result<impl Reply, Rejection> {
    match get_mod_reply(path, pkg_data_fetcher, pkg_content_fetcher).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(3600).unwrap()),
    }
}

pub fn mod_route(
    pkg_data_fetcher: PackageDataFetcher,
    pkg_content_fetcher: PackageContentFetcher,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "mod" / String)
        .and(warp::get())
        .and(with_data(pkg_data_fetcher))
        .and(with_data(pkg_content_fetcher))
        .and_then(mod_route_handler)
}
