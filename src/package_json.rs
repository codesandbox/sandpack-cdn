use crate::app_error::ServerError;

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
    name: String,
    version: String,
    main: Option<String>,
    module: Option<String>,
    #[serde(rename = "jsnext:main")]
    js_next_main: Option<String>,
    browser: Option<PackageJSONExport>,
    exports: Option<HashMap<String, PackageJSONExport>>,
    dependencies: Option<HashMap<String, String>>,
}

pub fn parse_pkg_json(content: String) -> Result<PackageJSON, ServerError> {
    let pkg_json: PackageJSON = serde_json::from_str(&content)?;
    Ok(pkg_json)
}

// exports key order: 'browser', 'development', 'default', 'require', 'import'
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
fn get_main_entry(pkg_json: PackageJSON) -> Option<String> {
    if let Some(exports) = pkg_json.exports {
        let root_module = exports.get(".");
        if let Some(root_export) = root_module {
            if let Some(root_export_str) = get_export_entry(root_export) {
                return Some(root_export_str);
            }
        }
    }

    if let Some(module_export) = pkg_json.module {
        return Some(module_export);
    }

    if let Some(browser_export) = pkg_json.browser {
        match browser_export {
            PackageJSONExport::Value(val) => {
                return Some(val);
            }
            _ => {}
        }
    }

    if let Some(main_export) = pkg_json.main {
        return Some(main_export);
    }

    if let Some(js_next_main_export) = pkg_json.js_next_main {
        return Some(js_next_main_export);
    }

    return None;
}

pub fn collect_pkg_entries(pkg_json: PackageJSON) -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();

    if let Some(main_entry) = get_main_entry(pkg_json.clone()) {
        entries.push(main_entry);
    }

    if let Some(exports_map) = pkg_json.exports {
        for (_, value) in exports_map {
            if let Some(export_val) = get_export_entry(&value) {
                entries.push(export_val);
            }
        }
    }

    return entries;
}

#[cfg(test)]
mod test {
    use crate::package_json::{parse_pkg_json, PackageJSONExport};
    use crate::test_utils;

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
        assert_eq!(
            match parsed.exports.unwrap().get("something").unwrap() {
                PackageJSONExport::Value(v) => v,
                _ => panic!("incorrect something export value"),
            },
            "src/something.js"
        );
    }
}
