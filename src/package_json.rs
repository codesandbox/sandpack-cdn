use crate::app_error::ServerError;

use serde::{self, Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::env;
use std::fs;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum PackageJSONExport {
    Ignored(Option<bool>),
    Value(String),
    Map(HashMap<String, PackageJSONExport>),
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageJSON {
    name: String,
    version: String,
    // main fields order: 'module', 'browser', 'main', 'jsnext:main'
    main: Option<String>,
    module: Option<String>,
    #[serde(rename = "jsnext:main")]
    js_next_main: Option<String>,
    browser: Option<PackageJSONExport>,
    // exports key order: 'browser', 'development', 'default', 'require', 'import'
    exports: Option<HashMap<String, PackageJSONExport>>,
    dependencies: Option<HashMap<String, String>>,
}

pub fn parse_pkg_json(content: String) -> Result<PackageJSON, ServerError> {
    let pkg_json: PackageJSON = serde_json::from_str(&content)?;
    Ok(pkg_json)
}

// TODO: Write function to collect files to include

fn read_fixture(fixture_name: &str) -> Result<String, ServerError> {
    let fixture_path = env::current_dir()?.join(fixture_name);
    let fixture_content: String = String::from_utf8_lossy(&fs::read(fixture_path)?).parse()?;
    Ok(fixture_content)
}

#[test]
fn pkg_json_parse_test() {
    let content = read_fixture("fixtures/pkg-json/parse-test.json").unwrap();
    let parsed = parse_pkg_json(content.clone()).unwrap();

    assert_eq!(parsed.name, "react");
    assert_eq!(parsed.version, "17.0.2");
    assert_eq!(parsed.js_next_main.unwrap(), "index.next.js");
    assert_eq!(parsed.main.unwrap(), "index.cjs");
    assert_eq!(parsed.module.unwrap(), "index.mjs");
    assert_eq!(
        match parsed.browser.unwrap() {
            PackageJSONExport::Value(v) => {
                v
            }
            _ => {
                panic!("incorrect browser value")
            }
        },
        "index.browser.js"
    );
    assert_eq!(
        match parsed.exports.unwrap().get("something").unwrap() {
            PackageJSONExport::Value(v) => {
                v
            }
            _ => {
                panic!("incorrect something export value")
            }
        },
        "src/something.js"
    );
}
