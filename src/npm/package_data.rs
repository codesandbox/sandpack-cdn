use std::{collections::HashMap, sync::Arc, time::Duration, fmt};

use crate::{app_error::ServerError, cached::Cached, utils::request};
use moka::future::Cache;
use reqwest::StatusCode;
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use tracing::error;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RawPackageDataVersionDist {
    tarball: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RawPackageDataVersion {
    dist: RawPackageDataVersionDist,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RawPackageData {
    pub name: String,
    #[serde(rename = "dist-tags")]
    dist_tags: HashMap<String, String>,
    versions: HashMap<String, RawPackageDataVersion>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PackageVersionData {
    pub tarball: String,
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PackageData {
    pub name: String,
    pub etag: Option<String>,
    pub dist_tags: HashMap<String, String>,
    pub versions: HashMap<String, PackageVersionData>,
}

impl PackageData {
    pub fn from_raw(raw: RawPackageData, etag: Option<String>) -> PackageData {
        let mut data = PackageData {
            name: raw.name,
            etag,
            dist_tags: raw.dist_tags,
            versions: HashMap::new(),
        };
        for (key, value) in raw.versions {
            data.versions.insert(
                key,
                PackageVersionData {
                    tarball: value.dist.tarball,
                    dependencies: value.dependencies,
                },
            );
        }
        data
    }
}

// TODO: Add etag logic back
#[tracing::instrument(name = "fetch_package_data", skip(client))]
async fn fetch_package_data(
    client: &ClientWithMiddleware,
    package_name: &str,
    previous_etag: Option<String>,
) -> Result<PackageData, ServerError> {
    let mut request = client.get(format!("https://registry.npmjs.org/{}", package_name));
    if let Some(prev_etag_value) = previous_etag {
        request = request.header("If-None-Match", prev_etag_value.as_str());
    }

    let response = request.send().await?;
    if StatusCode::NOT_MODIFIED.eq(&response.status()) {
        return Err(ServerError::NotChanged);
    }

    if !response.status().is_success() {
        return Err(ServerError::NpmManifestDownloadError {
            status_code: response.status().as_u16(),
            package_name: String::from(package_name),
        });
    }

    let mut etag: Option<String> = None;
    if let Some(etag_header_value) = response.headers().get("etag") {
        if let Ok(etag_header_str) = etag_header_value.to_str() {
            etag = Some(String::from(etag_header_str))
        }
    }

    let raw_data: RawPackageData = response.json().await?;
    Ok(PackageData::from_raw(raw_data, etag))
}

#[tracing::instrument(name = "get_package_data", skip(client, cached))]
async fn get_package_data(
    package_name: &str,
    client: Arc<ClientWithMiddleware>,
    cached: Cached<Arc<PackageData>>,
) -> Result<Arc<PackageData>, ServerError> {
    let package_name_string = String::from(package_name);
    let res = cached
        .get_cached(|last_val| {
            Box::pin(async move {
                let etag = last_val.clone().map(|val| val.etag.clone()).unwrap_or(None);
                let pkg_data = {
                    match fetch_package_data(&client, package_name_string.as_str(), etag).await {
                        Ok(res) => Ok(Arc::new(res)),
                        Err(err) => {
                            if let Some(val) = last_val {
                                error!("Fetch failed {:?}", err);
                                Ok(val)
                            } else {
                                Err(err)
                            }
                        }
                    }
                }?;
                Ok::<_, ServerError>(pkg_data)
            })
        })
        .await?;

    Ok(res)
}

#[derive(Clone)]
pub struct PackageDataFetcher {
    client: Arc<ClientWithMiddleware>,
    cache: Cache<String, Cached<Arc<PackageData>>>,
    refresh_interval: Duration,
}

impl PackageDataFetcher {
    pub fn new(refresh_interval: Duration, ttl: Duration, max_capacity: u64) -> PackageDataFetcher {
        PackageDataFetcher {
            client: Arc::new(request::get_client(30)),
            cache: Cache::builder()
                .max_capacity(max_capacity)
                .time_to_idle(ttl)
                .build(),
            refresh_interval,
        }
    }

    #[tracing::instrument(name = "pkg_data_get", skip(self))]
    pub async fn get(&self, name: &str) -> Result<Arc<PackageData>, ServerError> {
        let key = String::from(name);
        if let Some(found_value) = self.cache.get(&key) {
            return get_package_data(name, self.client.clone(), found_value).await;
        } else {
            let cached: Cached<Arc<PackageData>> = Cached::new(self.refresh_interval);
            self.cache.insert(key, cached.clone()).await;
            return get_package_data(name, self.client.clone(), cached).await;
        }
    }
}

impl fmt::Debug for PackageDataFetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PackageDataFetcher")
    }
}
