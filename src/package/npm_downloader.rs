use flate2::read::GzDecoder;
use std::fmt;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tar::Archive;
use warp::hyper::body::Bytes;

use crate::app_error::ServerError;
use crate::npm::package_data::PackageData;
use crate::npm::package_data::PackageDataFetcher;
use crate::utils::request;

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

#[tracing::instrument(name = "download_tarball")]
async fn download_tarball(url: &str) -> Result<(Cursor<Bytes>, TarballType), ServerError> {
    let tarball_type: TarballType = if url.ends_with(".tar") {
        TarballType::Tar
    } else {
        TarballType::TarGzip
    };

    let client = request::get_client(120);
    let response = client.get(url).send().await?;
    let response_status = response.status();
    if !response_status.is_success() {
        return Err(ServerError::TarballDownloadError {
            status_code: response_status.as_u16(),
            url: String::from(url),
        });
    }

    // save the tarball
    return Ok((Cursor::new(response.bytes().await?), tarball_type));
}

#[tracing::instrument(name = "store_tarball", skip(content, data_dir, tarball_type))]
async fn store_tarball(
    content: Cursor<Bytes>,
    tarball_type: TarballType,
    package_name: &str,
    version: &str,
    data_dir: &str,
) -> Result<PathBuf, ServerError> {
    let dir_path = Path::new(data_dir).join(format!("{}-{}", package_name, version));

    // Extract the tarball
    if tarball_type == TarballType::TarGzip {
        let tar = GzDecoder::new(content);
        let mut archive = Archive::new(tar);
        archive.unpack(dir_path.clone())?;
    } else {
        let mut archive = Archive::new(content);
        archive.unpack(dir_path.clone())?;
    }

    // Return target folder
    Ok(dir_path.clone().join("package"))
}

#[tracing::instrument(name = "download_package_content", skip(data_dir, data_fetcher))]
pub async fn download_package_content(
    package_name: &str,
    version: &str,
    data_dir: &str,
    data_fetcher: &PackageDataFetcher,
) -> Result<PathBuf, ServerError> {
    let manifest: Arc<PackageData> = data_fetcher.get(package_name).await?;
    if let Some(version_data) = manifest.versions.get(version) {
        let (content, tarball_type) = download_tarball(version_data.tarball.as_str()).await?;
        store_tarball(content, tarball_type, package_name, version, data_dir).await
    } else {
        Err(ServerError::PackageVersionNotFound)
    }
}
