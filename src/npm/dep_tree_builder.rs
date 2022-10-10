use std::collections::{BTreeMap, HashMap, HashSet};

use node_semver::{Range, Version};

use crate::app_error::ServerError;

use super::package_data::PackageDataFetcher;

#[derive(Eq, Hash, PartialEq)]
pub struct DepRequest {
    name: String,
    range: Range,
}

impl DepRequest {
    pub fn new(name: String, range: Range) -> DepRequest {
        DepRequest { name, range }
    }
}

pub struct DepTreeBuilder {
    pub resolutions: BTreeMap<String, Version>,
    packages: HashMap<String, HashSet<Version>>,
    data_fetcher: PackageDataFetcher,
}

impl DepTreeBuilder {
    pub fn new(data_fetcher: PackageDataFetcher) -> DepTreeBuilder {
        DepTreeBuilder {
            resolutions: BTreeMap::new(),
            packages: HashMap::new(),
            data_fetcher,
        }
    }

    fn add_dependency(&mut self, name: &str, version: &Version) {
        let mut key = String::from(name);
        key.push('@');
        key.push_str(&version.major.to_string());
        if self.resolutions.contains_key(&key) {
            return;
        }
        self.resolutions.insert(key, version.clone());
        if let Some(versions) = self.packages.get_mut(name) {
            versions.insert(version.clone());
        } else {
            let mut versions = HashSet::new();
            versions.insert(version.clone());
            self.packages.insert(String::from(name), versions);
        }
    }

    fn has_dependency(&mut self, name: &str, range: &Range) -> bool {
        if let Some(versions) = self.packages.get(&String::from(name)) {
            for version in versions {
                if range.satisfies(version) {
                    return true;
                }
            }
        }
        false
    }

    fn prefetch_module(&self, name: String) {
        let fetcher = self.data_fetcher.clone();
        tokio::spawn(async move {
            match fetcher.get(&name).await {
                Err(err) => {
                    println!("Failed to fetch pkg {:?}", err);
                }
                _ => {}
            };
        });
    }

    async fn process(
        &mut self,
        deps: HashSet<DepRequest>,
    ) -> Result<HashSet<DepRequest>, ServerError> {
        let mut transient_deps: HashSet<DepRequest> = HashSet::new();

        // Prefetch in background, this ensures the requests below are a bit faster, relying on the data_fetcher cache
        // Without overcomplicating the mostly synchronous logic in this function
        let deps_to_fetch: Vec<String> = deps.iter().map(|v| v.name.clone()).collect();
        for pkg_name in deps_to_fetch {
            self.prefetch_module(pkg_name);
        }

        for request in deps {
            if self.has_dependency(&request.name, &request.range) {
                println!(
                    "{}@{} is already resolved, skipping",
                    &request.name, &request.range
                );
                continue;
            }

            let data = self.data_fetcher.get(&request.name).await?;
            let mut highest_version: Option<Version> = None;
            for (version, _data) in data.versions.iter() {
                let parsed_version = Version::parse(version)?;
                if request.range.satisfies(&parsed_version) {
                    highest_version = Some(parsed_version);
                }
            }

            if let Some(resolved_version) = highest_version {
                self.add_dependency(&request.name, &resolved_version);

                let data = data.versions.get(&resolved_version.to_string());
                if let Some(data) = data {
                    for (name, range) in data.dependencies.iter() {
                        self.prefetch_module(name.clone());
                        transient_deps.insert(DepRequest::new(name.clone(), Range::parse(range)?));
                    }
                }
            } else {
                return Err(ServerError::PackageVersionNotFound(
                    request.name,
                    request.range.to_string(),
                ));
            }
        }

        Ok(transient_deps)
    }

    pub async fn push(&mut self, deps: HashSet<DepRequest>) -> Result<(), ServerError> {
        let mut deps = deps;
        while deps.len() > 0 {
            deps = self.process(deps).await?;
        }
        Ok(())
    }
}
