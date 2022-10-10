use std::collections::{HashMap, HashSet};

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
    pub resolutions: HashMap<String, Version>,
    packages: HashMap<String, HashSet<Version>>,
    data_fetcher: PackageDataFetcher,
}

impl DepTreeBuilder {
    pub fn new(data_fetcher: PackageDataFetcher) -> DepTreeBuilder {
        DepTreeBuilder {
            resolutions: HashMap::new(),
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

    async fn process(
        &mut self,
        deps: HashSet<DepRequest>,
    ) -> Result<HashSet<DepRequest>, ServerError> {
        let mut transient_deps: HashSet<DepRequest> = HashSet::new();

        for request in deps {
            if self.has_dependency(&request.name, &request.range) {
                println!(
                    "{}@{} is already resolved, skipping",
                    &request.name, &request.range
                );
                continue;
            }

            // TODO: Add transient deps for fetching...
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
