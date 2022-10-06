use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_bytes::ByteBuf;
use warp::{Filter, Rejection, Reply};

use crate::app_error::ServerError;
use crate::npm::package_data::PackageDataFetcher;
use crate::package::npm_downloader;
use crate::package::process::parse_package_specifier;
use crate::AppData;

use super::super::custom_reply::CustomReply;
use super::super::error_reply::ErrorReply;
use super::super::routes::with_data;
use super::super::utils::decode_req_part;

fn accumulate_files(
    dir: &Path,
    curr_dir: String,
    mut collected: HashMap<String, ByteBuf>,
) -> Result<HashMap<String, ByteBuf>, ServerError> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = path.file_name().unwrap().to_str().unwrap();
            let mut entry_name = curr_dir.clone();
            entry_name.push_str("/");
            entry_name.push_str(file_name);

            // Skip native modules
            if entry_name.ends_with(".node") {
                continue;
            }

            if path.is_dir() {
                collected = accumulate_files(&path, entry_name, collected)?;
            } else if path.is_file() {
                let content = fs::read(path)?;
                collected.insert(entry_name, ByteBuf::from(content));
            }
        }
    }

    Ok(collected)
}

#[tracing::instrument(name = "get_files")]
async fn get_files(pkg_output_path: PathBuf) -> Result<HashMap<String, ByteBuf>, ServerError> {
    let files = tokio::task::spawn_blocking(move || {
        let dir_path = pkg_output_path.as_path();
        accumulate_files(dir_path, String::new(), HashMap::new())
    })
    .await??;

    Ok(files)
}

pub async fn get_mod_reply(
    path: String,
    data: AppData,
    pkg_data_fetcher: PackageDataFetcher,
) -> Result<CustomReply, ServerError> {
    let decoded_specifier = decode_req_part(path.as_str())?;
    let (pkg_name, pkg_version) = parse_package_specifier(&decoded_specifier)?;
    let cache = data.cache.clone();
    let pkg_output_path: PathBuf = npm_downloader::download_package_content(
        &pkg_name,
        &pkg_version,
        data.data_dir.as_str(),
        &pkg_data_fetcher,
    )
    .await?;
    let files = get_files(pkg_output_path).await?;
    let mut reply = CustomReply::msgpack(&files)?;
    reply.add_header(
        "cache-control",
        format!("public, max-age={}", 365 * 24 * 3600).as_str(),
    );

    Ok(reply)
}

pub async fn mod_route_handler(path: String, data: AppData, pkg_data_fetcher: PackageDataFetcher) -> Result<impl Reply, Rejection> {
    match get_mod_reply(path, data, pkg_data_fetcher).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(3600).unwrap()),
    }
}

pub fn mod_route(
    app_data: AppData,
    pkg_data_fetcher: PackageDataFetcher,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "mod" / String)
        .and(warp::get())
        .and(with_data(app_data))
        .and(with_data(pkg_data_fetcher))
        .and_then(mod_route_handler)
}
