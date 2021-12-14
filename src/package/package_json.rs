use crate::app_error::ServerError;
use crate::package::additional_exports::get_additional_exports;

use semver::Version;
use serde::{self, Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub enum PackageJSONExport {
    Ignored(Option<bool>),
    Value(String),
    Map(HashMap<String, PackageJSONExport>),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PackageJSON {
    pub name: String,
    pub version: String,
    pub main: Option<String>,
    pub module: Option<String>,
    #[serde(rename = "jsnext:main")]
    pub js_next_main: Option<String>,
    pub browser: Option<PackageJSONExport>,
    pub exports: Option<PackageJSONExport>,
    pub dependencies: Option<HashMap<String, String>>,
}

pub fn parse_pkg_json(content: String) -> Result<PackageJSON, ServerError> {
    let pkg_json: PackageJSON = serde_json::from_str(&content)?;
    Ok(pkg_json)
}

// exports key order: 'browser', 'development', 'default', 'require', 'import'
// Surprisingly good documentation of exports: https://webpack.js.org/guides/package-exports/
pub fn get_export_entry(exports: &PackageJSONExport) -> Option<String> {
    match exports {
        PackageJSONExport::Value(s) => Some(s.clone()),
        PackageJSONExport::Map(nested_exports_value) => {
            for key in ["browser", "development", "default", "require", "import"] {
                let found_value = nested_exports_value.get(key);
                match found_value {
                    Some(v) => {
                        return get_export_entry(v);
                    }
                    _ => {}
                }
            }

            None
        }
        // Fallback to none
        _ => None,
    }
}

// main fields order: 'exports#.', 'module', 'browser', 'main', 'jsnext:main'
fn get_main_entry(pkg_json: &PackageJSON) -> Option<String> {
    if let Some(module_export) = pkg_json.module.clone() {
        return Some(module_export);
    }

    if let Some(browser_export) = pkg_json.browser.clone() {
        match browser_export {
            PackageJSONExport::Value(val) => {
                return Some(val);
            }
            _ => {}
        }
    }

    if let Some(main_export) = pkg_json.main.clone() {
        return Some(main_export);
    }

    if let Some(js_next_main_export) = pkg_json.js_next_main.clone() {
        return Some(js_next_main_export);
    }

    return None;
}

pub fn collect_pkg_entries(pkg_json: PackageJSON) -> Result<Vec<String>, ServerError> {
    let parsed_pkg_version = Version::parse(pkg_json.version.as_str())?;

    let mut entries: Vec<String> = Vec::new();
    let mut has_main_export = false;

    if let Some(exports_field) = pkg_json.exports.clone() {
        match exports_field {
            PackageJSONExport::Map(exports_map) => {
                for (key, value) in exports_map.iter() {
                    // If an export does not start with a dot it is a conditional group, handle it differently.
                    // Whoever invented this really does not respect tooling developers time
                    if !key.starts_with(".") {
                        let new_export_value = PackageJSONExport::Map(exports_map.clone());
                        if let Some(main_export) = get_export_entry(&new_export_value) {
                            has_main_export = true;
                            entries.push(main_export);
                        }
                        break;
                    }

                    // Export starts with a dot, now we have relative exports
                    if let Some(export_val) = get_export_entry(&value) {
                        entries.push(export_val);

                        if key.eq(".") {
                            has_main_export = true;
                        }
                    }
                }
            }
            PackageJSONExport::Value(export_val) => {
                has_main_export = true;
                entries.push(export_val);
            }
            _ => {}
        }
    }

    // This is a fallback to the old module export logic in case a module has no exports#. or exports is not a string
    if !has_main_export {
        if let Some(main_entry) = get_main_entry(&pkg_json) {
            entries.push(main_entry);
        }
    }

    // Add additional, hardcoded exports
    // Used for packages like react that are used a lot but have not properly defined exports yet
    let mut package_key = pkg_json.name.clone();
    package_key.push('@');
    package_key.push_str(parsed_pkg_version.major.to_string().as_str());

    let mut additional_exports = get_additional_exports(package_key.as_str());
    entries.append(&mut additional_exports);

    // Sort and deduplicate...
    entries.sort();
    entries.dedup();

    Ok(entries)
}

#[cfg(test)]
mod test {
    use crate::package::package_json::{parse_pkg_json, PackageJSONExport};
    use crate::utils::test_utils;

    #[test]
    fn pkg_json_parse_test() {
        let content = test_utils::read_fixture("fixtures/pkg-json/parse-test.json").unwrap();
        let parsed = parse_pkg_json(content.clone()).unwrap();

        assert_eq!(parsed.name, "react");
        assert_eq!(parsed.version, "17.0.2");
        assert_eq!(parsed.js_next_main.unwrap(), "index.next.js");
        assert_eq!(parsed.main.unwrap(), "index.cjs");
        assert_eq!(parsed.module.unwrap(), "index.mjs");
        assert_eq!(
            match parsed.browser.unwrap() {
                PackageJSONExport::Value(v) => v,
                _ => panic!("incorrect browser value"),
            },
            "index.browser.js"
        );
        // assert_eq!(
        //     match parsed.exports.unwrap() {
        //         PackageJSONExport::Map(exports_map) => {
        //             match exports_map.get("something").unwrap() {
        //                 PackageJSONExport::Value(v) => v,
        //                 _ => panic!("incorrect something export value"),
        //             }
        //         }
        //         _ => panic!("incorrect export field"),
        //     },
        //     "src/something.js"
        // );
    }
}
