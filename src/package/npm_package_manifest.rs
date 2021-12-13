use std::collections::HashMap;

use crate::app_error::ServerError;
use serde::{self, Deserialize, Serialize};

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
    pub dist_tags: HashMap<String, String>,
    pub versions: HashMap<String, String>,
    pub etag: String,
}

impl CachedPackageManifest {
    pub fn from_manifest(manifest: PackageManifest, etag: String) -> CachedPackageManifest {
        let mut versions: HashMap<String, String> = HashMap::new();
        for (key, val) in manifest.versions.iter() {
            versions.insert(key.clone(), val.dist.tarball.clone());
        }
        CachedPackageManifest {
            name: manifest.name,
            dist_tags: manifest.dist_tags,
            versions,
            etag,
        }
    }
}

async fn download_package_manifest(
    package_name: String,
) -> Result<(String, PackageManifest), ServerError> {
    let manifest: PackageManifest =
        reqwest::get(format!("https://registry.npmjs.org/{}", package_name))
            .await?
            .json()
            .await?;

    Ok((String::new(), manifest))
}

// TODO: Cache the manifest on redis
pub async fn download_cached_package_manifest(
    package_name: String,
) -> Result<CachedPackageManifest, ServerError> {
    let (etag, manifest) = download_package_manifest(package_name).await?;
    let cached_manifest = CachedPackageManifest::from_manifest(manifest, etag);
    Ok(cached_manifest)
}
