use std::{fmt, io::Cursor, sync::Arc, time::Duration};

use crate::{app_error::ServerError, cached::Cached, npm_replicator::database::NpmDatabase};
use flate2::{bufread::GzEncoder, Compression};
use moka::future::Cache;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use warp::hyper::body::Bytes;

pub type Content = Arc<Cursor<Bytes>>;

#[derive(PartialEq, Eq)]
pub enum TarballType {
    Tar,
    TarGzip,
}

#[tracing::instrument(name = "download_tarball")]
async fn download_tarball(
    client: &ClientWithMiddleware,
    url: &str,
) -> Result<Content, ServerError> {
    let tarball_type: TarballType = if url.ends_with(".tar") {
        TarballType::Tar
    } else {
        TarballType::TarGzip
    };

    let response = client.get(url).send().await?;
    let response_status = response.status();
    if !response_status.is_success() {
        return Err(ServerError::TarballDownloadError {
            status_code: response_status.as_u16(),
            url: String::from(url),
        });
    }

    let content = Cursor::new(response.bytes().await?);
    let content = match tarball_type {
        TarballType::Tar => {
            let gzipped = GzEncoder::new(content, Compression::fast());
            gzipped.into_inner()
        }
        TarballType::TarGzip => content,
    };
    Ok(Arc::new(content))
}

#[tracing::instrument(name = "get_tarball", skip(client, cached))]
async fn get_tarball(
    url: &str,
    client: ClientWithMiddleware,
    cached: Cached<Content>,
) -> Result<Content, ServerError> {
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

#[derive(Clone)]
pub struct PackageContentFetcher {
    cache: Cache<String, Cached<Content>>,
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

    pub fn get_client(&self) -> ClientWithMiddleware {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

        let client_builder = reqwest::ClientBuilder::new()
            .timeout(Duration::new(120, 0))
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

    #[tracing::instrument(name = "pkg_content_get", skip(self))]
    pub async fn get(&self, url: &str) -> Result<Content, ServerError> {
        let key = String::from(url);
        let client = self.get_client();
        if let Some(found_value) = self.cache.get(&key) {
            get_tarball(url, client, found_value).await
        } else {
            let cached: Cached<Content> = Cached::new(self.refresh_interval);
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
    npm_db: &NpmDatabase,
    content_fetcher: &PackageContentFetcher,
) -> Result<Content, ServerError> {
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
