///! Repository for queue operations

use anyhow::{Context, Result};
use sqlx::{SqlitePool, Row};
use chrono::{DateTime, Utc};
use crate::tui::apps::queue::models::{QueueItem, OperationStatus, QueueFilter, SortMode};

/// Queue settings (singleton)
#[derive(Debug, Clone)]
pub struct QueueSettings {
    pub auto_play: bool,
    pub max_concurrent: usize,
    pub filter: QueueFilter,
    pub sort_mode: SortMode,
}

impl Default for QueueSettings {
    fn default() -> Self {
        Self {
            auto_play: false,
            max_concurrent: 3,
            filter: QueueFilter::All,
            sort_mode: SortMode::Priority,
        }
    }
}

/// Save or update a queue item
pub async fn save_queue_item(pool: &SqlitePool, item: &QueueItem) -> Result<()> {
    let operations_json = serde_json::to_string(&item.operations)
        .context("Failed to serialize operations")?;
    let metadata_json = serde_json::to_string(&item.metadata)
        .context("Failed to serialize metadata")?;
    let result_json = item.result.as_ref()
        .map(|r| serde_json::to_string(r).ok())
        .flatten();
    let succeeded_indices_json = serde_json::to_string(&item.succeeded_indices)
        .context("Failed to serialize succeeded_indices")?;

    sqlx::query(
        r#"
        INSERT INTO queue_items (
            id, environment_name, operations_json, metadata_json,
            status, priority, result_json, was_interrupted, interrupted_at,
            succeeded_indices_json, created_at, updated_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT(id) DO UPDATE SET
            status = excluded.status,
            priority = excluded.priority,
            result_json = excluded.result_json,
            was_interrupted = excluded.was_interrupted,
            interrupted_at = excluded.interrupted_at,
            succeeded_indices_json = excluded.succeeded_indices_json,
            updated_at = CURRENT_TIMESTAMP
        "#
    )
    .bind(&item.id)
    .bind(&item.metadata.environment_name)
    .bind(&operations_json)
    .bind(&metadata_json)
    .bind(status_to_string(&item.status))
    .bind(item.priority as i64)
    .bind(result_json)
    .bind(item.was_interrupted)
    .bind(item.interrupted_at)
    .bind(&succeeded_indices_json)
    .execute(pool)
    .await
    .with_context(|| format!("Failed to save queue item '{}'", item.id))?;

    Ok(())
}

/// Get a single queue item by ID
pub async fn get_queue_item(pool: &SqlitePool, id: &str) -> Result<Option<QueueItem>> {
    let row = sqlx::query(
        r#"
        SELECT id, environment_name, operations_json, metadata_json,
               status, priority, result_json, was_interrupted, interrupted_at,
               succeeded_indices_json
        FROM queue_items
        WHERE id = ?
        "#
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("Failed to get queue item '{}'", id))?;

    if let Some(row) = row {
        Ok(Some(parse_queue_item_row(row)?))
    } else {
        Ok(None)
    }
}

/// List all queue items
pub async fn list_queue_items(pool: &SqlitePool) -> Result<Vec<QueueItem>> {
    let rows = sqlx::query(
        r#"
        SELECT id, environment_name, operations_json, metadata_json,
               status, priority, result_json, was_interrupted, interrupted_at,
               succeeded_indices_json
        FROM queue_items
        ORDER BY priority ASC, created_at ASC
        "#
    )
    .fetch_all(pool)
    .await
    .context("Failed to list queue items")?;

    let mut items = Vec::new();
    for row in rows {
        items.push(parse_queue_item_row(row)?);
    }

    Ok(items)
}

/// Update queue item status
pub async fn update_queue_item_status(pool: &SqlitePool, id: &str, status: OperationStatus) -> Result<()> {
    let result = sqlx::query(
        "UPDATE queue_items SET status = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
    )
    .bind(status_to_string(&status))
    .bind(id)
    .execute(pool)
    .await
    .with_context(|| format!("Failed to update status for queue item '{}'", id))?;

    if result.rows_affected() == 0 {
        anyhow::bail!("Queue item '{}' not found", id);
    }

    Ok(())
}

/// Update queue item priority
pub async fn update_queue_item_priority(pool: &SqlitePool, id: &str, priority: u8) -> Result<()> {
    let result = sqlx::query(
        "UPDATE queue_items SET priority = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
    )
    .bind(priority as i64)
    .bind(id)
    .execute(pool)
    .await
    .with_context(|| format!("Failed to update priority for queue item '{}'", id))?;

    if result.rows_affected() == 0 {
        anyhow::bail!("Queue item '{}' not found", id);
    }

    Ok(())
}

/// Update queue item result
pub async fn update_queue_item_result(
    pool: &SqlitePool,
    id: &str,
    result: &crate::tui::apps::queue::models::QueueResult
) -> Result<()> {
    let result_json = serde_json::to_string(result)
        .context("Failed to serialize queue result")?;

    let query_result = sqlx::query(
        "UPDATE queue_items SET result_json = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
    )
    .bind(&result_json)
    .bind(id)
    .execute(pool)
    .await
    .with_context(|| format!("Failed to update result for queue item '{}'", id))?;

    if query_result.rows_affected() == 0 {
        anyhow::bail!("Queue item '{}' not found", id);
    }

    Ok(())
}

/// Update succeeded indices for a queue item (for partial retry support)
pub async fn update_queue_item_succeeded_indices(
    pool: &SqlitePool,
    id: &str,
    succeeded_indices: &[usize]
) -> Result<()> {
    let indices_json = serde_json::to_string(succeeded_indices)
        .context("Failed to serialize succeeded_indices")?;

    let query_result = sqlx::query(
        "UPDATE queue_items SET succeeded_indices_json = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
    )
    .bind(&indices_json)
    .bind(id)
    .execute(pool)
    .await
    .with_context(|| format!("Failed to update succeeded_indices for queue item '{}'", id))?;

    if query_result.rows_affected() == 0 {
        anyhow::bail!("Queue item '{}' not found", id);
    }

    Ok(())
}

/// Mark a queue item as interrupted
pub async fn mark_queue_item_interrupted(pool: &SqlitePool, id: &str, interrupted_at: DateTime<Utc>) -> Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE queue_items
        SET was_interrupted = TRUE,
            interrupted_at = ?,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ?
        "#
    )
    .bind(interrupted_at)
    .bind(id)
    .execute(pool)
    .await
    .with_context(|| format!("Failed to mark queue item '{}' as interrupted", id))?;

    if result.rows_affected() == 0 {
        anyhow::bail!("Queue item '{}' not found", id);
    }

    Ok(())
}

