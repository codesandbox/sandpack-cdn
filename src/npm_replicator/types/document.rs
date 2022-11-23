use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DocumentPackageDist {
    pub tarball: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DocumentPackageVersion {
    pub dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "optionalDependencies")]
    pub optional_dependencies: Option<HashMap<String, String>>,
    pub dist: DocumentPackageDist,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RegistryDocument {
    #[serde(rename = "_id")]
    pub id: String,

    #[serde(default, rename = "_deleted")]
    pub deleted: bool,

    #[serde(rename = "dist-tags")]
    pub dist_tags: Option<HashMap<String, String>>,

    pub versions: Option<BTreeMap<String, DocumentPackageVersion>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MinimalPackageVersionData {
    pub tarball: String,
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MinimalPackageData {
    pub name: String,
    pub dist_tags: HashMap<String, String>,
    pub versions: BTreeMap<String, MinimalPackageVersionData>,
}

impl MinimalPackageData {
    pub fn from_doc(raw: RegistryDocument) -> MinimalPackageData {
        let mut data = MinimalPackageData {
            name: raw.id,
            dist_tags: raw.dist_tags.unwrap_or(HashMap::new()),
            versions: BTreeMap::new(),
        };
        for (key, value) in raw.versions.unwrap_or(BTreeMap::new()) {
            let mut dependencies = value.dependencies.unwrap_or(HashMap::new());
            for (name, _version) in value.optional_dependencies.unwrap_or(HashMap::new()) {
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
