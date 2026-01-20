//! Repository for entity cache operations

use anyhow::{Context, Result};
use sqlx::SqlitePool;

/// Get cached entities for an environment
pub async fn get(
    pool: &SqlitePool,
    environment_name: &str,
) -> Result<Option<(Vec<String>, chrono::DateTime<chrono::Utc>)>> {
    let row: Option<(String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT entities, cached_at
        FROM entity_cache
        WHERE environment_name = ?
        "#,
    )
    .bind(environment_name)
    .fetch_optional(pool)
    .await
    .context("Failed to fetch entity cache")?;

    if let Some((entities_json, cached_at)) = row {
        let entities: Vec<String> =
            serde_json::from_str(&entities_json).context("Failed to parse cached entities JSON")?;
        Ok(Some((entities, cached_at)))
    } else {
        Ok(None)
    }
}

/// Set cached entities for an environment
pub async fn set(pool: &SqlitePool, environment_name: &str, entities: Vec<String>) -> Result<()> {
    let entities_json =
        serde_json::to_string(&entities).context("Failed to serialize entities to JSON")?;

    sqlx::query(
        r#"
        INSERT OR REPLACE INTO entity_cache (environment_name, entities, cached_at)
        VALUES (?, ?, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(environment_name)
    .bind(entities_json)
    .execute(pool)
    .await
    .context("Failed to set entity cache")?;

    Ok(())
}

/// Delete cached entities for an environment
pub async fn delete(pool: &SqlitePool, environment_name: &str) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM entity_cache
        WHERE environment_name = ?
        "#,
    )
    .bind(environment_name)
    .execute(pool)
    .await
    .context("Failed to delete entity cache")?;

    Ok(())
}
