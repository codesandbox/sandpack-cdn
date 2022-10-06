use std::{collections::HashMap, sync::Arc, time::Duration};

use crate::{app_error::ServerError, cached::Cached, utils::request};
use moka::future::Cache;
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RawPackageDataVersionDist {
    tarball: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RawPackageDataVersion {
    dist: RawPackageDataVersionDist,
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
) -> Result<PackageData, ServerError> {
    let mut request = client.get(format!("https://registry.npmjs.org/{}", package_name));
    let response = request.send().await?;

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
    cached: Cached<PackageData>,
) -> Result<PackageData, ServerError> {
    let package_name_string = String::from(package_name);
    let res = cached
        .get_cached(|| {
            Box::pin(async move {
                let pkg_data = fetch_package_data(&client, package_name_string.as_str()).await?;
                Ok::<_, ServerError>(pkg_data)
            })
        })
        .await?;

    Ok(res)
}

#[derive(Clone)]
pub struct PackageDataFetcher {
    client: Arc<ClientWithMiddleware>,
    cache: Cache<String, Cached<PackageData>>,
    refresh_interval: Duration,
}

impl PackageDataFetcher {
    pub fn new(refresh_interval: Duration, max_capacity: u64) -> PackageDataFetcher {
        PackageDataFetcher {
            client: Arc::new(request::get_client(30)),
            cache: Cache::new(max_capacity),
            refresh_interval,
        }
    }

    #[tracing::instrument(name = "pkg_data_get", skip(self))]
    pub async fn get(&self, name: &str) -> Result<PackageData, ServerError> {
        let key = String::from(name);
        if let Some(found_value) = self.cache.get(&key) {
            return get_package_data(name, self.client.clone(), found_value).await;
        } else {
            let cached: Cached<PackageData> = Cached::new(self.refresh_interval);
            self.cache.insert(key, cached.clone()).await;
            return get_package_data(name, self.client.clone(), cached).await;
        }
    }
}
