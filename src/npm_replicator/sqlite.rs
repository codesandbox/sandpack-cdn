use std::sync::Arc;

use lru::LruCache;
use parking_lot::Mutex;
use rusqlite::{named_params, Connection, OpenFlags, OptionalExtension};
use std::num::NonZeroUsize;

use crate::app_error::AppResult;

use super::types::document::MinimalPackageData;

#[derive(Clone, Debug)]
pub struct NpmDatabase {
    pub db_path: String,
    db: Arc<Mutex<Connection>>,
    cache: Arc<Mutex<LruCache<String, MinimalPackageData>>>,
}

impl NpmDatabase {
    pub fn new(db_path: &str) -> AppResult<Self> {
        let connection = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_URI,
        )?;
        let cache = LruCache::new(NonZeroUsize::new(500).unwrap());

        Ok(Self {
            db_path: String::from(db_path),
            db: Arc::new(Mutex::new(connection)),
            cache: Arc::new(Mutex::new(cache)),
        })
    }

    pub fn init(&self) -> AppResult<()> {
        let connection = self.db.lock();

        connection.execute(
            "CREATE TABLE IF NOT EXISTS package (
                id    TEXT PRIMARY KEY,
                content  TEXT NOT NULL
            );",
            (),
        )?;

        connection.execute(
            "CREATE TABLE IF NOT EXISTS last_sync (
                id    TEXT PRIMARY KEY,
                seq   INTEGER NOT NULL
            );",
            (),
        )?;

        Ok(())
    }

    pub fn get_last_seq(&self) -> AppResult<i64> {
        let connection = self.db.lock();

        let mut stmt = connection.prepare("SELECT id, seq FROM last_sync WHERE id = (:id);")?;

        let res = stmt
            .query_row(named_params! { ":id": "_last" }, |row| {
                Ok(row.get(1).unwrap_or(0))
            })
            .optional()
            .unwrap_or(Some(0))
            .unwrap_or(0);

        Ok(res)
    }

    pub fn get_package(&self, pkg_name: &str) -> AppResult<MinimalPackageData> {
        {
            let mut cache = self.cache.lock();
            let cached_value = cache.get(pkg_name);
            if let Some(pkg_data) = cached_value {
                return Ok(pkg_data.clone());
            }
        };

        let content_val: Option<String> = {
            let connection = self.db.lock();
            let mut stmt = connection.prepare("SELECT content FROM package where id = (:id);")?;
            stmt.query_row(named_params! { ":id": pkg_name }, |row| row.get(0))
                .optional()?
        };

        if let Some(pkg_content) = content_val {
            let found_pkg: MinimalPackageData = {
                serde_json::from_str(&pkg_content)?
            };

            {
                let mut cache = self.cache.lock();
                cache.put(pkg_name.to_string(), found_pkg.clone());
            }

            Ok(found_pkg)
        } else {
            Err(crate::app_error::ServerError::PackageNotFound(
                pkg_name.to_string(),
            ))
        }
    }

    pub fn list_packages(&self) -> AppResult<Vec<String>> {
        let connection = self.db.lock();
        let mut stmt = connection.prepare("SELECT id FROM package;")?;
        let name_iter = stmt
            .query_map(named_params! {}, |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })
            .optional()?
            .unwrap();
        let mut result_vec: Vec<String> = Vec::new();
        for line in name_iter {
            result_vec.push(line.unwrap());
        }
        Ok(result_vec)
    }
}
