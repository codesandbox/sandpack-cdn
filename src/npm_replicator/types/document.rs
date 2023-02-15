use crate::app_error::AppResult;
use crate::minimal_pkg_capnp;
use capnp::message::Builder;
use capnp::message::ReaderOptions;
use capnp::serialize;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DefaultOnError};
use std::collections::BTreeMap;

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

    pub fn from_buffer(buf: &[u8]) -> AppResult<MinimalPackageData> {
        let reader = serialize::read_message(buf, ReaderOptions::new())?;
        let pkg_reader = reader.get_root::<minimal_pkg_capnp::minimal_package_data::Reader>()?;
        let pkg_name = pkg_reader.get_name()?.to_string();
        let mut dist_tags: BTreeMap<String, String> = BTreeMap::new();
        let dist_tags_iter = pkg_reader.get_dist_tags()?.iter();
        for dist_tag in dist_tags_iter {
            let tag = dist_tag.get_tag()?;
            let version = dist_tag.get_version()?;
            dist_tags.insert(tag.to_string(), version.to_string());
        }
        let versions_iter = pkg_reader.get_versions()?.iter();
        let mut versions: BTreeMap<String, MinimalPackageVersionData> = BTreeMap::new();
        for version_entry in versions_iter {
            let version = version_entry.get_version()?.to_string();
            let tarball = version_entry.get_tarball()?.to_string();
            let mut dependencies: BTreeMap<String, String> = BTreeMap::new();
            let deps_iter = version_entry.get_dependencies()?;
            for dep_entry in deps_iter {
                let dep_name = dep_entry.get_name()?.to_string();
                let dep_version = dep_entry.get_version()?.to_string();
                dependencies.insert(dep_name, dep_version);
            }
            versions.insert(
                version,
                MinimalPackageVersionData {
                    tarball,
                    dependencies,
                },
            );
        }
        Ok(MinimalPackageData {
            name: pkg_name,
            dist_tags,
            versions,
        })
    }

    pub fn to_buffer(&self) -> AppResult<Vec<u8>> {
        let mut message = Builder::new_default();

        let mut pkg_builder =
            message.init_root::<minimal_pkg_capnp::minimal_package_data::Builder>();
        {
            let mut dist_tags_builder = pkg_builder
                .reborrow()
                .init_dist_tags(self.dist_tags.len() as u32);
            for (idx, (tag, version)) in self.dist_tags.iter().enumerate() {
                let mut entry = dist_tags_builder.reborrow().get(idx as u32);
                entry.set_tag(tag);
                entry.set_version(version);
            }
        }

        {
            let mut versions_builder = pkg_builder
                .reborrow()
                .init_versions(self.versions.len() as u32);
            for (idx, (version, version_data)) in self.versions.iter().enumerate() {
                let mut entry = versions_builder.reborrow().get(idx as u32);
                entry.set_version(version);
                entry.set_tarball(&version_data.tarball);
                let mut deps_builder = entry.init_dependencies(version_data.dependencies.len() as u32);
                for (idx, (name, version)) in version_data.dependencies.iter().enumerate() {
                    let mut entry = deps_builder.reborrow().get(idx as u32);
                    entry.set_name(name);
                    entry.set_version(version);
                }
            }
        }

        pkg_builder.set_name(&self.name);

        Ok(serialize::write_message_to_words(&message))
    }
}
