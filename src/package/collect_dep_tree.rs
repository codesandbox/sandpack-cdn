use semver::{Version, VersionReq};
use serde::{self, Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use crate::{app_error::ServerError, cache::layered::LayeredCache};

use super::{
    npm_package_manifest::{download_package_manifest_cached, CachedPackageManifest},
    process::module_dependencies_cached,
};

#[derive(Clone, Debug)]
pub enum VersionRange {
    Range(VersionReq),
    Alias(String),
}

#[derive(Clone, Debug)]
pub struct DependencyRequest {
    name: String,
    version_range: VersionRange,
    depth: u32,
}

impl DependencyRequest {
    pub fn new(name: &str, version_range_str: &str, depth: u32) -> Result<Self, ServerError> {
        // TODO: Handle aliases, "react": "npm:preact@^7.0.0"
        let version_range = match VersionReq::parse(version_range_str) {
            Ok(req) => VersionRange::Range(req),
            Err(_) => VersionRange::Alias(String::from(version_range_str)),
        };

        Ok(DependencyRequest {
            name: String::from(name),
            version_range,
            depth,
        })
    }

    pub fn resolve_version(&self, manifest: &CachedPackageManifest) -> Option<String> {
        match self.version_range.clone() {
            VersionRange::Alias(alias_str) => manifest.dist_tags.get(&alias_str).map(|v| v.clone()),
            VersionRange::Range(req) => {
                let mut versions: Vec<&String> = manifest.versions.keys().collect();
                versions.sort_by(|a, b| b.cmp(a));
                for version in versions {
                    let parsed_version = Version::parse(version.as_str());
                    if let Ok(v) = parsed_version {
                        req.matches(&v);
                        return Some(v.to_string());
                    }
                }
                None
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Dependency {
    #[serde(rename = "v")]
    version: String,
    #[serde(rename = "d")]
    depth: u32,
}

impl Dependency {
    pub fn new(version: String, depth: u32) -> Self {
        Dependency { version, depth }
    }
}

pub type DependencyMap = HashMap<String, Dependency>;

pub fn process_dep_map(
    dep_map: HashMap<String, String>,
    depth: u32,
) -> Result<Vec<DependencyRequest>, ServerError> {
    let mut deps: Vec<DependencyRequest> = Vec::with_capacity(dep_map.len());
    for (key, val) in dep_map.iter() {
        match DependencyRequest::new(key.as_str(), val.as_str(), depth) {
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
    req: DependencyRequest,
    data_dir: &str,
    cache: Arc<Arc<Mutex<LayeredCache>>>,
) -> Result<Option<(String, Vec<DependencyRequest>)>, ServerError> {
    let mut cache_claim = cache.lock().unwrap();
    let manifest = download_package_manifest_cached(req.name.as_str(), &mut cache_claim).await?;
    if let Some(resolved_version) = req.resolve_version(&manifest) {
        let dependencies = module_dependencies_cached(
            req.name.as_str(),
            resolved_version.as_str(),
            data_dir,
            &mut cache_claim,
        )
        .await?;
        let mut transient_deps: Vec<DependencyRequest> = Vec::with_capacity(dependencies.len());
        for (dep_name, dep_meta) in dependencies {
            if dep_meta.is_used {
                let dep_req_res = DependencyRequest::new(
                    dep_name.as_str(),
                    dep_meta.version.as_str(),
                    req.depth + 1,
                );
                if let Ok(dep_req) = dep_req_res {
                    transient_deps.push(dep_req);
                }
            }
        }
        return Ok(Some((resolved_version, transient_deps)));
    }
    Ok(None)
}

pub async fn collect_dep_tree(
    deps: Vec<DependencyRequest>,
    data_dir: &str,
    cache: Arc<Arc<Mutex<LayeredCache>>>,
) -> Result<DependencyMap, ServerError> {
    let mut tree: DependencyMap = HashMap::new();
    let mut dep_queue: VecDeque<DependencyRequest> = VecDeque::from(deps);
    while dep_queue.len() > 0 {
        let item = dep_queue.pop_front();
        match item {
            Some(dep_req) => {
                if let Some((resolved_version, transient_deps)) =
                    resolve_dep(dep_req.clone(), data_dir, cache.clone()).await?
                {
                    tree.insert(
                        dep_req.name,
                        Dependency::new(resolved_version.clone(), dep_req.depth),
                    );

                    for transient_dep in transient_deps {
                        dep_queue.push_back(transient_dep);
                    }
                }
            }
            None => {
                break;
            }
        }
    }
    Ok(tree)
}
