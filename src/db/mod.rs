mod schema;

use anyhow::{Context, Result};
use duckdb::{Connection, params};
use std::sync::{Arc, Mutex};

pub use schema::MIGRATIONS;

/// Extension trait for optional query results
trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, duckdb::Error>;
}

impl<T> OptionalExt<T> for Result<T, duckdb::Error> {
    fn optional(self) -> Result<Option<T>, duckdb::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(duckdb::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path).context("Failed to open database")?;

        // Enable optimizations
        conn.execute_batch(
            "SET threads=4;
             SET memory_limit='512MB';",
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Create migrations table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS _migrations (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Run migrations
        for (id, (name, sql)) in MIGRATIONS.iter().enumerate() {
            let applied: Option<i32> = conn
                .query_row(
                    "SELECT id FROM _migrations WHERE id = ?",
                    params![id as i32],
                    |row| row.get(0),
                )
                .optional()?;

            if applied.is_none() {
                tracing::info!("Running migration: {}", name);
                conn.execute_batch(sql)?;
                conn.execute(
                    "INSERT INTO _migrations (id, name) VALUES (?, ?)",
                    params![id as i32, *name],
                )?;
            }
        }

        Ok(())
    }

    pub fn execute<P: duckdb::Params>(&self, sql: &str, params: P) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.execute(sql, params)?)
    }

    pub fn execute_batch(&self, sql: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.execute_batch(sql)?)
    }

    pub fn query_one<T, P, F>(&self, sql: &str, params: P, f: F) -> Result<Option<T>>
    where
        P: duckdb::Params,
        F: FnOnce(&duckdb::Row<'_>) -> Result<T, duckdb::Error>,
    {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(sql, params, f).optional()?;
        Ok(result)
    }

    pub fn query_all<T, P, F>(&self, sql: &str, params: P, mut f: F) -> Result<Vec<T>>
    where
        P: duckdb::Params,
        F: FnMut(&duckdb::Row<'_>) -> Result<T, duckdb::Error>,
    {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query(params)?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            results.push(f(row)?);
        }
        Ok(results)
    }

    // Sync status methods
    pub fn get_sync_height(&self) -> Result<i64> {
        let result: Option<i64> = self.query_one(
            "SELECT MAX(height) FROM blocks",
            [],
            |row| row.get(0),
        )?;
        Ok(result.unwrap_or(-1))
    }

    pub fn get_stats(&self) -> Result<DbStats> {
        let conn = self.conn.lock().unwrap();

        let block_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM blocks", [], |row| row.get(0))
            .unwrap_or(0);

        let tx_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))
            .unwrap_or(0);

        let box_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM boxes", [], |row| row.get(0))
            .unwrap_or(0);

        let unspent_box_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM boxes WHERE spent_tx_id IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let token_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tokens", [], |row| row.get(0))
            .unwrap_or(0);

        let address_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM address_stats", [], |row| row.get(0))
            .unwrap_or(0);

        Ok(DbStats {
            block_count,
            tx_count,
            box_count,
            unspent_box_count,
            token_count,
            address_count,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DbStats {
    pub block_count: i64,
    pub tx_count: i64,
    pub box_count: i64,
    pub unspent_box_count: i64,
    pub token_count: i64,
    pub address_count: i64,
}
