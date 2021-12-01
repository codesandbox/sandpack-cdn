use crate::app_error::ServerError;

use serde::{self, Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum PackageJSONExport {
    Ignored(Option<bool>),
    Value(String),
    Map(HashMap<String, PackageJSONExport>),
    #[serde(skip_serializing)]
    Unknown,
}

#[derive(Serialize, Deserialize)]
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
