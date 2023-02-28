use std::collections::HashMap;

use serde_bytes::ByteBuf;
use warp::{Filter, Rejection, Reply};

use crate::app_error::ServerError;
use crate::npm::package_content::{download_package_content, FileMap, PackageContentFetcher};
use crate::npm_replicator::registry::NpmRocksDB;
use crate::package::process::parse_package_specifier;
use crate::router::utils::decode_base64;

use super::super::custom_reply::CustomReply;
use super::super::error_reply::ErrorReply;
use super::super::routes::with_data;

#[tracing::instrument(name = "get_files", skip(files))]
async fn encode_files(files: FileMap) -> Result<HashMap<String, ByteBuf>, ServerError> {
    let mut encoded_files: HashMap<String, ByteBuf> = HashMap::new();
    for (filepath, content) in files.iter() {
        encoded_files.insert(filepath.clone(), ByteBuf::from(content.clone()));
    }
    Ok(encoded_files)
}

#[tracing::instrument(name = "create_files_reply", skip(files))]
async fn create_reply(files: FileMap) -> Result<CustomReply, ServerError> {
    let files = encode_files(files).await?;
    let mut reply = CustomReply::msgpack(&files)?;
    let cache_ttl = 365 * 24 * 3600;
    reply.add_header(
        "Cache-Control",
        format!("public, max-age={}", cache_ttl).as_str(),
    );
    reply.add_header(
        "CDN-Cache-Control",
        format!("max-age={}", cache_ttl).as_str(),
    );
    Ok(reply)
}

pub async fn get_mod_reply(
    path: String,
    npm_db: NpmRocksDB,
    pkg_content_fetcher: PackageContentFetcher,
) -> Result<CustomReply, ServerError> {
    let decoded_specifier = decode_base64(&path)?;
    let (pkg_name, pkg_version) = parse_package_specifier(&decoded_specifier)?;

    let content =
        download_package_content(&pkg_name, &pkg_version, &npm_db, &pkg_content_fetcher).await?;

    create_reply(content).await
}

pub async fn mod_route_handler(
    path: String,
    npm_db: NpmRocksDB,
    pkg_content_fetcher: PackageContentFetcher,
) -> Result<impl Reply, Rejection> {
    match get_mod_reply(path, npm_db, pkg_content_fetcher).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(300).unwrap()),
    }
}

pub fn mod_route(
    npm_db: NpmRocksDB,
    pkg_content_fetcher: PackageContentFetcher,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "mod" / String)
        .and(warp::get())
        .and(with_data(npm_db))
        .and(with_data(pkg_content_fetcher))
        .and_then(mod_route_handler)
}
