// This file contains hardcoded additional exports for packages that don't provide exports
use lazy_static::lazy_static;
use std::collections::HashMap;

lazy_static! {
    static ref EXPORTS_MAP: HashMap<&'static str, Vec<String>> = {
        let mut m = HashMap::new();
        m.insert(
            "react@17",
            Vec::from([String::from("jsx-runtime"), String::from("jsx-dev-runtime")]),
        );
        m
    };
}

/**
 * Get the additional exports for packages with missing/no exports
 * package_specifier: package@major_version, example: react@17
 */
pub fn get_additional_exports(package_specifier: &str) -> Vec<String> {
    match EXPORTS_MAP.get_key_value(package_specifier) {
        Some((_, val)) => val.clone(),
        None => Vec::new(),
    }
}
