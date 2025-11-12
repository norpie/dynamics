//! Field and prefix mappings repository

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::collections::HashMap;

/// Get all field mappings for a source/target entity pair
/// Returns HashMap<source_field, Vec<target_fields>> to support 1-to-N mappings
pub async fn get_field_mappings(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
) -> Result<HashMap<String, Vec<String>>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT source_field, target_field FROM field_mappings
         WHERE source_entity = ? AND target_entity = ?
         ORDER BY source_field, target_field",
    )
    .bind(source_entity)
    .bind(target_entity)
    .fetch_all(pool)
    .await
    .context("Failed to get field mappings")?;

    // Group by source_field to support 1-to-N mappings
    let mut mappings: HashMap<String, Vec<String>> = HashMap::new();
    for (source_field, target_field) in rows {
        mappings.entry(source_field)
            .or_insert_with(Vec::new)
            .push(target_field);
    }

    Ok(mappings)
}

/// Set a field mapping (insert new source->target pair)
/// With 1-to-N support, this adds a new target to a source (or does nothing if already exists)
pub async fn set_field_mapping(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
    source_field: &str,
    target_field: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO field_mappings (source_entity, target_entity, source_field, target_field)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(source_entity, target_entity, source_field, target_field)
         DO NOTHING",
    )
    .bind(source_entity)
    .bind(target_entity)
    .bind(source_field)
    .bind(target_field)
    .execute(pool)
    .await
    .context("Failed to set field mapping")?;

    Ok(())
}

/// Delete all mappings for a source field (removes all targets)
pub async fn delete_field_mapping(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
    source_field: &str,
) -> Result<()> {
    sqlx::query(
        "DELETE FROM field_mappings
         WHERE source_entity = ? AND target_entity = ? AND source_field = ?",
    )
    .bind(source_entity)
    .bind(target_entity)
    .bind(source_field)
    .execute(pool)
    .await
    .context("Failed to delete field mapping")?;

    Ok(())
}

/// Delete a specific source->target mapping (for 1-to-N support)
/// Use this when removing one target from a source that maps to multiple targets
pub async fn delete_specific_field_mapping(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
    source_field: &str,
    target_field: &str,
) -> Result<()> {
    sqlx::query(
        "DELETE FROM field_mappings
         WHERE source_entity = ? AND target_entity = ?
           AND source_field = ? AND target_field = ?",
    )
    .bind(source_entity)
    .bind(target_entity)
    .bind(source_field)
    .bind(target_field)
    .execute(pool)
    .await
    .context("Failed to delete specific field mapping")?;

    Ok(())
}

/// Get all prefix mappings for a source/target entity pair
/// Returns HashMap<source_prefix, Vec<target_prefixes>> to support 1-to-N mappings
pub async fn get_prefix_mappings(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
) -> Result<HashMap<String, Vec<String>>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT source_prefix, target_prefix FROM prefix_mappings
         WHERE source_entity = ? AND target_entity = ?
         ORDER BY source_prefix, target_prefix",
    )
    .bind(source_entity)
    .bind(target_entity)
    .fetch_all(pool)
    .await
    .context("Failed to get prefix mappings")?;

    // Group by source_prefix to support 1-to-N mappings
    let mut mappings: HashMap<String, Vec<String>> = HashMap::new();
    for (source_prefix, target_prefix) in rows {
        mappings.entry(source_prefix)
            .or_insert_with(Vec::new)
            .push(target_prefix);
    }

    Ok(mappings)
}

/// Set a prefix mapping (insert new source->target pair)
/// With 1-to-N support, this adds a new target to a source (or does nothing if already exists)
pub async fn set_prefix_mapping(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
    source_prefix: &str,
    target_prefix: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO prefix_mappings (source_entity, target_entity, source_prefix, target_prefix)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(source_entity, target_entity, source_prefix, target_prefix)
         DO NOTHING",
    )
    .bind(source_entity)
    .bind(target_entity)
    .bind(source_prefix)
    .bind(target_prefix)
    .execute(pool)
    .await
    .context("Failed to set prefix mapping")?;

    Ok(())
}

/// Delete all mappings for a source prefix (removes all targets)
pub async fn delete_prefix_mapping(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
    source_prefix: &str,
) -> Result<()> {
    sqlx::query(
        "DELETE FROM prefix_mappings
         WHERE source_entity = ? AND target_entity = ? AND source_prefix = ?",
    )
    .bind(source_entity)
    .bind(target_entity)
    .bind(source_prefix)
    .execute(pool)
    .await
    .context("Failed to delete prefix mapping")?;

    Ok(())
}

/// Delete a specific source->target prefix mapping (for 1-to-N support)
/// Use this when removing one target from a source that maps to multiple targets
pub async fn delete_specific_prefix_mapping(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
    source_prefix: &str,
    target_prefix: &str,
) -> Result<()> {
    sqlx::query(
        "DELETE FROM prefix_mappings
         WHERE source_entity = ? AND target_entity = ?
           AND source_prefix = ? AND target_prefix = ?",
    )
    .bind(source_entity)
    .bind(target_entity)
    .bind(source_prefix)
    .bind(target_prefix)
    .execute(pool)
    .await
    .context("Failed to delete specific prefix mapping")?;

    Ok(())
}

