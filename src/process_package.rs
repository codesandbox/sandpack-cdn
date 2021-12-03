use crate::app_error::ServerError;
use crate::npm;
use crate::package_json;
use crate::resolver;
use crate::transform_file;

use semver::Version;
use serde::{self, Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use transform_file::transform_file;

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum MinimalFile {
    // This file got used so we transform it or return it
    File {
        // content
        c: String,
        // dependencies
        d: Vec<String>,
        // is transpiled?
        t: bool,
    },
    // We didn't compile or detected this file being used, so we return the size in bytes instead
    Ignored(u64),
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

async fn transform_files(
    specifiers: Vec<String>,
    curr_file: &str,
    result_map: &mut HashMap<String, MinimalFile>,
    files_map: &HashMap<String, u64>,
    pkg_root: PathBuf,
) -> Result<(), ServerError> {
    let curr_dir = resolver::file_path_to_dirname(curr_file);
    let curr_extension = resolver::extract_file_extension(curr_file);
    for specifier in specifiers {
        let abs_specifier =
            resolver::make_mod_specifier_absolute(curr_dir.as_str(), specifier.as_str());
        let found_files =
            resolver::collect_files(abs_specifier.as_str(), files_map, curr_extension);
        for found_file in found_files {
            if !result_map.contains_key(found_file.as_str()) {
                let file_path = pkg_root.join(found_file.as_str());
                if let Ok(content) = fs::read_to_string(file_path) {
                    match transform_file(content.as_str()) {
                        Ok(transformed_file) => {
                            result_map.insert(
                                found_file.clone(),
                                MinimalFile::File {
                                    c: transformed_file.content,
                                    d: transformed_file.dependencies,
                                    t: false,
                                },
                            );
                        }
                        Err(err) => {
                            println!("{:?}", err);

                            result_map.insert(
                                found_file.clone(),
                                MinimalFile::File {
                                    c: content.clone(),
                                    d: vec![],
                                    t: false,
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    return Ok(());
}

pub async fn process_package(
    package_name: String,
    package_version: String,
    data_dir: String,
) -> Result<MinimalCachedModule, ServerError> {
    let parsed_version = Version::parse(package_version.as_str())?;

    let download_start_time = Instant::now();
    let pkg_output_path = npm::download_package_content(
        package_name.clone(),
        parsed_version.to_string(),
        data_dir.to_string(),
    )
    .await?;
    let download_duration_ms = download_start_time.elapsed().as_millis();

    let file_collection_start_time = Instant::now();
    let mut file_paths: HashMap<String, u64> = HashMap::new();
    collect_file_paths(
        pkg_output_path.clone(),
        pkg_output_path.clone(),
        &mut file_paths,
    )?;
    let file_collection_duration_ms = file_collection_start_time.elapsed().as_millis();

    let mut module_files: HashMap<String, MinimalFile> = HashMap::new();
    let mut used_modules: Vec<String> = Vec::new();

    let pkg_json_content = fs::read_to_string(Path::new(&pkg_output_path).join("package.json"))?;
    let parsed_pkg_json = package_json::parse_pkg_json(pkg_json_content.clone())?;

    // add package.json content to the files
    module_files.insert(
        String::from("package.json"),
        MinimalFile::File {
            c: pkg_json_content.clone(),
            d: vec![],
            t: false,
        },
    );

    // transform entries
    let file_collection_start_time = Instant::now();
    transform_files(
        package_json::collect_pkg_entries(parsed_pkg_json),
        ".",
        &mut module_files,
        &file_paths,
        pkg_output_path,
    )
    .await?;
    let transformation_duration_ms = file_collection_start_time.elapsed().as_millis();

    // add remaining files as ignored files
    for (key, value) in &file_paths {
        if !module_files.contains_key(key) {
            module_files.insert(String::from(key), MinimalFile::Ignored(*value));
        }
    }

    let module_spec = MinimalCachedModule {
        f: module_files,
        m: used_modules,
    };

    println!(
        "\nMetrics for {}@{}\nDownload: {}ms\nFile Collection: {}ms\nTransformation: {}ms\n",
        package_name,
        package_version,
        download_duration_ms,
        file_collection_duration_ms,
        transformation_duration_ms
    );

    return Ok(module_spec);
}
