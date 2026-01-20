//! Repository for entity metadata cache operations

use crate::api::EntityMetadata;
use anyhow::{Context, Result};
use sqlx::SqlitePool;

/// Get cached entity metadata
pub async fn get(
    pool: &SqlitePool,
    environment_name: &str,
    entity_name: &str,
) -> Result<Option<(EntityMetadata, chrono::DateTime<chrono::Utc>)>> {
    let row: Option<(String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT metadata, cached_at
        FROM entity_metadata_cache
        WHERE environment_name = ? AND entity_name = ?
        "#,
    )
    .bind(environment_name)
    .bind(entity_name)
    .fetch_optional(pool)
    .await
    .context("Failed to fetch entity metadata cache")?;

    if let Some((metadata_json, cached_at)) = row {
        let metadata: EntityMetadata = serde_json::from_str(&metadata_json)
            .context("Failed to parse cached entity metadata JSON")?;
        Ok(Some((metadata, cached_at)))
    } else {
        Ok(None)
    }
}

/// Set cached entity metadata
pub async fn set(
    pool: &SqlitePool,
    environment_name: &str,
    entity_name: &str,
    metadata: &EntityMetadata,
) -> Result<()> {
    let metadata_json =
        serde_json::to_string(metadata).context("Failed to serialize entity metadata to JSON")?;

    sqlx::query(
        r#"
        INSERT OR REPLACE INTO entity_metadata_cache (environment_name, entity_name, metadata, cached_at)
        VALUES (?, ?, ?, CURRENT_TIMESTAMP)
        "#
    )
    .bind(environment_name)
    .bind(entity_name)
    .bind(metadata_json)
    .execute(pool)
    .await
    .context("Failed to set entity metadata cache")?;

    Ok(())
}

/// Delete cached entity metadata for a specific entity
pub async fn delete(pool: &SqlitePool, environment_name: &str, entity_name: &str) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM entity_metadata_cache
        WHERE environment_name = ? AND entity_name = ?
        "#,
    )
    .bind(environment_name)
    .bind(entity_name)
    .execute(pool)
    .await
    .context("Failed to delete entity metadata cache")?;

    Ok(())
}

/// Delete all cached entity metadata for an environment
pub async fn delete_all_for_environment(pool: &SqlitePool, environment_name: &str) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM entity_metadata_cache
        WHERE environment_name = ?
        "#,
    )
    .bind(environment_name)
    .execute(pool)
    .await
    .context("Failed to delete entity metadata cache for environment")?;

    Ok(())
}
