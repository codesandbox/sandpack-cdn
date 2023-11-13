use std::{collections::BTreeMap, time::Duration};

use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::app_error::ServerError;

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

#[serde_as]
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct PackageDist {
    pub tarball: String,
}

#[serde_as]
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct PackageVersion {
    pub dist: PackageDist,
}

#[serde_as]
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct PackageMetadata {
    pub name: String,

    #[serde(rename = "dist-tags")]
    pub dist_tags: Option<BTreeMap<String, String>>,

    pub versions: Option<BTreeMap<String, PackageVersion>>,
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct NormalizedPackageMetadata {
    pub name: String,
    pub dist_tags: Option<BTreeMap<String, String>>,
    pub versions: Option<BTreeMap<String, String>>,
}

#[tracing::instrument(name = "download_pkg_metadata")]
pub async fn download_pkg_metadata(
    pkg_name: &str,
) -> Result<NormalizedPackageMetadata, ServerError> {
    let url: String = format!("https://registry.npmjs.org/{}", pkg_name);
    let client = get_client();
    let response = client.get(&url).send().await?;
    let response_status = response.status();
    if !response_status.is_success() {
        return Err(ServerError::PackageMetadataDownloadError {
            status_code: response_status.as_u16(),
            url: String::from(&url),
        });
    }

    let txt = response.text().await?;
    let metadata: PackageMetadata = serde_json::from_str(&txt)?;

    let mut versions = BTreeMap::new();
    if let Some(raw_versions) = metadata.versions {
        for (version, version_data) in raw_versions {
            versions.insert(version, version_data.dist.tarball);
        }
    }

    Ok(NormalizedPackageMetadata {
        name: metadata.name,
        dist_tags: metadata.dist_tags,
        versions: Some(versions),
    })
}