/// Clear the interruption flag for a queue item
pub async fn clear_interruption_flag(pool: &SqlitePool, id: &str) -> Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE queue_items
        SET was_interrupted = FALSE,
            interrupted_at = NULL,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ?
        "#
    )
    .bind(id)
    .execute(pool)
    .await
    .with_context(|| format!("Failed to clear interruption flag for queue item '{}'", id))?;

    if result.rows_affected() == 0 {
        anyhow::bail!("Queue item '{}' not found", id);
    }

    Ok(())
}

/// Delete a single queue item
pub async fn delete_queue_item(pool: &SqlitePool, id: &str) -> Result<()> {
    let result = sqlx::query("DELETE FROM queue_items WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .with_context(|| format!("Failed to delete queue item '{}'", id))?;

    if result.rows_affected() == 0 {
        anyhow::bail!("Queue item '{}' not found", id);
    }

    Ok(())
}

/// Clear all queue items
pub async fn clear_queue(pool: &SqlitePool) -> Result<()> {
    sqlx::query("DELETE FROM queue_items")
        .execute(pool)
        .await
        .context("Failed to clear queue")?;

    log::info!("Cleared all queue items");
    Ok(())
}

/// Get queue settings
pub async fn get_queue_settings(pool: &SqlitePool) -> Result<QueueSettings> {
    let row = sqlx::query(
        "SELECT auto_play, max_concurrent, filter, sort_mode FROM queue_settings WHERE id = 1"
    )
    .fetch_optional(pool)
    .await
    .context("Failed to get queue settings")?;

    if let Some(row) = row {
        let auto_play: bool = row.try_get("auto_play")?;
        let max_concurrent: i64 = row.try_get("max_concurrent")?;
        let filter_str: String = row.try_get("filter")?;
        let sort_mode_str: String = row.try_get("sort_mode")?;

        Ok(QueueSettings {
            auto_play,
            max_concurrent: max_concurrent as usize,
            filter: parse_filter(&filter_str),
            sort_mode: parse_sort_mode(&sort_mode_str),
        })
    } else {
        // Settings don't exist, return defaults
        Ok(QueueSettings::default())
    }
}

/// Save queue settings
pub async fn save_queue_settings(pool: &SqlitePool, settings: &QueueSettings) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO queue_settings (id, auto_play, max_concurrent, filter, sort_mode, updated_at)
        VALUES (1, ?, ?, ?, ?, CURRENT_TIMESTAMP)
        ON CONFLICT(id) DO UPDATE SET
            auto_play = excluded.auto_play,
            max_concurrent = excluded.max_concurrent,
            filter = excluded.filter,
            sort_mode = excluded.sort_mode,
            updated_at = CURRENT_TIMESTAMP
        "#
    )
    .bind(settings.auto_play)
    .bind(settings.max_concurrent as i64)
    .bind(filter_to_string(&settings.filter))
    .bind(sort_mode_to_string(&settings.sort_mode))
    .execute(pool)
    .await
    .context("Failed to save queue settings")?;

    Ok(())
}

