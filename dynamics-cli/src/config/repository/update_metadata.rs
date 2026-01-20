//! Repository for update metadata operations
//!
//! Stores key-value metadata related to updates, such as:
//! - last_check_timestamp: When we last checked for updates

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

/// Get a string value from update metadata
pub async fn get_string(pool: &SqlitePool, key: &str) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM update_metadata WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .with_context(|| format!("Failed to get update metadata for key '{}'", key))?;

    Ok(row.map(|(value,)| value))
}

/// Set a string value in update metadata
pub async fn set_string(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    sqlx::query("INSERT OR REPLACE INTO update_metadata (key, value) VALUES (?, ?)")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await
        .with_context(|| format!("Failed to set update metadata for key '{}'", key))?;

    log::debug!("Set update metadata: {} = {}", key, value);
    Ok(())
}

/// Delete a value from update metadata
pub async fn delete(pool: &SqlitePool, key: &str) -> Result<()> {
    let result = sqlx::query("DELETE FROM update_metadata WHERE key = ?")
        .bind(key)
        .execute(pool)
        .await
        .with_context(|| format!("Failed to delete update metadata for key '{}'", key))?;

    if result.rows_affected() > 0 {
        log::debug!("Deleted update metadata: {}", key);
    }

    Ok(())
}

/// Get the timestamp of the last update check
pub async fn get_last_check_time(pool: &SqlitePool) -> Result<Option<DateTime<Utc>>> {
    let value = get_string(pool, "last_check_timestamp").await?;

    if let Some(timestamp_str) = value {
        let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
            .context("Failed to parse last check timestamp")?
            .with_timezone(&Utc);
        Ok(Some(timestamp))
    } else {
        Ok(None)
    }
}

/// Set the timestamp of the last update check to now
pub async fn set_last_check_time(pool: &SqlitePool, time: DateTime<Utc>) -> Result<()> {
    let timestamp_str = time.to_rfc3339();
    set_string(pool, "last_check_timestamp", &timestamp_str).await
}

/// Check if we should check for updates (based on last check time)
/// Returns true if we should check (no last check, or > 24 hours ago)
pub async fn should_check_for_updates(pool: &SqlitePool) -> Result<bool> {
    let last_check = get_last_check_time(pool).await?;

    if let Some(last_check) = last_check {
        let now = Utc::now();
        let duration = now.signed_duration_since(last_check);
        // Check if more than 24 hours have passed
        Ok(duration.num_hours() >= 24)
    } else {
        // Never checked before
        Ok(true)
    }
}

/// Clear all update metadata
pub async fn clear_all(pool: &SqlitePool) -> Result<()> {
    sqlx::query("DELETE FROM update_metadata")
        .execute(pool)
        .await
        .context("Failed to clear update metadata")?;

    log::debug!("Cleared all update metadata");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::db;

    #[tokio::test]
    async fn test_string_operations() {
        let pool = db::connect_memory().await.unwrap();
        db::run_migrations(&pool).await.unwrap();

        // Test set and get
        set_string(&pool, "test_key", "test_value").await.unwrap();
        let value = get_string(&pool, "test_key").await.unwrap();
        assert_eq!(value, Some("test_value".to_string()));

        // Test update
        set_string(&pool, "test_key", "new_value").await.unwrap();
        let value = get_string(&pool, "test_key").await.unwrap();
        assert_eq!(value, Some("new_value".to_string()));

        // Test delete
        delete(&pool, "test_key").await.unwrap();
        let value = get_string(&pool, "test_key").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_last_check_time() {
        let pool = db::connect_memory().await.unwrap();
        db::run_migrations(&pool).await.unwrap();

        // Initially should be None
        let last_check = get_last_check_time(&pool).await.unwrap();
        assert_eq!(last_check, None);

        // Should check since no last check
        let should_check = should_check_for_updates(&pool).await.unwrap();
        assert!(should_check);

        // Set last check time to now
        let now = Utc::now();
        set_last_check_time(&pool, now).await.unwrap();

        // Should not check (< 24 hours)
        let should_check = should_check_for_updates(&pool).await.unwrap();
        assert!(!should_check);

        // Set last check time to 25 hours ago
        let old_time = now - chrono::Duration::hours(25);
        set_last_check_time(&pool, old_time).await.unwrap();

        // Should check (> 24 hours)
        let should_check = should_check_for_updates(&pool).await.unwrap();
        assert!(should_check);
    }
}
