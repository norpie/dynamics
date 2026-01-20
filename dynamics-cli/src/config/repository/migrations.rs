//! Repository for migration operations

use crate::config::models::{DbComparison, DbMigration};
use anyhow::{Context, Result};
use sqlx::SqlitePool;

/// Represents a migration with its comparisons
#[derive(Debug, Clone)]
pub struct SavedMigration {
    pub name: String,
    pub source_env: String,
    pub target_env: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used: chrono::DateTime<chrono::Utc>,
}

/// Represents a comparison within a migration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedComparison {
    pub id: i64,
    pub migration_name: String,
    pub name: String,
    pub source_entity: String,             // Legacy: first source entity
    pub target_entity: String,             // Legacy: first target entity
    pub source_entities: Vec<String>,      // All source entities
    pub target_entities: Vec<String>,      // All target entities
    pub entity_comparison: Option<String>, // JSON
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used: chrono::DateTime<chrono::Utc>,
}

/// Insert or update migration
pub async fn insert(pool: &SqlitePool, migration: SavedMigration) -> Result<()> {
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO migrations (name, source_env, target_env, last_used)
        VALUES (?, ?, ?, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(&migration.name)
    .bind(&migration.source_env)
    .bind(&migration.target_env)
    .execute(pool)
    .await
    .with_context(|| format!("Failed to insert migration '{}'", migration.name))?;

    log::info!("Saved migration: {}", migration.name);
    Ok(())
}

/// Get migration by name
pub async fn get(pool: &SqlitePool, name: &str) -> Result<Option<SavedMigration>> {
    let row: Option<DbMigration> = sqlx::query_as(
        "SELECT name, source_env, target_env, created_at, last_used FROM migrations WHERE name = ?",
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("Failed to get migration '{}'", name))?;

    Ok(row.map(|r| SavedMigration {
        name: r.name,
        source_env: r.source_env,
        target_env: r.target_env,
        created_at: r.created_at,
        last_used: r.last_used,
    }))
}

/// List all migrations
pub async fn list(pool: &SqlitePool) -> Result<Vec<SavedMigration>> {
    let rows: Vec<DbMigration> = sqlx::query_as(
        "SELECT name, source_env, target_env, created_at, last_used FROM migrations ORDER BY last_used DESC",
    )
    .fetch_all(pool)
    .await
    .context("Failed to list migrations")?;

    Ok(rows
        .into_iter()
        .map(|r| SavedMigration {
            name: r.name,
            source_env: r.source_env,
            target_env: r.target_env,
            created_at: r.created_at,
            last_used: r.last_used,
        })
        .collect())
}

/// Delete migration by name (cascades to comparisons)
pub async fn delete(pool: &SqlitePool, name: &str) -> Result<()> {
    let result = sqlx::query("DELETE FROM migrations WHERE name = ?")
        .bind(name)
        .execute(pool)
        .await
        .with_context(|| format!("Failed to delete migration '{}'", name))?;

    if result.rows_affected() == 0 {
        anyhow::bail!("Migration '{}' not found", name);
    }

    log::info!("Deleted migration: {}", name);
    Ok(())
}

/// Update migration last_used timestamp
pub async fn touch(pool: &SqlitePool, name: &str) -> Result<()> {
    sqlx::query("UPDATE migrations SET last_used = CURRENT_TIMESTAMP WHERE name = ?")
        .bind(name)
        .execute(pool)
        .await
        .with_context(|| format!("Failed to update migration '{}'", name))?;

    Ok(())
}

/// Insert or update comparison for a migration
pub async fn insert_comparison(pool: &SqlitePool, comparison: SavedComparison) -> Result<i64> {
    // Serialize entity lists to JSON
    let source_entities_json = serde_json::to_string(&comparison.source_entities)
        .context("Failed to serialize source_entities")?;
    let target_entities_json = serde_json::to_string(&comparison.target_entities)
        .context("Failed to serialize target_entities")?;

    let result = sqlx::query(
        r#"
        INSERT INTO comparisons (migration_name, name, source_entity, target_entity, source_entities, target_entities, entity_comparison, last_used)
        VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
        ON CONFLICT(migration_name, name) DO UPDATE SET
            source_entity = excluded.source_entity,
            target_entity = excluded.target_entity,
            source_entities = excluded.source_entities,
            target_entities = excluded.target_entities,
            entity_comparison = excluded.entity_comparison,
            last_used = CURRENT_TIMESTAMP
        "#,
    )
    .bind(&comparison.migration_name)
    .bind(&comparison.name)
    .bind(&comparison.source_entity)
    .bind(&comparison.target_entity)
    .bind(source_entities_json)
    .bind(target_entities_json)
    .bind(&comparison.entity_comparison)
    .execute(pool)
    .await
    .with_context(|| format!("Failed to insert comparison '{}' for migration '{}'", comparison.name, comparison.migration_name))?;

    Ok(result.last_insert_rowid())
}

/// Get a single comparison by id
pub async fn get_comparison_by_id(pool: &SqlitePool, id: i64) -> Result<Option<SavedComparison>> {
    let row: Option<DbComparison> = sqlx::query_as(
        "SELECT id, migration_name, name, source_entity, target_entity, source_entities, target_entities, entity_comparison, created_at, last_used
         FROM comparisons WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("Failed to get comparison with id {}", id))?;

    Ok(row.map(|r| {
        // Backwards compatibility: If source_entities is NULL, use legacy single entity
        let source_entities = if let Some(json) = &r.source_entities {
            serde_json::from_str(json).unwrap_or_else(|_| vec![r.source_entity.clone()])
        } else {
            vec![r.source_entity.clone()]
        };

        let target_entities = if let Some(json) = &r.target_entities {
            serde_json::from_str(json).unwrap_or_else(|_| vec![r.target_entity.clone()])
        } else {
            vec![r.target_entity.clone()]
        };

        SavedComparison {
            id: r.id,
            migration_name: r.migration_name,
            name: r.name,
            source_entity: r.source_entity,
            target_entity: r.target_entity,
            source_entities,
            target_entities,
            entity_comparison: r.entity_comparison,
            created_at: r.created_at,
            last_used: r.last_used,
        }
    }))
}

/// Get comparisons for a migration
pub async fn get_comparisons(
    pool: &SqlitePool,
    migration_name: &str,
) -> Result<Vec<SavedComparison>> {
    let rows: Vec<DbComparison> = sqlx::query_as(
        "SELECT id, migration_name, name, source_entity, target_entity, source_entities, target_entities, entity_comparison, created_at, last_used
         FROM comparisons WHERE migration_name = ? ORDER BY last_used DESC",
    )
    .bind(migration_name)
    .fetch_all(pool)
    .await
    .with_context(|| format!("Failed to get comparisons for migration '{}'", migration_name))?;

    Ok(rows
        .into_iter()
        .map(|r| {
            // Backwards compatibility: If source_entities is NULL, use legacy single entity
            let source_entities = if let Some(json) = &r.source_entities {
                serde_json::from_str(json).unwrap_or_else(|_| vec![r.source_entity.clone()])
            } else {
                vec![r.source_entity.clone()]
            };

            let target_entities = if let Some(json) = &r.target_entities {
                serde_json::from_str(json).unwrap_or_else(|_| vec![r.target_entity.clone()])
            } else {
                vec![r.target_entity.clone()]
            };

            SavedComparison {
                id: r.id,
                migration_name: r.migration_name,
                name: r.name,
                source_entity: r.source_entity,
                target_entity: r.target_entity,
                source_entities,
                target_entities,
                entity_comparison: r.entity_comparison,
                created_at: r.created_at,
                last_used: r.last_used,
            }
        })
        .collect())
}

/// Delete comparison by id
pub async fn delete_comparison(pool: &SqlitePool, id: i64) -> Result<()> {
    let result = sqlx::query("DELETE FROM comparisons WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .with_context(|| format!("Failed to delete comparison with id {}", id))?;

    if result.rows_affected() == 0 {
        anyhow::bail!("Comparison with id {} not found", id);
    }

    log::info!("Deleted comparison: {}", id);
    Ok(())
}

/// Rename comparison by id
pub async fn rename_comparison(pool: &SqlitePool, id: i64, new_name: &str) -> Result<()> {
    let result =
        sqlx::query("UPDATE comparisons SET name = ?, last_used = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(new_name)
            .bind(id)
            .execute(pool)
            .await
            .with_context(|| format!("Failed to rename comparison with id {}", id))?;

    if result.rows_affected() == 0 {
        anyhow::bail!("Comparison with id {} not found", id);
    }

    log::info!("Renamed comparison {} to: {}", id, new_name);
    Ok(())
}
