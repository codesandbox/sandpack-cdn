use crate::app_error::ServerError;
use crate::npm;
use semver::Version;

pub async fn process_package(
    package_name: String,
    package_version: String,
    data_dir: String,
) -> Result<String, ServerError> {
    let parsed_version = Version::parse(package_version.as_str())?;

    npm::download_package_content(
        package_name.clone(),
        parsed_version.to_string(),
        data_dir.to_string(),
    )
    .await?;

    return Ok(format!("Package {}@{}", package_name, parsed_version.to_string()));
}
