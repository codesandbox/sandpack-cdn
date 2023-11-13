use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
};

use node_semver::{Range, Version};
use tracing::{error, info};

use crate::{
    app_error::ServerError, npm_replicator::registry::NpmRocksDB,
    package::process::parse_package_specifier_no_validation,
};

#[derive(Clone, Eq, Hash, PartialEq, Debug)]
pub enum DepRange {
    Range(Range),
    Tag(String),
}

impl DepRange {
    pub fn parse(value: String) -> DepRange {
        if value == *"*" || value == *"" {
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

#[derive(Clone, Eq, Hash, PartialEq, Debug)]
pub struct DepRequest {
    name: String,
    range: DepRange,
}

impl DepRequest {
    fn new(name: String, range: DepRange) -> DepRequest {
        DepRequest { name, range }
    }

    pub fn from_name_version(name: String, version: String) -> Result<DepRequest, ServerError> {
        let parsed_range = DepRange::parse(version);
        if let DepRange::Tag(tag) = parsed_range.clone() {
            if tag.contains(':') {
                // Example: npm:@babel/core@7.12.9
                if tag.starts_with("npm:") {
                    let (actual_name, actual_version) =
                        parse_package_specifier_no_validation(&tag[4..])?;
                    let parsed_range = DepRange::parse(actual_version);
                    return Ok(DepRequest::new(actual_name.to_string(), parsed_range));
                }
            }
        }
        Ok(DepRequest::new(name, parsed_range))
    }
}

pub type ResolutionsMap = BTreeMap<String, Version>;
pub type AliasesMap = BTreeMap<String, String>;

pub struct DepTreeBuilder {
    pub resolutions: ResolutionsMap,
    pub aliases: AliasesMap,
    packages: HashMap<String, HashSet<Version>>,
    npm_db: NpmRocksDB,
}

impl DepTreeBuilder {
    pub fn new(npm_db: NpmRocksDB) -> DepTreeBuilder {
        DepTreeBuilder {
            resolutions: BTreeMap::new(),
            aliases: BTreeMap::new(),
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
                    // TODO: Make this only run for dev builds, it slows down requests a lot sometimes...
                    // println!(
                    //     "{}@{} is already resolved, skipping",
                    //     &request.name, &request.range
                    // );

                    return true;
                }
            }
        }
        false
    }

    #[tracing::instrument(name = "resolve_dependency", skip_all, fields(pkg_name = request.name.as_str(), range = request.range.to_string().as_str()))]
    fn resolve_dependency(
        &mut self,
        request: DepRequest,
        mut transient_deps: HashSet<DepRequest>,
    ) -> Result<HashSet<DepRequest>, ServerError> {
        let data = self.npm_db.get_package(&request.name)?;
        let mut range = Range::any();
        if let DepRange::Tag(tag) = &request.range {
            match data.dist_tags.get(tag) {
                Some(found_version) => {
                    range = Range::parse(found_version)?;
                    let version = Version::parse(found_version)?;
                    self.aliases.insert(
                        format!("{}@{}", &request.name, tag),
                        format!("{}@{}", &request.name, &version.major),
                    );
                }
                None => {
                    // If it contains a colon, it's a special specifier and we should just ignore those
                    if tag.contains(':') {
                        return Ok(transient_deps);
                    } else {
                        error!("Invalid package specifier");
                        return Err(ServerError::InvalidPackageSpecifier);
                    }
                }
            }
        } else if let DepRange::Range(original_range) = &request.range {
            range = original_range.clone();
        }

        if self.has_dependency(&request.name, &range) {
            info!("Dependency already exists, skipping");
            return Ok(transient_deps);
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
                    transient_deps
                        .insert(DepRequest::from_name_version(name.clone(), range.clone())?);
                }
            }
            Ok(transient_deps)
        } else {
            error!("Package version not found");
            Err(ServerError::PackageVersionNotFound(
                request.name,
                request.range.to_string(),
            ))
        }
    }

    fn resolve_dependencies(
        &mut self,
        deps: HashSet<DepRequest>,
    ) -> Result<HashSet<DepRequest>, ServerError> {
        let mut transient_deps: HashSet<DepRequest> = HashSet::new();
        for request in deps {
            if let DepRange::Range(original_range) = &request.range {
                if self.has_dependency(&request.name, original_range) {
                    continue;
                }
            }

            transient_deps = self.resolve_dependency(request, transient_deps)?;
        }
        Ok(transient_deps)
    }

    #[tracing::instrument(name = "resolve_dep_tree", skip_all)]
    pub fn resolve_tree(&mut self, deps: HashSet<DepRequest>) -> Result<(), ServerError> {
        let mut deps = deps;
        let mut count = 0;
        while !deps.is_empty() && count < 200 {
            deps = self.resolve_dependencies(deps)?;
            count += 1;
        }

        info!("Finished resolving in {} ticks", count);

        Ok(())
    }
}
