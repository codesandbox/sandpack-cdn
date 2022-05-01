use node_semver::{Range, Version};
use parking_lot::Mutex;
use serde::{self, Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use tokio::task::JoinHandle;
use tracing::error;

use crate::{app_error::ServerError, cache::layered::LayeredCache};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
                error!(
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

type ResolveDepResult = Result<Option<(Dependency, Vec<DependencyRequest>)>, ServerError>;

#[tracing::instrument(name = "resolve_dep", skip(cache, data_dir))]
async fn resolve_dep(
    req: DependencyRequest,
    data_dir: String,
    cache: &mut LayeredCache,
) -> ResolveDepResult {
    let manifest = download_package_manifest_cached(req.name.as_str(), cache).await?;
    if let Some(resolved_version) = req.resolve_version(&manifest) {
        let dependencies = module_dependencies_cached(
            req.name.as_str(),
            resolved_version.as_str(),
            data_dir.as_str(),
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

        return Ok(Some((
            Dependency::new(req.name, resolved_version, req.depth),
            transient_deps,
        )));
    }
    Ok(None)
}

#[derive(Debug, Clone)]
struct DepTreeCollector {
    data_dir: String,
    cache: LayeredCache,
    dependencies: Arc<Mutex<DependencyList>>,
    futures: Arc<Mutex<VecDeque<JoinHandle<()>>>>,
    in_progress: Arc<Mutex<Vec<DependencyRequest>>>,
}

impl DepTreeCollector {
    pub fn new(data_dir: String, cache: LayeredCache) -> Self {
        DepTreeCollector {
            data_dir,
            cache,
            dependencies: Arc::new(Mutex::new(Vec::new())),
            futures: Arc::new(Mutex::new(VecDeque::new())),
            in_progress: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn get_dependencies(&self) -> DependencyList {
        self.dependencies.lock().clone()
    }

    fn add_dependency(&self, dep: Dependency) {
        if !self
            .dependencies
            .lock()
            .iter()
            .any(|d| d.name.eq(&dep.name))
        {
            self.dependencies.lock().push(dep.clone());
        }
    }

    fn add_future(&self, dep_req: DependencyRequest) {
        let dep_collector = self.clone();
        let future = tokio::spawn(async move {
            let mut cache = dep_collector.cache.clone();
            let data_dir = dep_collector.data_dir.clone();
            let result = resolve_dep(dep_req, data_dir, &mut cache).await;
            if let Ok(Some((dependency, transient_deps))) = result {
                dep_collector.add_dependency(dependency);
                dep_collector.add_dep_requests(transient_deps);
            }
        });
        self.futures.lock().push_back(future);
    }

    fn has_dep_request(&self, dep_request: DependencyRequest) -> bool {
        if self
            .dependencies
            .lock()
            .iter()
            .any(|d| d.name.eq(&dep_request.name))
        {
            return true;
        }

        if self
            .in_progress
            .lock()
            .iter()
            .any(|d| d.name.eq(&dep_request.name))
        {
            return true;
        }

        false
    }

    fn total_dep_count(&self) -> u64 {
        (self.dependencies.lock().len() + self.in_progress.lock().len()) as u64
    }

    fn should_skip_dep_request(&self, dep_request: DependencyRequest) -> bool {
        // Skip packages that start with @types/ as they don't contain any useful code, just typings...
        if dep_request.name.as_str().starts_with("@types/") {
            return true;
        }

        // Add a limit to the total amount of deps
        if self.total_dep_count() > 500 {
            return true;
        }

        self.has_dep_request(dep_request)
    }

    fn add_dep_request(&self, dep_request: DependencyRequest) {
        self.in_progress.lock().push(dep_request);
    }

    fn add_dep_requests(&self, dep_requests: Vec<DependencyRequest>) {
        for dep_req in dep_requests {
            if !self.should_skip_dep_request(dep_req.clone()) {
                self.add_future(dep_req.clone());
                self.add_dep_request(dep_req.clone());
            }
        }
    }

    fn get_next_join(&self) -> Option<JoinHandle<()>> {
        self.futures.lock().pop_front()
    }

    pub async fn try_collect(
        dep_requests: Vec<DependencyRequest>,
        data_dir: String,
        cache: LayeredCache,
    ) -> Result<DependencyList, ServerError> {
        let collector = DepTreeCollector::new(data_dir, cache);
        collector.add_dep_requests(dep_requests);

        while let Some(handle) = collector.get_next_join() {
            if let Err(err) = handle.await {
                error!("Dependency collection error {:?}", err);
            }
        }

        Ok(collector.get_dependencies())
    }
}

pub async fn collect_dep_tree(
    dep_requests: Vec<DependencyRequest>,
    data_dir: &str,
    cache: &LayeredCache,
) -> Result<DependencyList, ServerError> {
    DepTreeCollector::try_collect(dep_requests, String::from(data_dir), cache.clone()).await
}
