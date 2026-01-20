//! Repository for legacy configuration features (entity mappings, settings, etc.)

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::collections::HashMap;

// Entity mappings

/// Add entity mapping (singular -> plural)
pub async fn add_entity_mapping(pool: &SqlitePool, singular: String, plural: String) -> Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO entity_mappings (singular_name, plural_name) VALUES (?, ?)",
    )
    .bind(&singular)
    .bind(&plural)
    .execute(pool)
    .await
    .with_context(|| format!("Failed to add entity mapping: {} -> {}", singular, plural))?;

    log::info!("Added entity mapping: {} -> {}", singular, plural);
    Ok(())
}

/// Get entity mapping
pub async fn get_entity_mapping(pool: &SqlitePool, singular: &str) -> Result<Option<String>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT plural_name FROM entity_mappings WHERE singular_name = ?")
            .bind(singular)
            .fetch_optional(pool)
            .await
            .with_context(|| format!("Failed to get entity mapping for '{}'", singular))?;

    Ok(row.map(|(plural,)| plural))
}

/// List all entity mappings
pub async fn list_entity_mappings(pool: &SqlitePool) -> Result<Vec<(String, String)>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT singular_name, plural_name FROM entity_mappings ORDER BY singular_name",
    )
    .fetch_all(pool)
    .await
    .context("Failed to list entity mappings")?;

    Ok(rows)
}

/// Delete entity mapping
pub async fn delete_entity_mapping(pool: &SqlitePool, singular: &str) -> Result<()> {
    let result = sqlx::query("DELETE FROM entity_mappings WHERE singular_name = ?")
        .bind(singular)
        .execute(pool)
        .await
        .with_context(|| format!("Failed to delete entity mapping for '{}'", singular))?;

    if result.rows_affected() == 0 {
        anyhow::bail!("Entity mapping for '{}' not found", singular);
    }

    log::info!("Deleted entity mapping: {}", singular);
    Ok(())
}

// Settings

/// Get setting value
pub async fn get_setting(pool: &SqlitePool, key: &str) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .with_context(|| format!("Failed to get setting '{}'", key))?;

    Ok(row.map(|(value,)| value))
}

/// Set setting value
pub async fn set_setting(pool: &SqlitePool, key: String, value: String) -> Result<()> {
    // Determine value type
    let value_type = if value.parse::<i64>().is_ok() {
        "integer"
    } else if value.parse::<bool>().is_ok() {
        "boolean"
    } else if value.starts_with('{') || value.starts_with('[') {
        "json"
    } else {
        "string"
    };

    sqlx::query("INSERT OR REPLACE INTO settings (key, value, type) VALUES (?, ?, ?)")
        .bind(&key)
        .bind(&value)
        .bind(value_type)
        .execute(pool)
        .await
        .with_context(|| format!("Failed to set setting '{}' = '{}'", key, value))?;

    log::debug!("Set setting: {} = {}", key, value);
    Ok(())
}

/// List all settings
pub async fn list_settings(pool: &SqlitePool) -> Result<HashMap<String, String>> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM settings ORDER BY key")
            .fetch_all(pool)
            .await
            .context("Failed to list settings")?;

    Ok(rows.into_iter().collect())
}

/// Delete setting
pub async fn delete_setting(pool: &SqlitePool, key: &str) -> Result<()> {
    let result = sqlx::query("DELETE FROM settings WHERE key = ?")
        .bind(key)
        .execute(pool)
        .await
        .with_context(|| format!("Failed to delete setting '{}'", key))?;

    if result.rows_affected() == 0 {
        anyhow::bail!("Setting '{}' not found", key);
    }

    log::debug!("Deleted setting: {}", key);
    Ok(())
}

// Field mappings

