//! Example pairs repository

use anyhow::{Context, Result};
use sqlx::SqlitePool;

/// Get all example pairs for a source/target entity pair
pub async fn get_example_pairs(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
) -> Result<Vec<crate::tui::apps::migration::entity_comparison::ExamplePair>> {
    let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, source_uuid, target_uuid, label FROM example_pairs
         WHERE source_entity = ? AND target_entity = ?
         ORDER BY created_at DESC",
    )
    .bind(source_entity)
    .bind(target_entity)
    .fetch_all(pool)
    .await
    .context("Failed to get example pairs")?;

    Ok(rows
        .into_iter()
        .map(|(id, source_uuid, target_uuid, label)| {
            let mut pair = crate::tui::apps::migration::entity_comparison::ExamplePair::new(
                source_uuid,
                target_uuid,
            );
            pair.id = id;
            pair.label = label;
            pair
        })
        .collect())
}

/// Save an example pair (insert or update)
pub async fn save_example_pair(
    pool: &SqlitePool,
    source_entity: &str,
    target_entity: &str,
    pair: &crate::tui::apps::migration::entity_comparison::ExamplePair,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO example_pairs (id, source_entity, target_entity, source_uuid, target_uuid, label)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(id)
         DO UPDATE SET source_uuid = excluded.source_uuid, target_uuid = excluded.target_uuid, label = excluded.label",
    )
    .bind(&pair.id)
    .bind(source_entity)
    .bind(target_entity)
    .bind(&pair.source_record_id)
    .bind(&pair.target_record_id)
    .bind(&pair.label)
    .execute(pool)
    .await
    .context("Failed to save example pair")?;

    Ok(())
}

/// Delete an example pair
pub async fn delete_example_pair(pool: &SqlitePool, pair_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM example_pairs WHERE id = ?")
        .bind(pair_id)
        .execute(pool)
        .await
        .context("Failed to delete example pair")?;

    Ok(())
}
