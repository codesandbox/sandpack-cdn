use node_semver::{Range, Version};
use serde::{self, Deserialize, Serialize};
use std::collections::HashMap;

use crate::{app_error::ServerError, cache::layered::LayeredCache};
use futures::future::join_all;

use super::{
    npm_package_manifest::{download_package_manifest_cached, CachedPackageManifest},
    process::module_dependencies_cached,
};

#[derive(Clone, Debug)]
pub enum VersionRange {
    Range(Range),
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
        let version_range = match Range::parse(version_range_str) {
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
            VersionRange::Range(range) => {
                let mut versions: Vec<&String> = manifest.versions.keys().collect();
                versions.sort_by(|a, b| b.cmp(a));
                for version in versions {
                    let parsed_version = Version::parse(version.as_str());
                    if let Ok(v) = parsed_version {
                        if v.satisfies(&range) {
                            return Some(v.to_string());
                        }
                    }
                }
                None
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Dependency {
    #[serde(rename = "n")]
    name: String,
    #[serde(rename = "v")]
    version: String,
    #[serde(rename = "d")]
    depth: u32,
}

impl Dependency {
    pub fn new(name: String, version: String, depth: u32) -> Self {
        Dependency {
            name,
            version,
            depth,
        }
    }
}

pub type DependencyList = Vec<Dependency>;

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
    cache: &LayeredCache,
) -> Result<Option<(DependencyRequest, String, Vec<DependencyRequest>)>, ServerError> {
    let manifest = download_package_manifest_cached(req.name.as_str(), cache).await?;
    if let Some(resolved_version) = req.resolve_version(&manifest) {
        let dependencies = module_dependencies_cached(
            req.name.as_str(),
            resolved_version.as_str(),
            data_dir,
            cache,
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
        return Ok(Some((req, resolved_version, transient_deps)));
    }
    Ok(None)
}

pub async fn collect_dep_tree(
    deps: Vec<DependencyRequest>,
    data_dir_slice: &str,
    cache_ref: &LayeredCache,
) -> Result<DependencyList, ServerError> {
    let mut dependencies: DependencyList = Vec::new();
    let mut resolve_dep_futures = Vec::new();
    let mut dep_requests: Vec<DependencyRequest> = Vec::from(deps);
    while !dep_requests.is_empty() {
        for dep_req in dep_requests {
            let data_dir = String::from(data_dir_slice);
            let cache = cache_ref.clone();

            // TODO: Only skip if version range also matches, also find a better way to de-duplicate, probably when they get added...
            let future =
                tokio::spawn(async move { resolve_dep(dep_req, data_dir.as_str(), &cache).await });
            resolve_dep_futures.push(future);
        }

        let new_dep_requests = join_all(resolve_dep_futures).await;

        dep_requests = Vec::new();
        resolve_dep_futures = Vec::new();

        for new_dep_request_res in new_dep_requests.into_iter().flatten().flatten().flatten() {
            let (original_dep_req, resolved_version, transient_deps) = new_dep_request_res;

            if !dependencies
                .iter()
                .any(|d| d.name.eq(&original_dep_req.name))
            {
                dependencies.push(Dependency::new(
                    original_dep_req.name,
                    resolved_version.clone(),
                    original_dep_req.depth,
                ));
            }

            for transient_dep_req in transient_deps {
                if dependencies
                    .iter()
                    .any(|d| d.name.eq(&transient_dep_req.name))
                {
                    continue;
                }

                dep_requests.push(transient_dep_req);
            }
        }
    }

    Ok(dependencies)
}
