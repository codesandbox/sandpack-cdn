use std::sync::Arc;

use lru::LruCache;
use parking_lot::Mutex;
use rusqlite::{named_params, Connection, OpenFlags, OptionalExtension};
use std::num::NonZeroUsize;
use tracing::{info, span, Level};

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

    #[tracing::instrument(name = "npm_db_get_last_seq", skip(self))]
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

    #[tracing::instrument(name = "npm_db_update_last_seq", skip(self))]
    pub fn update_last_seq(&self, next_seq: i64) -> AppResult<usize> {
        let connection = self.db.lock();
        let mut stmt =
            connection.prepare("INSERT OR REPLACE INTO last_sync (id, seq) VALUES (:id, :seq);")?;
        let res = stmt.execute(named_params! { ":id": "_last", ":seq": next_seq })?;
        Ok(res)
    }

    #[tracing::instrument(name = "npm_db_delete_package", skip(self))]
    pub fn delete_package(&self, pkg_name: &str) -> AppResult<usize> {
        let connection = self.db.lock();
        let mut stmt = connection.prepare("DELETE FROM package WHERE id = (:id);")?;
        let res = stmt.execute(named_params! { ":id": pkg_name })?;
        let mut cache = self.cache.lock();
        cache.pop(pkg_name);
        Ok(res)
    }

    #[tracing::instrument(name = "npm_db_write_package", skip(self, pkg), fields(pkg_name = pkg.name.as_str()))]
    pub fn write_package(&self, pkg: MinimalPackageData) -> AppResult<usize> {
        if pkg.versions.is_empty() {
            println!("Tried to write pkg {}, but has no versions", pkg.name);
            return self.delete_package(&pkg.name);
        }

        let pkg_name = pkg.name.clone();
        let content = serde_json::to_string(&pkg)?;
        let res = {
            let span = span!(Level::INFO, "sqlite_write_pkg").entered();
            let connection = self.db.lock();
            let mut stmt = connection
                .prepare("INSERT OR REPLACE INTO package (id, content) VALUES (:id, :content);")?;
            let res = stmt.execute(named_params! { ":id": pkg.name, ":content": content });
            span.exit();
            res
        }?;

        {
            let span = span!(Level::INFO, "delete_cached_pkg").entered();
            let mut cache = self.cache.lock();
            cache.pop(&pkg_name);
            span.exit();
        }

        Ok(res)
    }

    #[tracing::instrument(name = "npm_db_get_package", skip(self))]
    pub fn get_package(&self, pkg_name: &str) -> AppResult<MinimalPackageData> {
        {
            let mut cache = self.cache.lock();
            let cached_value = cache.get(pkg_name);
            if let Some(pkg_data) = cached_value {
                info!("NPM Cache hit");
                return Ok(pkg_data.clone());
            }
        };

        let content_val: Option<String> = {
            let span = span!(Level::INFO, "sqlite_get_pkg").entered();
            let connection = self.db.lock();
            let mut stmt = connection.prepare("SELECT content FROM package where id = (:id);")?;
            let res = stmt
                .query_row(named_params! { ":id": pkg_name }, |row| row.get(0))
                .optional()?;
            span.exit();
            res
        };

        if let Some(pkg_content) = content_val {
            let found_pkg: MinimalPackageData = {
                let span = span!(Level::INFO, "parse_pkg").entered();
                let res = serde_json::from_str(&pkg_content)?;
                span.exit();
                res
            };

            {
                let span = span!(Level::INFO, "write_cached_pkg").entered();
                let mut cache = self.cache.lock();
                cache.put(pkg_name.to_string(), found_pkg.clone());
                span.exit();
            }

            Ok(found_pkg)
        } else {
            Err(crate::app_error::ServerError::PackageNotFound(
                pkg_name.to_string(),
            ))
        }
    }

    #[tracing::instrument(name = "npm_db_get_package", skip(self))]
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