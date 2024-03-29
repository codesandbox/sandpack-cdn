use std::{num::NonZeroUsize, path::PathBuf, sync::Arc};

use lru::LruCache;
use parking_lot::Mutex;
use rocksdb::DB;

use crate::{
    app_error::{AppResult, ServerError},
    npm::package_data::download_pkg_metadata,
    utils::{msgpack::serialize_msgpack, time::secs_since_epoch},
};

use super::types::document::MinimalPackageData;

#[derive(Clone, Debug)]
pub struct NpmRocksDB {
    pub db_path: PathBuf,
    db: Arc<Mutex<DB>>,
    cache: Arc<Mutex<LruCache<String, Arc<MinimalPackageData>>>>,
}

impl NpmRocksDB {
    pub fn new(db_path: &str) -> Self {
        let db = DB::open_default(db_path).unwrap();
        let cache = LruCache::new(NonZeroUsize::new(500).unwrap());

        Self {
            db_path: PathBuf::from(db_path),
            db: Arc::new(Mutex::new(db)),
            cache: Arc::new(Mutex::new(cache)),
        }
    }

    #[tracing::instrument(name = "npm_db_get_last_seq", level = "debug", skip(self))]
    pub fn get_last_seq(&self) -> AppResult<i64> {
        if let Some(result) = self.db.lock().get(b"#CDN_LAST_SYNC").unwrap() {
            Ok(i64::from_le_bytes(
                result[..]
                    .try_into()
                    .expect("last sync invalid byte length"),
            ))
        } else {
            Ok(0)
        }
    }

    #[tracing::instrument(name = "npm_db_update_last_seq", level = "debug", skip(self))]
    pub fn update_last_seq(&self, next_seq: i64) -> AppResult<usize> {
        self.db
            .lock()
            .put(b"#CDN_LAST_SYNC", next_seq.to_le_bytes())
            .unwrap();
        Ok(1)
    }

    #[tracing::instrument(name = "npm_db_delete_package", level = "debug", skip(self))]
    pub fn delete_package(&self, pkg_name: &str) -> AppResult<usize> {
        self.db.lock().delete(pkg_name.as_bytes()).unwrap();
        Ok(1)
    }

    #[tracing::instrument(name = "npm_db_write_package", level = "debug", skip(self, pkg), fields(pkg_name = pkg.name.as_str()))]
    pub fn write_package(&self, pkg: MinimalPackageData) -> AppResult<usize> {
        if pkg.versions.is_empty() {
            println!("Tried to write pkg {}, but has no versions", pkg.name);
            return self.delete_package(&pkg.name);
        }

        let pkg_name = pkg.name.clone();
        let content = serialize_msgpack(&pkg)?;

        {
            self.db.lock().put(pkg_name.as_bytes(), content).unwrap();
        }

        {
            let mut cache = self.cache.lock();
            cache.pop(&pkg_name);
        }

        Ok(1)
    }

    #[tracing::instrument(name = "npm_db_get_package", level = "debug", skip(self))]
    pub fn get_package(&self, pkg_name: &str) -> AppResult<Arc<MinimalPackageData>> {
        {
            let mut cache = self.cache.lock();
            let cached_value = cache.get(pkg_name);
            if let Some(pkg_data) = cached_value {
                tracing::debug!("NPM Cache hit");
                return Ok(pkg_data.clone());
            }
        };

        let content_val: Option<Vec<u8>> = {
            let span = tracing::span!(tracing::Level::DEBUG, "db_get_pkg").entered();
            let result = self.db.lock().get(pkg_name.as_bytes()).unwrap();
            span.exit();
            result
        };

        if let Some(pkg_content) = content_val {
            let span = tracing::span!(tracing::Level::DEBUG, "parse_pkg").entered();
            let found_pkg: MinimalPackageData = rmp_serde::from_slice(&pkg_content)?;
            span.exit();

            let span = tracing::span!(tracing::Level::DEBUG, "write_cached_pkg").entered();
            let mut cache = self.cache.lock();
            let wrapped_pkg = Arc::new(found_pkg);
            cache.put(pkg_name.to_string(), wrapped_pkg.clone());
            span.exit();

            Ok(wrapped_pkg)
        } else {
            Err(crate::app_error::ServerError::PackageNotFound(
                pkg_name.to_string(),
            ))
        }
    }

    pub async fn fetch_missing_pkg(&mut self, pkg_name: &str) -> Result<(), ServerError> {
        let mut should_fetch = false;
        match self.get_package(pkg_name) {
            Ok(pkg) => {
                if pkg.last_updated.is_none() {
                    should_fetch = true;
                } else {
                    let last_updated = pkg.last_updated.unwrap();
                    let now = secs_since_epoch();
                    let diff = now - last_updated;
                    if diff > 60 {
                        should_fetch = true;
                    }
                }
            }
            Err(err) => match err {
                ServerError::PackageNotFound(_) => {
                    should_fetch = true;
                }
                _ => {
                    return Err(err);
                }
            },
        }

        if should_fetch {
            let metadata = download_pkg_metadata(pkg_name).await?;
            let pkg = MinimalPackageData::from_registry_meta(metadata);
            self.write_package(pkg)?;
        }

        Ok(())
    }
}
