use semver::VersionReq;
use serde::{self, Deserialize, Serialize};
use std::{collections::HashMap, sync::MutexGuard};

use crate::{app_error::ServerError, cache::layered::LayeredCache};

pub struct DependencyRequest {
    name: String,
    version_req: VersionReq,
}

#[derive(Serialize, Deserialize)]
pub struct Dependency {
    #[serde(rename = "v")]
    version: String,
    #[serde(rename = "d")]
    depth: u32,
    #[serde(rename = "u")]
    is_used: bool,
}

#[derive(Serialize, Deserialize)]
pub struct DependencyTree {
    #[serde(rename = "m")]
    modules: HashMap<String, String>,
    #[serde(rename = "m")]
    dependencies: HashMap<String, String>,
}

impl DependencyTree {
    pub fn new() -> Self {
        DependencyTree {
            modules: HashMap::new(),
            dependencies: HashMap::new(),
        }
    }
}

pub async fn collect_dep_tree(
    deps: Vec<DependencyRequest>,
    data_dir: String,
    cache: &mut MutexGuard<'_, LayeredCache>,
) -> Result<DependencyTree, ServerError> {
    let mut tree = DependencyTree::new();
    Ok(tree)
}
