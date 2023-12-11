use node_semver::Version;

use crate::app_error::ServerError;

// Used for parsing specifiers with ranges
pub fn parse_package_specifier_no_validation(
    package_specifier: &str,
) -> Result<(String, String), ServerError> {
    let mut parts: Vec<&str> = package_specifier.split('@').collect();
    let package_version_opt = parts.pop();
    if let Some(package_version) = package_version_opt {
        if parts.len() > 2 {
            return Err(ServerError::InvalidPackageSpecifier);
        }

        let package_name = parts.join("@");
        Ok((
            String::from(package_name.trim()),
            String::from(package_version.trim()),
        ))
    } else {
        Err(ServerError::InvalidPackageSpecifier)
    }
}

// Used for parsing specifiers that are exact versions, ensures version is valid semver
pub fn parse_package_specifier(package_specifier: &str) -> Result<(String, String), ServerError> {
    let (name, version) = parse_package_specifier_no_validation(package_specifier)?;
    Version::parse(&version)?;
    Ok((name, version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_version() {
        let (pkg_name, pkg_version) = parse_package_specifier("foo@1.2.3").unwrap();
        assert_eq!(pkg_name, "foo");
        assert_eq!(pkg_version, "1.2.3");
    }

    #[test]
    fn simple_version_range_scoped() {
        let (pkg_name, pkg_version) =
            parse_package_specifier_no_validation("@types/react-dom@^1.2.3").unwrap();
        assert_eq!(pkg_name, "@types/react-dom");
        assert_eq!(pkg_version, "^1.2.3");
    }

    #[test]
    fn version_range_whitespace() {
        let (pkg_name, pkg_version) =
            parse_package_specifier_no_validation("@types/dom @ 1 - 2 ").unwrap();
        assert_eq!(pkg_name, "@types/dom");
        assert_eq!(pkg_version, "1 - 2");
    }

    #[test]
    fn larger_than_range() {
        let (pkg_name, pkg_version) =
            parse_package_specifier_no_validation("@code-sandbox_/test@ >=4 ").unwrap();
        assert_eq!(pkg_name, "@code-sandbox_/test");
        assert_eq!(pkg_version, ">=4");
    }
}
