use flate2::read::GzDecoder;
use std::collections::HashMap;
use std::fmt;
use std::io::Cursor;
use std::path::Path;
use tar::Archive;
use url::Url;

use crate::app_error::ServerError;
use serde::{self, Deserialize, Serialize};

#[derive(PartialEq, Eq)]
pub enum TarballType {
    Tar,
    TarGzip,
}

impl std::fmt::Display for TarballType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TarballType::Tar => write!(f, "tar"),
            TarballType::TarGzip => write!(f, "tgz"),
        }
    }
}

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

// TODO: Cache the manifest on redis
pub async fn download_package_manifest(
    package_name: String,
) -> Result<PackageManifest, ServerError> {
    let manifest: PackageManifest =
        reqwest::get(format!("https://registry.npmjs.org/{}", package_name))
            .await?
            .json()
            .await?;

    Ok(manifest)
}

pub async fn download_package_content(
    package_name: String,
    version: String,
    data_dir: String,
) -> Result<(), ServerError> {
    let manifest: PackageManifest = download_package_manifest(package_name.clone()).await?;
    if let Some(package_data) = manifest.versions.get(version.as_str()) {
        // process the tarball url
        let tarball_url_str: String = package_data.dist.tarball.clone();
        let parsed_tarball_url: Url = Url::parse(tarball_url_str.as_str())?;
        let tarball_url_path = String::from(parsed_tarball_url.path());
        let tarball_type: TarballType = if tarball_url_path.as_str().ends_with(".tar") {
            TarballType::Tar
        } else {
            TarballType::TarGzip
        };

        // download the tarball
        let response = reqwest::get(tarball_url_str.as_str()).await?;
        let response_status = response.status();
        if !response_status.is_success() {
            return Err(ServerError::RequestErrorStatus(response_status.as_u16()));
        }

        // save the tarball
        let dir_path = Path::new(data_dir.as_str()).join(format!(
            "{}-{}",
            package_name.as_str(),
            version.as_str()
        ));
        let content = Cursor::new(response.bytes().await?);

        // Extract the tarball
        if tarball_type == TarballType::TarGzip {
            let tar = GzDecoder::new(content);
            let mut archive = Archive::new(tar);
            archive.unpack(dir_path)?;
        } else {
            let mut archive = Archive::new(content);
            archive.unpack(dir_path)?;
        }
    } else {
        return Err(ServerError::PackageVersionNotFound);
    }

    Ok(())
}
