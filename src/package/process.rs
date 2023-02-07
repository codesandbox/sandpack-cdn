use node_semver::Version;

use crate::app_error::ServerError;

pub fn parse_package_specifier_no_validation(
    package_specifier: &str,
) -> Result<(String, String), ServerError> {
    if package_specifier.contains(char::is_whitespace) {
        return Err(ServerError::InvalidPackageSpecifier);
    }

    let mut parts: Vec<&str> = package_specifier.split('@').collect();
    let package_version_opt = parts.pop();
    if let Some(package_version) = package_version_opt {
        if parts.len() > 2 {
            return Err(ServerError::InvalidPackageSpecifier);
        }

        let package_name = parts.join("@");
        Ok((package_name, String::from(package_version)))
    } else {
        Err(ServerError::InvalidPackageSpecifier)
    }
}

pub fn parse_package_specifier(package_specifier: &str) -> Result<(String, String), ServerError> {
    let (name, version) = parse_package_specifier_no_validation(package_specifier)?;
    Version::parse(&version)?;
    Ok((name, version))
}