/// Add field mapping
pub async fn add_field_mapping(
    pool: &SqlitePool,
    source_entity: String,
    target_entity: String,
    source_field: String,
    target_field: String,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO field_mappings
        (source_entity, target_entity, source_field, target_field)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(&source_entity)
    .bind(&target_entity)
    .bind(&source_field)
    .bind(&target_field)
    .execute(pool)
    .await
    .with_context(|| {
        format!(
            "Failed to add field mapping: {}:{} -> {}:{}",
            source_entity, source_field, target_entity, target_field
        )
    })?;

    log::info!(
        "Added field mapping: {}:{} -> {}:{}",
        source_entity,
        source_field,
        target_entity,
        target_field
    );
    Ok(())
}

/// Get field mappings for entity pair
pub async fn get_field_mappings(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
) -> Result<HashMap<String, String>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT source_field, target_field FROM field_mappings WHERE source_entity = ? AND target_entity = ?",
    )
    .bind(source_entity)
    .bind(target_entity)
    .fetch_all(pool)
    .await
    .with_context(|| {
        format!(
            "Failed to get field mappings for {} -> {}",
            source_entity, target_entity
        )
    })?;

    Ok(rows.into_iter().collect())
}

/// Delete field mapping
pub async fn delete_field_mapping(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
    source_field: &str,
) -> Result<()> {
    let result = sqlx::query(
        "DELETE FROM field_mappings WHERE source_entity = ? AND target_entity = ? AND source_field = ?",
    )
    .bind(source_entity)
    .bind(target_entity)
    .bind(source_field)
    .execute(pool)
    .await
    .with_context(|| {
        format!(
            "Failed to delete field mapping: {}:{} -> {}",
            source_entity, source_field, target_entity
        )
    })?;

    if result.rows_affected() == 0 {
        anyhow::bail!(
            "Field mapping not found: {}:{} -> {}",
            source_entity,
            source_field,
            target_entity
        );
    }

    log::info!(
        "Deleted field mapping: {}:{} -> {}",
        source_entity,
        source_field,
        target_entity
    );
    Ok(())
}

// Prefix mappings

/// Add prefix mapping
pub async fn add_prefix_mapping(
    pool: &SqlitePool,
    source_entity: String,
    target_entity: String,
    source_prefix: String,
    target_prefix: String,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO prefix_mappings
        (source_entity, target_entity, source_prefix, target_prefix)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(&source_entity)
    .bind(&target_entity)
    .bind(&source_prefix)
    .bind(&target_prefix)
    .execute(pool)
    .await
    .with_context(|| {
        format!(
            "Failed to add prefix mapping: {}:{} -> {}:{}",
            source_entity, source_prefix, target_entity, target_prefix
        )
    })?;

    log::info!(
        "Added prefix mapping: {}:{} -> {}:{}",
        source_entity,
        source_prefix,
        target_entity,
        target_prefix
    );
    Ok(())
}

/// Get prefix mappings for entity pair
pub async fn get_prefix_mappings(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
) -> Result<HashMap<String, String>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT source_prefix, target_prefix FROM prefix_mappings WHERE source_entity = ? AND target_entity = ?",
    )
    .bind(source_entity)
    .bind(target_entity)
    .fetch_all(pool)
    .await
    .with_context(|| {
        format!(
            "Failed to get prefix mappings for {} -> {}",
            source_entity, target_entity
        )
    })?;

    Ok(rows.into_iter().collect())
}

/// Delete prefix mapping
pub async fn delete_prefix_mapping(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
    source_prefix: &str,
) -> Result<()> {
    let result = sqlx::query(
        "DELETE FROM prefix_mappings WHERE source_entity = ? AND target_entity = ? AND source_prefix = ?",
    )
    .bind(source_entity)
    .bind(target_entity)
    .bind(source_prefix)
    .execute(pool)
    .await
    .with_context(|| {
        format!(
            "Failed to delete prefix mapping: {}:{} -> {}",
            source_entity, source_prefix, target_entity
        )
    })?;

    if result.rows_affected() == 0 {
        anyhow::bail!(
            "Prefix mapping not found: {}:{} -> {}",
            source_entity,
            source_prefix,
            target_entity
        );
    }

    log::info!(
        "Deleted prefix mapping: {}:{} -> {}",
        source_entity,
        source_prefix,
        target_entity
    );
    Ok(())
}
