use std::sync::Arc;

use r2d2::Pool;
use r2d2_sqlite::{
    rusqlite::{named_params, OptionalExtension},
    SqliteConnectionManager,
};

use crate::app_error::AppResult;

use super::types::document::MinimalPackageData;

#[derive(Clone, Debug)]
pub struct NpmDatabase {
    pub db_path: String,
    pool: Arc<Pool<SqliteConnectionManager>>,
}

impl NpmDatabase {
    pub fn new(db_path: &str) -> AppResult<Self> {
        let sqlite_connection_manager = SqliteConnectionManager::file(db_path);
        let sqlite_pool = r2d2::Pool::new(sqlite_connection_manager)
            .expect("Failed to create r2d2 SQLite connection pool");
        let pool_arc = Arc::new(sqlite_pool);
        Ok(Self {
            db_path: String::from(db_path),
            pool: pool_arc,
        })
    }

    pub fn init(&self) -> AppResult<()> {
        let connection = self.pool.get()?;

        connection.execute(
            "CREATE TABLE IF NOT EXISTS package (
                id    TEXT PRIMARY KEY,
                content  TEXT NOT NULL
            )",
            (),
        )?;

        connection.execute(
            "CREATE TABLE IF NOT EXISTS last_sync (
                id    TEXT PRIMARY KEY,
                seq   INTEGER NOT NULL
            )",
            (),
        )?;

        Ok(())
    }

    pub fn get_last_seq(&self) -> AppResult<i64> {
        let connection = self.pool.get()?;

        let mut prepared_statement =
            connection.prepare("SELECT id, seq FROM last_sync WHERE id = (:id)")?;

        let res = prepared_statement
            .query_row(named_params! { ":id": "_last" }, |row| {
                Ok(row.get(1).unwrap_or(0))
            })
            .optional()
            .unwrap_or(Some(0))
            .unwrap_or(0);

        Ok(res)
    }

    pub fn update_last_seq(&self, next_seq: i64) -> AppResult<usize> {
        let connection = self.pool.get()?;
        let mut prepared_statement =
            connection.prepare("INSERT OR REPLACE INTO last_sync (id, seq) VALUES (:id, :seq)")?;
        let res = prepared_statement.execute(named_params! { ":id": "_last", ":seq": next_seq })?;
        Ok(res)
    }

    pub fn delete_package(&self, name: &str) -> AppResult<usize> {
        let connection = self.pool.get()?;
        let mut prepared_statement = connection.prepare("DELETE FROM package WHERE id = (:id)")?;
        let res = prepared_statement.execute(named_params! { ":id": name })?;
        Ok(res)
    }

    pub fn write_package(&self, pkg: MinimalPackageData) -> AppResult<usize> {
        if pkg.versions.len() <= 0 {
            println!("Tried to write pkg {}, but has no versions", pkg.name);
            return self.delete_package(&pkg.name);
        }

        let connection = self.pool.get()?;
        let mut prepared_statement = connection
            .prepare("INSERT OR REPLACE INTO package (id, content) VALUES (:id, :content)")?;
        let res = prepared_statement.execute(
            named_params! { ":id": pkg.name, ":content": serde_json::to_string(&pkg).unwrap() },
        )?;
        Ok(res)
    }

    pub fn get_package(&self, name: &str) -> AppResult<MinimalPackageData> {
        let connection = self.pool.get()?;
        let mut prepared_statement =
            connection.prepare("SELECT content FROM package where id = (:id)")?;

        let res = prepared_statement
            .query_row(named_params! { ":id": name }, |row| {
                let content_val: String = row.get(0).unwrap();
                Ok(serde_json::from_str(&content_val).unwrap())
            })
            .optional()?;

        if let Some(found_pkg) = res {
            Ok(found_pkg)
        } else {
            Err(crate::app_error::ServerError::PackageNotFound(
                name.to_string(),
            ))
        }
    }

    pub fn get_package_count(&self) -> AppResult<i64> {
        let connection = self.pool.get()?;
        let mut prepared_statement = connection.prepare("SELECT COUNT(*) FROM package")?;

        let res =
            prepared_statement.query_row(named_params! {}, |row| Ok(row.get(0).unwrap_or(0)))?;
        Ok(res)
    }
}
