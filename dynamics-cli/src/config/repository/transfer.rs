//! Repository for transfer configuration operations

use anyhow::{Context, Result};
use sqlx::{Row, SqlitePool};

use crate::transfer::{
    EntityMapping, FieldMapping, FieldPath, MatchField, OperationFilter, Resolver, ResolverFallback,
    Transform, TransferConfig,
};

/// Summary of a transfer config (for listing)
#[derive(Debug, Clone)]
pub struct TransferConfigSummary {
    pub id: i64,
    pub name: String,
    pub source_env: String,
    pub target_env: String,
    pub entity_count: usize,
}

/// List all transfer configs (summary only)
pub async fn list_transfer_configs(pool: &SqlitePool) -> Result<Vec<TransferConfigSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT
            tc.id,
            tc.name,
            tc.source_env,
            tc.target_env,
            COUNT(tem.id) as entity_count
        FROM transfer_configs tc
        LEFT JOIN transfer_entity_mappings tem ON tc.id = tem.config_id
        GROUP BY tc.id
        ORDER BY tc.name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list transfer configs")?;

    let mut configs = Vec::new();
    for row in rows {
        configs.push(TransferConfigSummary {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            source_env: row.try_get("source_env")?,
            target_env: row.try_get("target_env")?,
            entity_count: row.try_get::<i64, _>("entity_count")? as usize,
        });
    }

    Ok(configs)
}

