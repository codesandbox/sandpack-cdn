use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
};

use node_semver::{Range, Version};

use crate::{app_error::ServerError, npm_replicator::database::NpmDatabase};

#[derive(Eq, Hash, PartialEq, Debug)]
pub enum DepRange {
    Range(Range),
    Tag(String),
}

impl DepRange {
    pub fn parse(value: String) -> DepRange {
        if value == String::from("*") || value == String::from("") {
            DepRange::Range(Range::any())
        } else {
            match Range::parse(&value) {
                Ok(value) => DepRange::Range(value),
                Err(_err) => DepRange::Tag(value),
            }
        }
    }
}

impl fmt::Display for DepRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DepRange::Range(range) => {
                write!(f, "{}", range)
            }
            DepRange::Tag(tag) => {
                write!(f, "{}", tag)
            }
        }
    }
}

#[derive(Eq, Hash, PartialEq)]
pub struct DepRequest {
    name: String,
    range: DepRange,
}

impl DepRequest {
    pub fn new(name: String, range: DepRange) -> DepRequest {
        DepRequest { name, range }
    }
}

pub type ResolutionsMap = BTreeMap<String, Version>;

pub struct DepTreeBuilder {
    pub resolutions: ResolutionsMap,
    packages: HashMap<String, HashSet<Version>>,
    npm_db: NpmDatabase,
}

impl DepTreeBuilder {
    pub fn new(npm_db: NpmDatabase) -> DepTreeBuilder {
        DepTreeBuilder {
            resolutions: BTreeMap::new(),
            packages: HashMap::new(),
            npm_db,
        }
    }

    fn add_dependency(&mut self, name: &str, version: &Version) {
        let mut key = String::from(name);
        key.push('@');
        key.push_str(&version.major.to_string());
        if let Some(value) = self.resolutions.get(&key) {
            // If value is larger than version we continue
            // otherwise we need to add this version to prevent infinite recursion
            // We also make the highest version win
            if value >= version {
                return;
            }
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

    fn process(&mut self, deps: HashSet<DepRequest>) -> Result<HashSet<DepRequest>, ServerError> {
        let mut transient_deps: HashSet<DepRequest> = HashSet::new();

        // Prefetch in background, this ensures the requests below are a bit faster, relying on the data_fetcher cache
        // Without overcomplicating the mostly synchronous logic in this function
        for request in deps {
            let data = self.npm_db.get_package(&request.name)?;
            let mut range = Range::any();
            if let DepRange::Tag(tag) = &request.range {
                match data.dist_tags.get(tag) {
                    Some(found_version) => {
                        range = Range::parse(found_version)?;
                    }
                    None => {
                        return Err(ServerError::InvalidPackageSpecifier);
                    }
                }
            } else if let DepRange::Range(original_range) = &request.range {
                range = original_range.clone();
            }

            if self.has_dependency(&request.name, &range) {
                println!(
                    "{}@{} is already resolved, skipping",
                    &request.name, &request.range
                );
                continue;
            }

            let mut highest_version: Option<Version> = None;
            for (version, _data) in data.versions.iter().rev() {
                let parsed_version = Version::parse(version)?;
                if range.satisfies(&parsed_version) {
                    highest_version = Some(parsed_version);
                    break;
                }
            }

            if let Some(resolved_version) = highest_version {
                self.add_dependency(&request.name, &resolved_version);

                let data = data.versions.get(&resolved_version.to_string());
                if let Some(data) = data {
                    for (name, range) in data.dependencies.iter() {
                        transient_deps.insert(DepRequest::new(
                            name.clone(),
                            DepRange::parse(range.clone()),
                        ));
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

    pub fn resolve_tree(&mut self, deps: HashSet<DepRequest>) -> Result<(), ServerError> {
        let mut deps = deps;
        let mut count = 0;
        while deps.len() > 0 && count < 200 {
            deps = self.process(deps)?;
            count += 1;
        }

        println!("Finished resolving in {} ticks", count);

        Ok(())
    }
}
