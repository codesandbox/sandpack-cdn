use std::{fmt, io::Cursor, sync::Arc, time::Duration};

use crate::{app_error::ServerError, cached::Cached, utils::request};
use flate2::{bufread::GzEncoder, Compression};
use moka::future::Cache;
use reqwest_middleware::ClientWithMiddleware;
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

#[tracing::instrument(name = "get_package_data", skip(client, cached))]
async fn get_tarball(
    url: &str,
    client: Arc<ClientWithMiddleware>,
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
    client: Arc<ClientWithMiddleware>,
    cache: Cache<String, Cached<Content>>,
    refresh_interval: Duration,
}

impl PackageContentFetcher {
    pub fn new(
        refresh_interval: Duration,
        ttl: Duration,
        max_capacity: u64,
    ) -> PackageContentFetcher {
        PackageContentFetcher {
            client: Arc::new(request::get_client(120)),
            cache: Cache::builder()
                .max_capacity(max_capacity)
                .time_to_idle(ttl)
                .build(),
            refresh_interval,
        }
    }

    #[tracing::instrument(name = "pkg_content_get", skip(self))]
    pub async fn get(&self, url: &str) -> Result<Content, ServerError> {
        let key = String::from(url);
        if let Some(found_value) = self.cache.get(&key) {
            return get_tarball(url, self.client.clone(), found_value).await;
        } else {
            let cached: Cached<Content> = Cached::new(self.refresh_interval);
            self.cache.insert(key, cached.clone()).await;
            return get_tarball(url, self.client.clone(), cached).await;
        }
    }
}

impl fmt::Debug for PackageContentFetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PackageContentFetcher")
    }
}
