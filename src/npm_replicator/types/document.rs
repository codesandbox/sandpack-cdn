use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DefaultOnError};
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct DocumentPackageDist {
    pub tarball: String,
}

#[serde_as]
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct DocumentPackageVersion {
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub dependencies: Option<HashMap<String, String>>,
    #[serde(default, rename = "optionalDependencies")]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub optional_dependencies: Option<HashMap<String, String>>,
    pub dist: DocumentPackageDist,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct RegistryDocument {
    #[serde(rename = "_id")]
    pub id: String,

    #[serde(default, rename = "_deleted")]
    pub deleted: bool,

    #[serde(rename = "dist-tags")]
    pub dist_tags: Option<HashMap<String, String>>,

    pub versions: Option<BTreeMap<String, DocumentPackageVersion>>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct MinimalPackageVersionData {
    pub tarball: String,
    pub dependencies: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Default)]
pub struct MinimalPackageData {
    pub name: String,
    pub dist_tags: HashMap<String, String>,
    pub versions: BTreeMap<String, MinimalPackageVersionData>,
}

impl MinimalPackageData {
    pub fn from_doc(raw: RegistryDocument) -> MinimalPackageData {
        let mut data = MinimalPackageData {
            name: raw.id,
            dist_tags: raw.dist_tags.unwrap_or_default(),
            versions: BTreeMap::new(),
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
}