// Helper functions

fn parse_queue_item_row(row: sqlx::sqlite::SqliteRow) -> Result<QueueItem> {
    use sqlx::Row;

    let id: String = row.try_get("id")?;
    let operations_json: String = row.try_get("operations_json")?;
    let metadata_json: String = row.try_get("metadata_json")?;
    let status_str: String = row.try_get("status")?;
    let priority: i64 = row.try_get("priority")?;
    let result_json: Option<String> = row.try_get("result_json")?;
    let was_interrupted: bool = row.try_get("was_interrupted")?;
    let interrupted_at: Option<DateTime<Utc>> = row.try_get("interrupted_at")?;
    let succeeded_indices_json: Option<String> = row.try_get("succeeded_indices_json").ok();

    let operations = serde_json::from_str(&operations_json)
        .with_context(|| format!("Failed to deserialize operations for queue item '{}'", id))?;
    let metadata = serde_json::from_str(&metadata_json)
        .with_context(|| format!("Failed to deserialize metadata for queue item '{}'", id))?;
    let status = parse_status(&status_str);
    let result = result_json
        .map(|json| serde_json::from_str(&json).ok())
        .flatten();
    let succeeded_indices: Vec<usize> = succeeded_indices_json
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();

    Ok(QueueItem {
        id,
        operations,
        metadata,
        status,
        priority: priority as u8,
        result,
        started_at: None, // Runtime state, not persisted
        was_interrupted,
        interrupted_at,
        succeeded_indices,
    })
}

fn status_to_string(status: &OperationStatus) -> &'static str {
    match status {
        OperationStatus::Pending => "Pending",
        OperationStatus::Running => "Running",
        OperationStatus::Paused => "Paused",
        OperationStatus::Done => "Done",
        OperationStatus::Failed => "Failed",
        OperationStatus::PartiallyFailed => "PartiallyFailed",
    }
}

fn parse_status(s: &str) -> OperationStatus {
    match s {
        "Pending" => OperationStatus::Pending,
        "Running" => OperationStatus::Running,
        "Paused" => OperationStatus::Paused,
        "Done" => OperationStatus::Done,
        "Failed" => OperationStatus::Failed,
        "PartiallyFailed" => OperationStatus::PartiallyFailed,
        _ => OperationStatus::Pending, // Default fallback
    }
}

fn filter_to_string(filter: &QueueFilter) -> &'static str {
    match filter {
        QueueFilter::All => "All",
        QueueFilter::Pending => "Pending",
        QueueFilter::Running => "Running",
        QueueFilter::Paused => "Paused",
        QueueFilter::Failed => "Failed",
    }
}

fn parse_filter(s: &str) -> QueueFilter {
    match s {
        "All" => QueueFilter::All,
        "Pending" => QueueFilter::Pending,
        "Running" => QueueFilter::Running,
        "Paused" => QueueFilter::Paused,
        "Failed" => QueueFilter::Failed,
        _ => QueueFilter::All, // Default fallback
    }
}

fn sort_mode_to_string(mode: &SortMode) -> &'static str {
    match mode {
        SortMode::Priority => "Priority",
        SortMode::Status => "Status",
        SortMode::Source => "Source",
    }
}

fn parse_sort_mode(s: &str) -> SortMode {
    match s {
        "Priority" => SortMode::Priority,
        "Status" => SortMode::Status,
        "Source" => SortMode::Source,
        _ => SortMode::Priority, // Default fallback
    }
}
