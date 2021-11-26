use crate::app_error::ServerError;
use crate::npm::download_package_manifest;
use semver::Version;

pub async fn process_package(
    package_name: String,
    package_version: String,
) -> Result<String, ServerError> {
    let parsed_version = Version::parse(package_version.as_str())?;

    download_package_manifest(package_name.clone()).await?;

    return Ok(format!("Package {}@{}", package_name, parsed_version.major));
}
