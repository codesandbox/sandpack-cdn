use chrono::{DateTime, Utc};
use std::collections::HashMap;
use reqwest::StatusCode;
use serde::{self, Deserialize, Serialize};
use tracing::{info, error};

use crate::utils::request;
use crate::{app_error::ServerError, cache::layered::LayeredCache};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PackageDist {
    tarball: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MinimalPackageData {
    dist: PackageDist,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PackageManifest {
    name: String,
    #[serde(rename = "dist-tags")]
    dist_tags: HashMap<String, String>,
    versions: HashMap<String, MinimalPackageData>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CachedPackageManifest {
    pub name: String,
    #[serde(rename = "tags")]
    pub dist_tags: HashMap<String, String>,
    pub versions: HashMap<String, String>,
    pub etag: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

impl CachedPackageManifest {
    pub fn from_manifest(manifest: PackageManifest, etag: Option<String>) -> CachedPackageManifest {
        let mut versions: HashMap<String, String> = HashMap::new();
        for (key, val) in manifest.versions.iter() {
            versions.insert(key.clone(), val.dist.tarball.clone());
        }
        CachedPackageManifest {
            name: manifest.name,
            dist_tags: manifest.dist_tags,
            versions,
            etag,
            fetched_at: Utc::now(),
        }
    }
}

#[tracing::instrument("download_package_manifest", skip(cached_etag))]
async fn download_package_manifest(
    package_name: &str,
    cached_etag: Option<String>,
) -> Result<Option<(Option<String>, PackageManifest)>, ServerError> {
    let client = request::get_client(5)?;
    let mut request = client.get(format!("https://registry.npmjs.org/{}", package_name));
    if let Some(cached_etag_val) = cached_etag {
        request = request.header("If-None-Match", cached_etag_val.as_str());
    }
    let response = request.send().await?;

    if StatusCode::NOT_MODIFIED.eq(&response.status()) {
        return Ok(None);
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

    let manifest: PackageManifest = response.json().await?;

    Ok(Some((etag, manifest)))
}

fn get_cache_key(package_name: &str) -> String {
    String::from(format!("v1::manifest::{}", package_name))
}

#[tracing::instrument("download_and_cache_manifest", skip(cache))]
async fn download_and_cache_manifest(
    package_name: &str,
    cache: &LayeredCache,
    cached_etag: Option<String>,
) -> Result<Option<CachedPackageManifest>, ServerError> {
    let cache_key = get_cache_key(package_name);
    let download_manifest_result = download_package_manifest(package_name, cached_etag).await?;
    if let Some((etag, manifest)) = download_manifest_result {
        let cached_manifest = CachedPackageManifest::from_manifest(manifest, etag);
        let serialized = serde_json::to_string(&cached_manifest)?;
        cache
            .store_value(cache_key.as_str(), serialized.as_str())
            .await?;
        return Ok(Some(cached_manifest));
    } else {
        return Ok(None);
    }
}

pub async fn download_package_manifest_cached(
    package_name: &str,
    cache: &LayeredCache,
) -> Result<CachedPackageManifest, ServerError> {
    let cache_key = get_cache_key(package_name);
    let mut originally_cached_manifest: Option<CachedPackageManifest> = None;
    if let Some(cached_value) = cache.get_value(cache_key.as_str()).await {
        let deserialized: serde_json::Result<CachedPackageManifest> =
            serde_json::from_str(cached_value.as_str());
        if let Ok(found_manifest) = deserialized {
            // We return instantly if manifest is less than 15 minutes old
            let time_diff = Utc::now() - found_manifest.fetched_at;
            if time_diff.num_minutes() < 15 {
                return Ok(found_manifest);
            }
            originally_cached_manifest = Some(found_manifest);
        }
    }

    if let Some(cached_manifest) = originally_cached_manifest.clone() {
        // Fetch new manifest in the background and use the old one for this request
        info!(
            "Found npm manifest of {} in cache, fetching new version in background",
            package_name
        );

        let pkg_name_string = String::from(package_name);
        let cloned_cache = cache.clone();
        let etag = cached_manifest.etag.clone();
        tokio::spawn(async move {
            match download_and_cache_manifest(pkg_name_string.as_str(), &cloned_cache, etag).await {
                Ok(val) => {
                    if let Some(_) = val {
                        info!(
                            "Updated manifest for npm module {}",
                            pkg_name_string.as_str()
                        );
                    }
                }
                Err(err) => {
                    error!("Error updating npm manifest cache {:?}", err);
                }
            }
        });

        return Ok(cached_manifest);
    } else {
        if let Some(result) = download_and_cache_manifest(package_name, cache, None).await? {
            return Ok(result);
        } else {
            return Err(ServerError::NpmManifestDownloadError {
                status_code: 404,
                package_name: String::from(package_name),
            });
        }
    }
}
