use std::io::{Cursor, Read};
use std::{fmt, sync::Arc, time::Duration};

use crate::{app_error::ServerError, cached::Cached, npm_replicator::fs_db::FSNpmDatabase};
use ::tar::{Archive, EntryType};
use flate2::read::GzDecoder;
use moka::future::Cache;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::collections::HashMap;

pub type ByteVec = Vec<u8>;
pub type FileMap = Arc<HashMap<String, ByteVec>>;

#[tracing::instrument(name = "accumulate_files", skip(archive))]
fn accumulate_files<R: Read>(
    mut archive: Archive<R>,
) -> Result<HashMap<String, ByteVec>, ServerError> {
    let mut collected: HashMap<String, ByteVec> = HashMap::new();
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
        let first_slash_position = filepath_str.chars().position(|c| c == '/').unwrap_or(0);
        let filepath = String::from(&filepath_str[first_slash_position..]);

        // Insert into collection
        collected.insert(filepath, buf);
    }
    Ok(collected)
}

#[tracing::instrument(name = "download_tarball", skip(client))]
async fn download_tarball(
    client: &ClientWithMiddleware,
    url: &str,
) -> Result<FileMap, ServerError> {
    let response = client.get(url).send().await?;
    let response_status = response.status();
    if !response_status.is_success() {
        return Err(ServerError::TarballDownloadError {
            status_code: response_status.as_u16(),
            url: String::from(url),
        });
    }

    let content = Cursor::new(response.bytes().await?);
    let files = if url.ends_with(".tar") {
        let archive = Archive::new(content);
        accumulate_files(archive)
    } else {
        let tar = GzDecoder::new(content);
        let archive = Archive::new(tar);
        accumulate_files(archive)
    };
    Ok(Arc::new(files?))
}

#[tracing::instrument(name = "get_tarball", skip(client, cached))]
async fn get_tarball(
    url: &str,
    client: ClientWithMiddleware,
    cached: Cached<FileMap>,
) -> Result<FileMap, ServerError> {
    let url_string = String::from(url);
    let res = cached
        .get_cached(|_last_val| {
            Box::pin(async move {
                let content = download_tarball(&client, url_string.as_str()).await?;
                Ok::<_, ServerError>(content)
            })
        })
        .await?;

    Ok(res)
}

fn get_client() -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

    let client_builder = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(120))
        .deflate(true)
        .gzip(true)
        .brotli(true);
    let base_client = client_builder
        .build()
        .expect("reqwest::ClientBuilder::build()");

    ClientBuilder::new(base_client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

#[derive(Clone)]
pub struct PackageContentFetcher {
    cache: Cache<String, Cached<FileMap>>,
    refresh_interval: Duration,
}

impl PackageContentFetcher {
    pub fn new() -> PackageContentFetcher {
        let ttl = Duration::from_secs(86400);
        let max_capacity = 50;
        PackageContentFetcher {
            cache: Cache::builder()
                .max_capacity(max_capacity)
                .time_to_idle(ttl)
                .build(),
            refresh_interval: Duration::from_secs(604800),
        }
    }

    #[tracing::instrument(name = "pkg_content_get", skip(self))]
    pub async fn get(&self, url: &str) -> Result<FileMap, ServerError> {
        let key = String::from(url);
        let client = get_client();
        if let Some(found_value) = self.cache.get(&key) {
            get_tarball(url, client, found_value).await
        } else {
            let cached: Cached<FileMap> = Cached::new(self.refresh_interval);
            self.cache.insert(key, cached.clone()).await;
            get_tarball(url, client, cached).await
        }
    }
}

impl fmt::Debug for PackageContentFetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PackageContentFetcher")
    }
}

#[tracing::instrument(name = "download_package_content", skip(npm_db, content_fetcher))]
pub async fn download_package_content(
    package_name: &str,
    version: &str,
    npm_db: &FSNpmDatabase,
    content_fetcher: &PackageContentFetcher,
) -> Result<FileMap, ServerError> {
    let manifest = npm_db.get_package(package_name)?;
    if let Some(version_data) = manifest.versions.get(version) {
        let content = content_fetcher.get(version_data.tarball.as_str()).await?;
        Ok(content)
    } else {
        Err(ServerError::PackageVersionNotFound(
            String::from(package_name),
            String::from(version),
        ))
    }
}
