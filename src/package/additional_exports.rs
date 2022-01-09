// This file contains hardcoded additional exports for packages that don't provide exports
use lazy_static::lazy_static;
use std::collections::HashMap;

lazy_static! {
    static ref EXPORTS_MAP: HashMap<&'static str, Vec<String>> = {
        let mut m = HashMap::new();
        m.insert(
            "react",
            Vec::from([
                String::from("jsx-runtime"),
                String::from("jsx-dev-runtime"),
                String::from("unstable-shared-subset"),
            ]),
        );
        m.insert(
            "scheduler",
            Vec::from([
                String::from("tracing"),
                String::from("tracing-profiling"),
                String::from("unstable_mock"),
                String::from("unstable_post_task"),
            ]),
        );
        m
    };
}

/**
 * Get the additional exports for packages with missing/no exports
 * package_name example: react
 */
pub fn get_additional_exports(package_name: &str) -> Vec<String> {
    match EXPORTS_MAP.get_key_value(package_name) {
        Some((_, val)) => val.clone(),
        None => Vec::new(),
    }
}
