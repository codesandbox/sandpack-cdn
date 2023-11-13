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
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    #[serde(rename = "devDependencies", default)]
    pub dev_dependencies: BTreeMap<String, String>,
    #[serde(rename = "peerDependencies", default)]
    pub peer_dependencies: BTreeMap<String, String>,
}

#[serde_as]
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct PackageMetadata {
    pub name: String,

    #[serde(rename = "dist-tags", default)]
    pub dist_tags: BTreeMap<String, String>,

    #[serde(default)]
    pub versions: BTreeMap<String, PackageVersion>,
}

#[tracing::instrument(name = "download_pkg_metadata")]
pub async fn download_pkg_metadata(
    pkg_name: &str,
) -> Result<PackageMetadata, ServerError> {
    let url: String = format!("https://registry.npmjs.org/{}", pkg_name);
    let client = get_client();
    let response = client
        .get(&url)
        // Return a minimal version of the package metadata
        .header(
            "Accept",
            "application/vnd.npm.install-v1+json; q=1.0, application/json; q=0.8, */*",
        )
        .send()
        .await?;
    let response_status = response.status();
    if !response_status.is_success() {
        return Err(ServerError::PackageMetadataDownloadError {
            status_code: response_status.as_u16(),
            url: String::from(&url),
        });
    }

    let txt = response.text().await?;
    let metadata: PackageMetadata = serde_json::from_str(&txt)?;

    Ok(metadata)
}
