use nanoid::nanoid;
use node_semver::Version;
use serde::{self, Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{error, info, span, Level};
use transform::transformer::transform_file;

use crate::app_error::ServerError;
use crate::npm::package_content::{download_package_content, PackageContentFetcher};
use crate::npm::package_data::PackageDataFetcher;
use crate::transform;
use crate::utils::tar;

use super::package_json::PackageJSON;
use super::{package_json, resolver};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MinimalFile {
    // This file got used so we transform it or return it
    File {
        // content
        #[serde(rename = "c")]
        content: String,
        // dependencies
        #[serde(rename = "d")]
        dependencies: Vec<String>,
        // is transpiled?
        #[serde(rename = "t")]
        is_transpiled: bool,
    },
    // We didn't compile or detected this file being used, so we return the size in bytes instead
    Ignored(u64),
    // Something went wrong with this file
    Failed(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimalCachedModule {
    // name, it's part of the request so leaving it out for now...
    // n: String,
    // version, it's part of the request so leaving it out for now...
    // v: String,
    // files
    #[serde(rename = "f")]
    files: HashMap<String, MinimalFile>,
    // used modules, this is different from dependencies as this only includes a
    // list of node_modules that are used in the code, used for the resolve endpoint
    // to eagerly fetch these modules
    #[serde(rename = "m")]
    modules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDependency {
    #[serde(rename = "v")]
    pub version: String,
    #[serde(rename = "i")]
    pub is_used: bool,
}

pub type ModuleDependenciesMap = HashMap<String, ModuleDependency>;

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

fn deps_to_files_and_modules(deps: &[String]) -> (HashSet<String>, HashSet<String>) {
    let mut used_modules: HashSet<String> = HashSet::new();
    let mut file_specifiers: HashSet<String> = HashSet::new();

    for dep in deps {
        if !dep.starts_with('.') {
            let parts: Vec<&str> = dep.split('/').collect();
            if !parts.is_empty() {
                let mut module_specifier = String::from(parts[0]);
                if module_specifier.starts_with('@') {
                    module_specifier.push('/');
                    module_specifier.push_str(parts[1]);
                }
                used_modules.insert(module_specifier);
            }
        } else {
            file_specifiers.insert(dep.clone());
        }
    }

    (file_specifiers, used_modules)
}

fn transform_files(
    specifiers: Vec<String>,
    curr_file: &str,
    result_map: &mut HashMap<String, MinimalFile>,
    files_map: &HashMap<String, u64>,
    pkg_root: PathBuf,
    used_modules: &mut HashSet<String>,
) {
    let curr_dir = resolver::file_path_to_dirname(curr_file);
    let curr_extension = resolver::extract_file_extension(curr_file);
    for specifier in specifiers {
        let abs_specifier =
            resolver::make_mod_specifier_absolute(curr_dir.as_str(), specifier.as_str());
        let found_files =
            resolver::collect_files(abs_specifier.as_str(), files_map, curr_extension);
        for found_file in found_files {
            if !result_map.contains_key(found_file.as_str()) {
                let file_path = pkg_root.clone().join(found_file.as_str());
                match fs::read_to_string(file_path) {
                    Ok(content) => match transform_file(found_file.as_str(), content.as_str()) {
                        Ok(transformed_file) => {
                            let deps: Vec<String> =
                                transformed_file.dependencies.into_iter().collect();
                            let (file_deps, module_deps) = deps_to_files_and_modules(&deps);

                            for module_dep in module_deps {
                                used_modules.insert(module_dep);
                            }

                            result_map.insert(
                                found_file.clone(),
                                MinimalFile::File {
                                    content: transformed_file.content,
                                    dependencies: deps.clone(),
                                    is_transpiled: true,
                                },
                            );

                            // Always keep this last, to prevent infinite loops
                            transform_files(
                                file_deps.into_iter().collect(),
                                found_file.as_str(),
                                result_map,
                                files_map,
                                pkg_root.clone(),
                                used_modules,
                            );
                        }
                        Err(err) => {
                            error!("{:?}", err);

                            result_map.insert(
                                found_file.clone(),
                                MinimalFile::File {
                                    content: content.clone(),
                                    dependencies: vec![],
                                    is_transpiled: false,
                                },
                            );
                        }
                    },
                    // TODO: Return an error in this case?
                    Err(err) => {
                        error!("Error reading file: {:?}", err);
                        result_map.insert(found_file.clone(), MinimalFile::Failed(false));
                    }
                }
            }
        }
    }
}

#[tracing::instrument(name = "transform_package", skip(pkg_output_path))]
fn transform_package(
    pkg_output_path: PathBuf,
    package_name: &str,
    package_version: &str,
) -> Result<(MinimalCachedModule, ModuleDependenciesMap), ServerError> {
    let mut file_paths: HashMap<String, u64> = HashMap::new();
    {
        let collect_files_span = span!(
            Level::INFO,
            "pkg_collect_file_paths",
            package_name = package_name,
            package_version = package_version
        )
        .entered();
        collect_file_paths(
            pkg_output_path.clone(),
            pkg_output_path.clone(),
            &mut file_paths,
        )?;
        collect_files_span.exit();
    }

    let mut module_files: HashMap<String, MinimalFile> = HashMap::new();
    let mut used_modules: HashSet<String> = HashSet::new();

    // Read and process pkg.json
    let read_pkg_json = span!(
        Level::INFO,
        "read_pkg_json",
        package_name = package_name,
        package_version = package_version
    )
    .entered();
    let pkg_json_content = fs::read_to_string(Path::new(&pkg_output_path).join("package.json"))?;
    let parsed_pkg_json: PackageJSON = package_json::parse_pkg_json(pkg_json_content.clone())?;

    // add package.json content to the files
    module_files.insert(
        String::from("package.json"),
        MinimalFile::File {
            content: pkg_json_content,
            dependencies: vec![],
            is_transpiled: false,
        },
    );
    read_pkg_json.exit();

    // transform entries
    {
        let transform_files_span = span!(
            Level::INFO,
            "pkg_transform_files",
            package_name = package_name,
            package_version = package_version
        )
        .entered();
        transform_files(
            package_json::collect_pkg_entries(parsed_pkg_json.clone())?,
            ".",
            &mut module_files,
            &file_paths,
            pkg_output_path,
            &mut used_modules,
        );
        transform_files_span.exit();
    }

    // add remaining files as ignored files
    for (key, value) in &file_paths {
        if !module_files.contains_key(key) {
            module_files.insert(String::from(key), MinimalFile::Ignored(*value));
        }
    }

    // collect dependencies
    let mut dependencies: ModuleDependenciesMap = HashMap::new();
    if let Some(deps) = parsed_pkg_json.dependencies {
        for (key, value) in deps.iter() {
            dependencies.insert(
                key.clone(),
                ModuleDependency {
                    version: value.clone(),
                    is_used: used_modules.contains(key),
                },
            );
        }
    }

    let used_modules: Vec<String> = used_modules
        .into_iter()
        .filter(|v| !v.eq(&package_name))
        .collect::<Vec<String>>();
    let module_spec = MinimalCachedModule {
        files: module_files,
        modules: used_modules,
    };

    Ok((module_spec, dependencies))
}

#[tracing::instrument(name = "process_npm_package", skip(temp_dir, data_fetcher))]
pub async fn process_npm_package(
    package_name: &str,
    package_version: &str,
    temp_dir: &str,
    data_fetcher: &PackageDataFetcher,
    content_fetcher: &PackageContentFetcher,
) -> Result<(MinimalCachedModule, ModuleDependenciesMap), ServerError> {
    info!(
        "Started processing package: {}@{}",
        package_name, package_version
    );

    let tarball_content = download_package_content(
        package_name,
        package_version,
        &data_fetcher,
        &content_fetcher,
    )
    .await?;
    let mut pkg_output_path = Path::new(temp_dir)
        .join(nanoid!())
        .join(format!("{}-{}", package_name, package_version));
    tar::store_tarball(tarball_content.as_ref().clone(), pkg_output_path.as_path())?;

    pkg_output_path = pkg_output_path.join("package");

    // Transform module in new thread
    let package_name_string = String::from(package_name);
    let package_version_string = String::from(package_version);
    let cloned_pkg_output_path = pkg_output_path.clone();
    let task = tokio::task::spawn_blocking(move || {
        transform_package(
            cloned_pkg_output_path,
            package_name_string.as_str(),
            package_version_string.as_str(),
        )
    });
    let transform_result = task.await?;

    // Cleanup package directory
    tokio::fs::remove_dir_all(pkg_output_path.as_path()).await?;

    transform_result
}

pub fn parse_package_specifier(package_specifier: &str) -> Result<(String, String), ServerError> {
    if package_specifier.contains(char::is_whitespace) {
        return Err(ServerError::InvalidPackageSpecifier);
    }

    let mut parts: Vec<&str> = package_specifier.split('@').collect();
    let package_version_opt = parts.pop();
    if let Some(package_version) = package_version_opt {
        if parts.len() > 2 {
            return Err(ServerError::InvalidPackageSpecifier);
        }

        let package_name = parts.join("@");
        let parsed_version = Version::parse(package_version)?;

        Ok((package_name, parsed_version.to_string()))
    } else {
        Err(ServerError::InvalidPackageSpecifier)
    }
}
