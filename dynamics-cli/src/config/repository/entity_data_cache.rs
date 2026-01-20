//! Repository for entity data cache operations

use anyhow::{Context, Result};
use serde_json::Value;
use sqlx::SqlitePool;

/// Get cached entity data
pub async fn get(
    pool: &SqlitePool,
    environment_name: &str,
    entity_name: &str,
) -> Result<Option<(Vec<Value>, chrono::DateTime<chrono::Utc>)>> {
    let row: Option<(String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT data, cached_at
        FROM entity_data_cache
        WHERE environment_name = ? AND entity_name = ?
        "#,
    )
    .bind(environment_name)
    .bind(entity_name)
    .fetch_optional(pool)
    .await
    .context("Failed to fetch entity data cache")?;

    if let Some((data_json, cached_at)) = row {
        let data: Vec<Value> =
            serde_json::from_str(&data_json).context("Failed to parse cached entity data JSON")?;
        Ok(Some((data, cached_at)))
    } else {
        Ok(None)
    }
}

/// Set cached entity data
pub async fn set(
    pool: &SqlitePool,
    environment_name: &str,
    entity_name: &str,
    data: &[Value],
) -> Result<()> {
    let data_json =
        serde_json::to_string(data).context("Failed to serialize entity data to JSON")?;

    sqlx::query(
        r#"
        INSERT OR REPLACE INTO entity_data_cache (environment_name, entity_name, data, cached_at)
        VALUES (?, ?, ?, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(environment_name)
    .bind(entity_name)
    .bind(data_json)
    .execute(pool)
    .await
    .context("Failed to set entity data cache")?;

    Ok(())
}

/// Delete cached entity data for a specific entity
pub async fn delete(pool: &SqlitePool, environment_name: &str, entity_name: &str) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM entity_data_cache
        WHERE environment_name = ? AND entity_name = ?
        "#,
    )
    .bind(environment_name)
    .bind(entity_name)
    .execute(pool)
    .await
    .context("Failed to delete entity data cache")?;

    Ok(())
}

/// Delete all cached entity data for an environment
pub async fn delete_all_for_environment(pool: &SqlitePool, environment_name: &str) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM entity_data_cache
        WHERE environment_name = ?
        "#,
    )
    .bind(environment_name)
    .execute(pool)
    .await
    .context("Failed to delete entity data cache for environment")?;

    Ok(())
}
