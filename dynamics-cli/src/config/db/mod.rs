//! Database connection and schema management

use anyhow::{Context, Result};
use sqlx::{Row, SqlitePool};
use std::path::Path;

/// Connect to SQLite database with proper configuration
pub async fn connect(db_path: &Path) -> Result<SqlitePool> {
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());

    let pool = SqlitePool::connect(&database_url)
        .await
        .with_context(|| format!("Failed to connect to database: {}", db_path.display()))?;

    // Configure SQLite for better concurrency and safety
    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(&pool)
        .await
        .context("Failed to enable WAL mode")?;

    sqlx::query("PRAGMA synchronous = NORMAL")
        .execute(&pool)
        .await
        .context("Failed to set synchronous mode")?;

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .context("Failed to enable foreign keys")?;

    sqlx::query("PRAGMA temp_store = MEMORY")
        .execute(&pool)
        .await
        .context("Failed to set temp store")?;

    log::debug!("Connected to SQLite database: {}", db_path.display());
    Ok(pool)
}

/// Connect to in-memory database for testing
pub async fn connect_memory() -> Result<SqlitePool> {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .context("Failed to connect to in-memory database")?;

    // Enable foreign keys for testing
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .context("Failed to enable foreign keys")?;

    log::debug!("Connected to in-memory SQLite database");
    Ok(pool)
}

/// Run database migrations using the new migration system
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    log::debug!("Running database migrations");

    let migration_manager = crate::config::migrations::MigrationManager::new(pool);
    migration_manager.migrate_up().await?;

    Ok(())
}

/// Get database info for debugging
pub async fn get_db_info(pool: &SqlitePool) -> Result<DatabaseInfo> {
    let version: String = sqlx::query_scalar("SELECT sqlite_version()")
        .fetch_one(pool)
        .await
        .context("Failed to get SQLite version")?;

    let schema_version: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM schema_version")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    let table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_one(pool)
    .await
    .context("Failed to get table count")?;

    let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
        .fetch_one(pool)
        .await
        .context("Failed to get page size")?;

    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(pool)
        .await
        .context("Failed to get journal mode")?;

    Ok(DatabaseInfo {
        sqlite_version: version,
        schema_version,
        table_count,
        page_size,
        journal_mode,
    })
}

#[derive(Debug)]
pub struct DatabaseInfo {
    pub sqlite_version: String,
    pub schema_version: i64,
    pub table_count: i64,
    pub page_size: i64,
    pub journal_mode: String,
}
