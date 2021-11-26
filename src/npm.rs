use std::collections::HashMap;

use crate::app_error::ServerError;
use serde::{self, Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct PackageDist {
    tarball: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MinimalPackageJSON {
    dist: PackageDist
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PackageManifest {
    name: String,
    #[serde(rename = "dist-tags")]
    dist_tags: HashMap<String, String>,
    versions: HashMap<String, MinimalPackageJSON>,
}

// TODO: Cache the manifest on redis
pub async fn download_package_manifest(
    package_name: String,
) -> Result<PackageManifest, ServerError> {
    let manifest: PackageManifest =
        reqwest::get(format!("https://registry.npmjs.org/{}", package_name))
            .await?
            .json()
            .await?;

    println!("{:?}", manifest);

    Ok(manifest)
}
