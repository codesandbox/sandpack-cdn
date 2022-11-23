use rusqlite::{named_params, Connection, OptionalExtension};

use crate::app_error::AppResult;

use super::types::document::MinimalPackageData;

#[derive(Debug)]
pub struct NpmDatabase {
    pub db_path: String,
    connection: Connection,
}

impl NpmDatabase {
    pub fn new(db_path: &str) -> AppResult<Self> {
        let connection = Connection::open(db_path)?;
        Ok(Self {
            db_path: String::from(db_path),
            connection,
        })
    }

    pub fn init(&self) -> AppResult<()> {
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS package (
                id    TEXT PRIMARY KEY,
                content  TEXT NOT NULL
            )",
            (),
        )?;

        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS last_sync (
                id    TEXT PRIMARY KEY,
                seq   INTEGER NOT NULL
            )",
            (),
        )?;

        Ok(())
    }

    pub fn get_last_seq(&self) -> AppResult<i64> {
        let mut prepared_statement = self
            .connection
            .prepare("SELECT id, seq FROM last_sync WHERE id = (:id)")?;

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
        let mut prepared_statement = self
            .connection
            .prepare("INSERT OR REPLACE INTO last_sync (id, seq) VALUES (:id, :seq)")?;
        let res = prepared_statement.execute(named_params! { ":id": "_last", ":seq": next_seq })?;
        Ok(res)
    }

    pub fn delete_package(&self, name: &str) -> AppResult<usize> {
        let mut prepared_statement = self
            .connection
            .prepare("DELETE FROM package WHERE id = (:id)")?;
        let res = prepared_statement.execute(named_params! { ":id": name })?;
        Ok(res)
    }

    pub fn write_package(&self, pkg: MinimalPackageData) -> AppResult<usize> {
        if pkg.versions.len() <= 0 {
            println!("Tried to write pkg {}, but has no versions", pkg.name);
            return self.delete_package(&pkg.name);
        }

        let mut prepared_statement = self
            .connection
            .prepare("INSERT OR REPLACE INTO package (id, content) VALUES (:id, :content)")?;
        let res = prepared_statement.execute(
            named_params! { ":id": pkg.name, ":content": serde_json::to_string(&pkg).unwrap() },
        )?;
        Ok(res)
    }

    pub fn get_package(&self, name: &str) -> AppResult<Option<MinimalPackageData>> {
        let mut prepared_statement = self
            .connection
            .prepare("SELECT content FROM package where id = (:id)")?;

        let res = prepared_statement
            .query_row(named_params! { ":id": name }, |row| {
                let content_val: String = row.get(0).unwrap();
                Ok(serde_json::from_str(&content_val).unwrap())
            })
            .optional()?;
        Ok(res)
    }
}