/// Get imported mappings for a source/target entity pair
/// Returns HashMap<source_field, Vec<target_fields>> to support 1-to-N mappings
pub async fn get_imported_mappings(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
) -> Result<(HashMap<String, Vec<String>>, Option<String>)> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT source_field, target_field, source_file FROM imported_mappings
         WHERE source_entity = ? AND target_entity = ?
         ORDER BY source_field, target_field",
    )
    .bind(source_entity)
    .bind(target_entity)
    .fetch_all(pool)
    .await
    .context("Failed to get imported mappings")?;

    // Group by source_field to support 1-to-N mappings
    let mut mappings: HashMap<String, Vec<String>> = HashMap::new();
    for (source_field, target_field, _) in &rows {
        mappings.entry(source_field.clone())
            .or_insert_with(Vec::new)
            .push(target_field.clone());
    }

    // Get the source file from the first row (all rows should have the same source_file)
    let source_file = rows.first().map(|(_, _, file)| file.clone());

    Ok((mappings, source_file))
}

/// Set imported mappings (clears existing imports for this entity pair and inserts new ones)
/// Accepts HashMap<source_field, Vec<target_fields>> to support 1-to-N mappings
pub async fn set_imported_mappings(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
    mappings: &HashMap<String, Vec<String>>,
    source_file: &str,
) -> Result<()> {
    // Start transaction
    let mut tx = pool.begin().await.context("Failed to begin transaction")?;

    // Clear existing imported mappings for this entity pair
    sqlx::query(
        "DELETE FROM imported_mappings
         WHERE source_entity = ? AND target_entity = ?",
    )
    .bind(source_entity)
    .bind(target_entity)
    .execute(&mut *tx)
    .await
    .context("Failed to clear existing imported mappings")?;

    // Insert new mappings (one row per source->target pair)
    for (source_field, target_fields) in mappings {
        for target_field in target_fields {
            sqlx::query(
                "INSERT INTO imported_mappings (source_entity, target_entity, source_field, target_field, source_file)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(source_entity)
            .bind(target_entity)
            .bind(source_field)
            .bind(target_field)
            .bind(source_file)
            .execute(&mut *tx)
            .await
            .context("Failed to insert imported mapping")?;
        }
    }

    // Commit transaction
    tx.commit().await.context("Failed to commit transaction")?;

    Ok(())
}

/// Clear all imported mappings for a source/target entity pair
pub async fn clear_imported_mappings(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
) -> Result<()> {
    sqlx::query(
        "DELETE FROM imported_mappings
         WHERE source_entity = ? AND target_entity = ?",
    )
    .bind(source_entity)
    .bind(target_entity)
    .execute(pool)
    .await
    .context("Failed to clear imported mappings")?;

    Ok(())
}

/// Get ignored items for entity comparison
pub async fn get_ignored_items(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
) -> Result<std::collections::HashSet<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT item_id FROM ignored_items
         WHERE source_entity = ? AND target_entity = ?",
    )
    .bind(source_entity)
    .bind(target_entity)
    .fetch_all(pool)
    .await
    .context("Failed to fetch ignored items")?;

    let ignored: std::collections::HashSet<String> = rows.into_iter()
        .map(|(item_id,)| item_id)
        .collect();

    Ok(ignored)
}

/// Set ignored items for entity comparison
pub async fn set_ignored_items(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
    ignored: &std::collections::HashSet<String>,
) -> Result<()> {
    // Clear existing ignored items
    clear_ignored_items(pool, source_entity, target_entity).await?;

    // Insert new ignored items
    for item_id in ignored {
        sqlx::query(
            "INSERT INTO ignored_items (source_entity, target_entity, item_id)
             VALUES (?, ?, ?)",
        )
        .bind(source_entity)
        .bind(target_entity)
        .bind(item_id)
        .execute(pool)
        .await
        .context("Failed to insert ignored item")?;
    }

    Ok(())
}

/// Clear all ignored items for entity comparison
pub async fn clear_ignored_items(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
) -> Result<()> {
    sqlx::query(
        "DELETE FROM ignored_items
         WHERE source_entity = ? AND target_entity = ?",
    )
    .bind(source_entity)
    .bind(target_entity)
    .execute(pool)
    .await
    .context("Failed to clear ignored items")?;

    Ok(())
}

/// Get all negative matches for a source/target entity pair
/// Returns HashSet of source field names that should be blocked from prefix matching
pub async fn get_negative_matches(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
) -> Result<std::collections::HashSet<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT source_field FROM negative_matches
         WHERE source_entity = ? AND target_entity = ?
         ORDER BY source_field",
    )
    .bind(source_entity)
    .bind(target_entity)
    .fetch_all(pool)
    .await
    .context("Failed to get negative matches")?;

    Ok(rows.into_iter().map(|(field,)| field).collect())
}

