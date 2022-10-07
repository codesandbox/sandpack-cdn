use std::collections::HashMap;
use std::fs::{self, remove_dir_all};
use std::path::{Path, PathBuf};

use nanoid::nanoid;
use serde_bytes::ByteBuf;
use warp::{Filter, Rejection, Reply};

use crate::app_error::ServerError;
use crate::npm::package_content::{download_package_content, PackageContentFetcher};
use crate::npm::package_data::PackageDataFetcher;
use crate::package::process::parse_package_specifier;
use crate::utils::tar;
use crate::AppConfig;

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

async fn create_reply(input_dir: PathBuf) -> Result<CustomReply, ServerError> {
    let files = get_files(input_dir).await?;
    let mut reply = CustomReply::msgpack(&files)?;
    reply.add_header(
        "cache-control",
        format!("public, max-age={}", 365 * 24 * 3600).as_str(),
    );
    Ok(reply)
}

pub async fn get_mod_reply(
    path: String,
    config: AppConfig,
    pkg_data_fetcher: PackageDataFetcher,
    pkg_content_fetcher: PackageContentFetcher,
) -> Result<CustomReply, ServerError> {
    let decoded_specifier = decode_req_part(path.as_str())?;
    let (pkg_name, pkg_version) = parse_package_specifier(&decoded_specifier)?;

    let tarball_content = download_package_content(
        &pkg_name,
        &pkg_version,
        &pkg_data_fetcher,
        &pkg_content_fetcher,
    )
    .await?;
    let mut output_dir = Path::new(&config.temp_dir)
        .join("v2_mod")
        .join(nanoid!())
        .join(format!("{}-{}", pkg_name, pkg_version));

    // TODO: Don't store anything, just loop over the archive contents
    tar::store_tarball(tarball_content.as_ref().clone(), output_dir.as_path()).await?;

    output_dir = output_dir.join("package");

    let response = create_reply(output_dir.clone()).await;

    remove_dir_all(output_dir.as_path())?;

    response
}

pub async fn mod_route_handler(
    path: String,
    data: AppConfig,
    pkg_data_fetcher: PackageDataFetcher,
    pkg_content_fetcher: PackageContentFetcher,
) -> Result<impl Reply, Rejection> {
    match get_mod_reply(path, data, pkg_data_fetcher, pkg_content_fetcher).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(3600).unwrap()),
    }
}

pub fn mod_route(
    app_data: AppConfig,
    pkg_data_fetcher: PackageDataFetcher,
    pkg_content_fetcher: PackageContentFetcher,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "mod" / String)
        .and(warp::get())
        .and(with_data(app_data))
        .and(with_data(pkg_data_fetcher))
        .and(with_data(pkg_content_fetcher))
        .and_then(mod_route_handler)
}