/// Get a transfer config by name (full structure with all mappings)
pub async fn get_transfer_config(pool: &SqlitePool, name: &str) -> Result<Option<TransferConfig>> {
    // Get the config
    let config_row = sqlx::query(
        "SELECT id, name, source_env, target_env FROM transfer_configs WHERE name = ?",
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .context("Failed to get transfer config")?;

    let config_row = match config_row {
        Some(row) => row,
        None => return Ok(None),
    };

    let config_id: i64 = config_row.try_get("id")?;

    // Get entity mappings
    let entity_rows = sqlx::query(
        r#"
        SELECT id, source_entity, target_entity, priority,
               allow_creates, allow_updates, allow_deletes, allow_deactivates
        FROM transfer_entity_mappings
        WHERE config_id = ?
        ORDER BY priority, source_entity
        "#,
    )
    .bind(config_id)
    .fetch_all(pool)
    .await
    .context("Failed to get entity mappings")?;

    let mut entity_mappings = Vec::new();
    for entity_row in entity_rows {
        let entity_id: i64 = entity_row.try_get("id")?;

        // Get field mappings for this entity
        let field_rows = sqlx::query(
            r#"
            SELECT id, target_field, transform_json
            FROM transfer_field_mappings
            WHERE entity_mapping_id = ?
            ORDER BY target_field
            "#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await
        .context("Failed to get field mappings")?;

        let mut field_mappings = Vec::new();
        for field_row in field_rows {
            let transform_json: String = field_row.try_get("transform_json")?;
            let transform: Transform = serde_json::from_str(&transform_json)
                .context("Failed to deserialize transform")?;

            field_mappings.push(FieldMapping {
                id: Some(field_row.try_get("id")?),
                target_field: field_row.try_get("target_field")?,
                transform,
            });
        }

        // Parse operation filter from boolean columns
        let operation_filter = OperationFilter {
            creates: entity_row.try_get::<i64, _>("allow_creates")? != 0,
            updates: entity_row.try_get::<i64, _>("allow_updates")? != 0,
            deletes: entity_row.try_get::<i64, _>("allow_deletes")? != 0,
            deactivates: entity_row.try_get::<i64, _>("allow_deactivates")? != 0,
        };

        entity_mappings.push(EntityMapping {
            id: Some(entity_id),
            source_entity: entity_row.try_get("source_entity")?,
            target_entity: entity_row.try_get("target_entity")?,
            priority: entity_row.try_get::<i64, _>("priority")? as u32,
            operation_filter,
            field_mappings,
        });
    }

    // Get resolvers
    let resolver_rows = sqlx::query(
        r#"
        SELECT id, name, source_entity, match_field, fallback
        FROM transfer_resolvers
        WHERE config_id = ?
        ORDER BY name
        "#,
    )
    .bind(config_id)
    .fetch_all(pool)
    .await
    .context("Failed to get resolvers")?;

    let mut resolvers = Vec::new();
    for row in resolver_rows {
        let fallback_str: String = row.try_get("fallback")?;
        let fallback_lower = fallback_str.to_lowercase();
        let fallback = if fallback_lower == "null" {
            ResolverFallback::Null
        } else if let Some(guid_str) = fallback_lower.strip_prefix("default:") {
            match uuid::Uuid::parse_str(guid_str) {
                Ok(guid) => ResolverFallback::Default(guid),
                Err(_) => ResolverFallback::Error, // Invalid GUID, fall back to Error
            }
        } else {
            ResolverFallback::Error
        };

        // Parse match_field - could be JSON array (new format) or plain string (legacy)
        let match_field_raw: String = row.try_get("match_field")?;
        let match_fields = if match_field_raw.starts_with('[') {
            // New JSON format: [{"source_path": {...}, "target_field": "..."}]
            serde_json::from_str::<Vec<MatchField>>(&match_field_raw)
                .unwrap_or_else(|_| vec![MatchField::simple(&match_field_raw)])
        } else {
            // Legacy single-field format: "emailaddress1"
            vec![MatchField::simple(&match_field_raw)]
        };

        resolvers.push(Resolver {
            id: Some(row.try_get("id")?),
            name: row.try_get("name")?,
            source_entity: row.try_get("source_entity")?,
            match_fields,
            fallback,
        });
    }

    Ok(Some(TransferConfig {
        id: Some(config_id),
        name: config_row.try_get("name")?,
        source_env: config_row.try_get("source_env")?,
        target_env: config_row.try_get("target_env")?,
        resolvers,
        entity_mappings,
    }))
}

/// Save a transfer config (insert or update)
pub async fn save_transfer_config(pool: &SqlitePool, config: &TransferConfig) -> Result<i64> {
    // Start a transaction
    let mut tx = pool.begin().await.context("Failed to start transaction")?;

    // Upsert the config
    let config_id = if let Some(id) = config.id {
        // Update existing
        sqlx::query(
            r#"
            UPDATE transfer_configs
            SET name = ?, source_env = ?, target_env = ?, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
        )
        .bind(&config.name)
        .bind(&config.source_env)
        .bind(&config.target_env)
        .bind(id)
        .execute(&mut *tx)
        .await
        .context("Failed to update transfer config")?;

        // Delete existing entity mappings (cascade will delete field mappings)
        sqlx::query("DELETE FROM transfer_entity_mappings WHERE config_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .context("Failed to delete old entity mappings")?;

        // Delete existing resolvers
        sqlx::query("DELETE FROM transfer_resolvers WHERE config_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .context("Failed to delete old resolvers")?;

        id
    } else {
        // Insert new
        let result = sqlx::query(
            r#"
            INSERT INTO transfer_configs (name, source_env, target_env)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(&config.name)
        .bind(&config.source_env)
        .bind(&config.target_env)
        .execute(&mut *tx)
        .await
        .context("Failed to insert transfer config")?;

        result.last_insert_rowid()
    };

    // Insert entity mappings
    for entity in &config.entity_mappings {
        let entity_result = sqlx::query(
            r#"
            INSERT INTO transfer_entity_mappings (
                config_id, source_entity, target_entity, priority,
                allow_creates, allow_updates, allow_deletes, allow_deactivates
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(config_id)
        .bind(&entity.source_entity)
        .bind(&entity.target_entity)
        .bind(entity.priority as i64)
        .bind(if entity.operation_filter.creates { 1i64 } else { 0i64 })
        .bind(if entity.operation_filter.updates { 1i64 } else { 0i64 })
        .bind(if entity.operation_filter.deletes { 1i64 } else { 0i64 })
        .bind(if entity.operation_filter.deactivates { 1i64 } else { 0i64 })
        .execute(&mut *tx)
        .await
        .context("Failed to insert entity mapping")?;

        let entity_id = entity_result.last_insert_rowid();

        // Insert field mappings
        for field in &entity.field_mappings {
            let transform_json = serde_json::to_string(&field.transform)
                .context("Failed to serialize transform")?;

            sqlx::query(
                r#"
                INSERT INTO transfer_field_mappings (entity_mapping_id, target_field, transform_json)
                VALUES (?, ?, ?)
                "#,
            )
            .bind(entity_id)
            .bind(&field.target_field)
            .bind(&transform_json)
            .execute(&mut *tx)
            .await
            .context("Failed to insert field mapping")?;
        }
    }

    // Insert resolvers
    for resolver in &config.resolvers {
        let fallback_str = match &resolver.fallback {
            ResolverFallback::Error => "error".to_string(),
            ResolverFallback::Null => "null".to_string(),
            ResolverFallback::Default(guid) => format!("default:{}", guid),
        };

        // Serialize match_fields as JSON
        let match_fields_json = serde_json::to_string(&resolver.match_fields)
            .unwrap_or_else(|_| "[]".to_string());

        sqlx::query(
            r#"
            INSERT INTO transfer_resolvers (config_id, name, source_entity, match_field, fallback)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(config_id)
        .bind(&resolver.name)
        .bind(&resolver.source_entity)
        .bind(&match_fields_json)
        .bind(&fallback_str)
        .execute(&mut *tx)
        .await
        .context("Failed to insert resolver")?;
    }

    tx.commit().await.context("Failed to commit transaction")?;

    Ok(config_id)
}

/// Delete a transfer config by name
pub async fn delete_transfer_config(pool: &SqlitePool, name: &str) -> Result<bool> {
    let result = sqlx::query("DELETE FROM transfer_configs WHERE name = ?")
        .bind(name)
        .execute(pool)
        .await
        .context("Failed to delete transfer config")?;

    Ok(result.rows_affected() > 0)
}

/// Check if a transfer config exists
pub async fn transfer_config_exists(pool: &SqlitePool, name: &str) -> Result<bool> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM transfer_configs WHERE name = ?")
            .bind(name)
            .fetch_optional(pool)
            .await
            .context("Failed to check transfer config existence")?;

    Ok(row.is_some())
}
