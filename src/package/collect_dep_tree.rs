use semver::VersionReq;
use serde::{self, Deserialize, Serialize};
use std::{collections::HashMap, sync::MutexGuard};

use crate::{app_error::ServerError, cache::layered::LayeredCache};

use super::npm_package_manifest::download_package_manifest_cached;

pub struct DependencyRequest {
    name: String,
    version_range: VersionReq,
}

impl DependencyRequest {
    pub fn new(name: &str, version_range_str: &str) -> Result<Self, ServerError> {
        // TODO: Handle aliases, "react": "npm:preact@^7.0.0"
        let version_range = VersionReq::parse(version_range_str)?;
        Ok(DependencyRequest {
            name: String::from(name),
            version_range,
        })
    }
}

// TODO: Add a flag to indicate whether this dependency is used in the code or not
#[derive(Serialize, Deserialize)]
pub struct Dependency {
    #[serde(rename = "v")]
    version: String,
    #[serde(rename = "d")]
    depth: u32,
}

#[derive(Serialize, Deserialize)]
pub struct DependencyTree {
    #[serde(rename = "m")]
    modules: HashMap<String, String>,
    #[serde(rename = "d")]
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

pub fn process_dep_map(
    dep_map: HashMap<String, String>,
) -> Result<Vec<DependencyRequest>, ServerError> {
    let mut deps: Vec<DependencyRequest> = Vec::new();
    for (key, val) in dep_map.iter() {
        match DependencyRequest::new(key.as_str(), val.as_str()) {
            Ok(dep) => {
                deps.push(dep);
            }
            Err(err) => {
                println!(
                    "Failed to parse dep range {} for {}. {:?}",
                    val.as_str(),
                    key.as_str(),
                    err
                )
            }
        }
    }
    Ok(deps)
}

async fn resolve_dep(
    tree: &mut DependencyTree,
    req: DependencyRequest,
    depth: u32,
    cache: &mut MutexGuard<'_, LayeredCache>,
) -> Result<(), ServerError> {
    let manifest = download_package_manifest_cached(req.name.as_str(), cache).await?;
    Ok(())
}

pub async fn collect_dep_tree(
    deps: Vec<DependencyRequest>,
    data_dir: &str,
    cache: &mut MutexGuard<'_, LayeredCache>,
) -> Result<DependencyTree, ServerError> {
    let mut tree = DependencyTree::new();
    for dep_req in deps {
        resolve_dep(&mut tree, dep_req, 0, cache).await?;
    }
    Ok(tree)
}