/// Add a negative match to block a specific source field from prefix matching
pub async fn add_negative_match(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
    source_field: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO negative_matches (source_entity, target_entity, source_field)
         VALUES (?, ?, ?)
         ON CONFLICT(source_entity, target_entity, source_field)
         DO NOTHING",
    )
    .bind(source_entity)
    .bind(target_entity)
    .bind(source_field)
    .execute(pool)
    .await
    .context("Failed to add negative match")?;

    Ok(())
}

/// Delete a negative match, allowing the source field to be prefix-matched again
pub async fn delete_negative_match(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
    source_field: &str,
) -> Result<()> {
    sqlx::query(
        "DELETE FROM negative_matches
         WHERE source_entity = ? AND target_entity = ? AND source_field = ?",
    )
    .bind(source_entity)
    .bind(target_entity)
    .bind(source_field)
    .execute(pool)
    .await
    .context("Failed to delete negative match")?;

    Ok(())
}

// ============================================================================
// Multi-Entity Support (N:M Comparisons)
// ============================================================================

/// Helper function to parse qualified field names like "entity.field"
/// Returns (entity_name, field_name) or error if invalid format
fn parse_qualified(qualified: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = qualified.splitn(2, '.').collect();
    if parts.len() == 2 {
        Ok((parts[0].to_string(), parts[1].to_string()))
    } else {
        anyhow::bail!("Invalid qualified field name: '{}'. Expected format: 'entity.field'", qualified)
    }
}

/// Get all field mappings for multiple source and target entities
/// Returns qualified names (entity.field) for multi-entity mode, unqualified for single-entity mode
/// This provides backwards compatibility with existing 1:1 comparisons
pub async fn get_field_mappings_multi(
    pool: &SqlitePool,
    source_entities: &[String],
    target_entities: &[String],
) -> Result<HashMap<String, Vec<String>>> {
    // Single-entity mode: return unqualified names (backwards compat)
    if source_entities.len() == 1 && target_entities.len() == 1 {
        return get_field_mappings(pool, &source_entities[0], &target_entities[0]).await;
    }

    // Multi-entity mode: load all entity pairs and qualify names
    let mut all_mappings = HashMap::new();

    for source_entity in source_entities {
        for target_entity in target_entities {
            let rows: Vec<(String, String)> = sqlx::query_as(
                "SELECT source_field, target_field FROM field_mappings
                 WHERE source_entity = ? AND target_entity = ?",
            )
            .bind(source_entity)
            .bind(target_entity)
            .fetch_all(pool)
            .await
            .context("Failed to get field mappings for multi-entity comparison")?;

            for (source_field, target_field) in rows {
                // Qualify with entity name for multi-entity mode
                let qualified_source = format!("{}.{}", source_entity, source_field);
                let qualified_target = format!("{}.{}", target_entity, target_field);

                all_mappings.entry(qualified_source)
                    .or_insert_with(Vec::new)
                    .push(qualified_target);
            }
        }
    }

    Ok(all_mappings)
}

/// Set a field mapping using qualified names (entity.field format)
/// Parses the qualified names and delegates to the existing set_field_mapping function
pub async fn set_field_mapping_qualified(
    pool: &SqlitePool,
    qualified_source: &str,  // e.g. "contact.fullname"
    qualified_target: &str,  // e.g. "lead.firstname"
) -> Result<()> {
    let (source_entity, source_field) = parse_qualified(qualified_source)
        .with_context(|| format!("Failed to parse source field: {}", qualified_source))?;
    let (target_entity, target_field) = parse_qualified(qualified_target)
        .with_context(|| format!("Failed to parse target field: {}", qualified_target))?;

    set_field_mapping(pool, &source_entity, &target_entity, &source_field, &target_field).await
}

/// Delete a field mapping using qualified names (entity.field format)
/// Parses the qualified names and delegates to the existing delete_field_mapping function
pub async fn delete_field_mapping_qualified(
    pool: &SqlitePool,
    qualified_source: &str,  // e.g. "contact.fullname"
) -> Result<()> {
    let (source_entity, source_field) = parse_qualified(qualified_source)
        .with_context(|| format!("Failed to parse source field: {}", qualified_source))?;

    // For now, we need a target entity to delete. In multi-entity mode, we'd need to know which
    // target entity to delete from. This may need refinement based on actual usage.
    // For now, let's assume we want to delete all mappings for this source field across all targets.
    // This is a simplification - in practice, we might need additional context.
    anyhow::bail!("delete_field_mapping_qualified requires target entity context. Use delete_specific_field_mapping_qualified instead.");
}

/// Delete a specific field mapping using qualified names
pub async fn delete_specific_field_mapping_qualified(
    pool: &SqlitePool,
    qualified_source: &str,  // e.g. "contact.fullname"
    qualified_target: &str,  // e.g. "lead.firstname"
) -> Result<()> {
    let (source_entity, source_field) = parse_qualified(qualified_source)
        .with_context(|| format!("Failed to parse source field: {}", qualified_source))?;
    let (target_entity, target_field) = parse_qualified(qualified_target)
        .with_context(|| format!("Failed to parse target field: {}", qualified_target))?;

    delete_specific_field_mapping(pool, &source_entity, &target_entity, &source_field, &target_field).await
}
