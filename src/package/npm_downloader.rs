use flate2::read::GzDecoder;
use std::fmt;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::sync::MutexGuard;
use tar::Archive;
use url::Url;

use crate::app_error::ServerError;
use crate::cache::layered::LayeredCache;
use crate::package::npm_package_manifest::{
    download_package_manifest_cached, CachedPackageManifest,
};

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

pub async fn download_package_content(
    package_name: String,
    version: String,
    data_dir: String,
    cache: &mut MutexGuard<'_, LayeredCache>,
) -> Result<PathBuf, ServerError> {
    let manifest: CachedPackageManifest =
        download_package_manifest_cached(package_name.clone(), cache).await?;
    if let Some(tarball_url) = manifest.versions.get(version.as_str()) {
        // process the tarball url
        let parsed_tarball_url: Url = Url::parse(tarball_url.as_str())?;
        let tarball_url_path = String::from(parsed_tarball_url.path());
        let tarball_type: TarballType = if tarball_url_path.as_str().ends_with(".tar") {
            TarballType::Tar
        } else {
            TarballType::TarGzip
        };

        // download the tarball
        let response = reqwest::get(tarball_url.as_str()).await?;
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
            archive.unpack(dir_path.clone())?;
        } else {
            let mut archive = Archive::new(content);
            archive.unpack(dir_path.clone())?;
        }

        return Ok(dir_path.clone().join("package"));
    } else {
        return Err(ServerError::PackageVersionNotFound);
    }
}
