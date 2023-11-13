use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DefaultOnError};
use std::collections::BTreeMap;

use crate::{npm::package_data::PackageMetadata, utils::time::secs_since_epoch};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct DocumentPackageDist {
    pub tarball: String,
}

#[serde_as]
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct DocumentPackageVersion {
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub dependencies: Option<BTreeMap<String, String>>,
    #[serde(default, rename = "optionalDependencies")]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub optional_dependencies: Option<BTreeMap<String, String>>,
    pub dist: DocumentPackageDist,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct RegistryDocument {
    #[serde(rename = "_id")]
    pub id: String,

    #[serde(default, rename = "_deleted")]
    pub deleted: bool,

    #[serde(rename = "dist-tags")]
    pub dist_tags: Option<BTreeMap<String, String>>,

    pub versions: Option<BTreeMap<String, DocumentPackageVersion>>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct MinimalPackageVersionData {
    pub tarball: String,
    pub dependencies: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Default)]
pub struct MinimalPackageData {
    pub name: String,
    pub dist_tags: BTreeMap<String, String>,
    pub versions: BTreeMap<String, MinimalPackageVersionData>,
    // Seconds since the epoch
    pub last_updated: Option<u64>,
}

impl MinimalPackageData {
    pub fn from_doc(raw: RegistryDocument) -> MinimalPackageData {
        let mut data = MinimalPackageData {
            name: raw.id,
            dist_tags: raw.dist_tags.unwrap_or_default(),
            versions: BTreeMap::new(),
            last_updated: Some(secs_since_epoch()),
        };
        for (key, value) in raw.versions.unwrap_or_default() {
            let mut dependencies = value.dependencies.unwrap_or_default();
            for (name, _version) in value.optional_dependencies.unwrap_or_default() {
                dependencies.remove(&name);
            }
            data.versions.insert(
                key,
                MinimalPackageVersionData {
                    tarball: value.dist.tarball,
                    dependencies,
                },
            );
        }
        data
    }

    pub fn from_registry_meta(raw: PackageMetadata) -> MinimalPackageData {
        let mut data = MinimalPackageData {
            name: raw.name,
            dist_tags: raw.dist_tags,
            versions: BTreeMap::new(),
            last_updated: Some(secs_since_epoch()),
        };
        for (key, value) in raw.versions {
            data.versions.insert(
                key,
                MinimalPackageVersionData {
                    tarball: value.dist.tarball,
                    dependencies: value.dependencies,
                },
            );
        }
        data
    }
}
