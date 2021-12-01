use crate::app_error::ServerError;
use crate::npm;

use semver::Version;
use serde::{self, Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum MinimalFile {
    // This file got used so we transform it or return it
    UsedFile {
        // content
        c: String,
        // dependencies
        d: Vec<String>,
        // is transpiled?
        t: bool,
    },
    // We didn't compile or detected this file being used, so we return the size in bytes instead
    RawFile(u64),
}

#[derive(Serialize, Deserialize)]
pub struct MinimalCachedModule {
    // name, it's part of the request so leaving it out for now...
    // n: String,
    // version, it's part of the request so leaving it out for now...
    // v: String,
    // files
    f: HashMap<String, MinimalFile>,
    // used modules, this is different from dependencies as this only includes a
    // list of node_modules that are used in the code, used for the resolve endpoint
    // to eagerly fetch these modules
    m: Vec<String>,
}

fn collect_file_paths(
    dir_path: PathBuf,
    root_dir: PathBuf,
    files_map: &mut HashMap<String, u64>,
) -> Result<(), ServerError> {
    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let entry_path = entry.path();

        let metadata = fs::metadata(&entry_path)?;
        if metadata.is_dir() {
            collect_file_paths(entry_path, root_dir.clone(), files_map)?;
        } else if metadata.is_file() {
            files_map.insert(
                String::from(
                    entry_path
                        .strip_prefix(root_dir.clone())
                        .unwrap()
                        .as_os_str()
                        .to_str()
                        .unwrap(),
                ),
                metadata.len(),
            );
        }
    }

    Ok(())
}

pub async fn process_package(
    package_name: String,
    package_version: String,
    data_dir: String,
) -> Result<MinimalCachedModule, ServerError> {
    let parsed_version = Version::parse(package_version.as_str())?;

    let pkg_output_path = npm::download_package_content(
        package_name.clone(),
        parsed_version.to_string(),
        data_dir.to_string(),
    )
    .await?;

    let mut file_paths: HashMap<String, u64> = HashMap::new();
    collect_file_paths(
        pkg_output_path.clone(),
        pkg_output_path.clone(),
        &mut file_paths,
    )?;

    let mut module_files: HashMap<String, MinimalFile> = HashMap::new();
    let mut used_modules: Vec<String> = Vec::new();

    // TODO: Process package.json and add the contents to module_files
    file_paths.remove("package.json");

    for (key, value) in &file_paths {
        module_files.insert(String::from(key), MinimalFile::RawFile(*value));
    }

    let module_spec = MinimalCachedModule {
        f: module_files,
        m: used_modules,
    };

    return Ok(module_spec);
}
