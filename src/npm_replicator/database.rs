use std::sync::Arc;

use parking_lot::Mutex;
use rusqlite::{named_params, Connection, OpenFlags, OptionalExtension};

use crate::app_error::AppResult;

use super::types::document::MinimalPackageData;

#[derive(Clone, Debug)]
pub struct NpmDatabase {
    pub db_path: String,
    db: Arc<Mutex<Connection>>,
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

        Ok(Self {
            db_path: String::from(db_path),
            db: Arc::new(Mutex::new(connection)),
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

    pub fn update_last_seq(&self, next_seq: i64) -> AppResult<usize> {
        let connection = self.db.lock();
        let mut stmt =
            connection.prepare("INSERT OR REPLACE INTO last_sync (id, seq) VALUES (:id, :seq);")?;
        let res = stmt.execute(named_params! { ":id": "_last", ":seq": next_seq })?;
        Ok(res)
    }

    pub fn delete_package(&self, name: &str) -> AppResult<usize> {
        let connection = self.db.lock();
        let mut stmt = connection.prepare("DELETE FROM package WHERE id = (:id);")?;
        let res = stmt.execute(named_params! { ":id": name })?;
        Ok(res)
    }

    pub fn write_package(&self, pkg: MinimalPackageData) -> AppResult<usize> {
        if pkg.versions.is_empty() {
            println!("Tried to write pkg {}, but has no versions", pkg.name);
            return self.delete_package(&pkg.name);
        }

        let content = serde_json::to_string(&pkg)?;
        let res = {
            let connection = self.db.lock();
            let mut stmt = connection
                .prepare("INSERT OR REPLACE INTO package (id, content) VALUES (:id, :content);")?;
            stmt.execute(named_params! { ":id": pkg.name, ":content": content })
        }?;

        Ok(res)
    }

    pub fn get_package(&self, name: &str) -> AppResult<MinimalPackageData> {
        let content_val: Option<String> = {
            let connection = self.db.lock();
            let mut stmt = connection.prepare("SELECT content FROM package where id = (:id);")?;
            stmt.query_row(named_params! { ":id": name }, |row| row.get(0))
                .optional()?
        };

        if let Some(pkg_content) = content_val {
            let found_pkg = serde_json::from_str(&pkg_content)?;
            Ok(found_pkg)
        } else {
            Err(crate::app_error::ServerError::PackageNotFound(
                name.to_string(),
            ))
        }
    }

    pub fn get_package_count(&self) -> AppResult<i64> {
        let connection = self.db.lock();
        let mut stmt = connection.prepare("SELECT COUNT(*) FROM package;")?;
        let res = stmt.query_row(named_params! {}, |row| Ok(row.get(0).unwrap_or(0)))?;
        Ok(res)
    }
}
