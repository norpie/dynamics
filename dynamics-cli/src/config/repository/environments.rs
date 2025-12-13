//! Repository for environment operations

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use crate::api::models::Environment as ApiEnvironment;
use crate::config::models::DbEnvironment;

/// Insert or update environment
pub async fn insert(pool: &SqlitePool, environment: ApiEnvironment) -> Result<()> {
    // Check if credentials exist
    let creds_exist: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM credentials WHERE name = ?",
    )
    .bind(&environment.credentials_ref)
    .fetch_one(pool)
    .await
    .context("Failed to check if credentials exist")?;

    if creds_exist == 0 {
        anyhow::bail!(
            "Credentials '{}' not found. Create credentials first.",
            environment.credentials_ref
        );
    }

    // Use ON CONFLICT DO UPDATE instead of INSERT OR REPLACE
    // INSERT OR REPLACE does DELETE + INSERT, which triggers ON DELETE CASCADE
    // and wipes out transfer_configs referencing this environment
    sqlx::query(
        r#"
        INSERT INTO environments (name, host, credentials_ref, updated_at)
        VALUES (?, ?, ?, CURRENT_TIMESTAMP)
        ON CONFLICT(name) DO UPDATE SET
            host = excluded.host,
            credentials_ref = excluded.credentials_ref,
            updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(&environment.name)
    .bind(&environment.host)
    .bind(&environment.credentials_ref)
    .execute(pool)
    .await
    .with_context(|| format!("Failed to insert environment '{}'", environment.name))?;

    log::info!("Saved environment: {}", environment.name);
    Ok(())
}

/// Get environment by name
pub async fn get(pool: &SqlitePool, name: &str) -> Result<Option<ApiEnvironment>> {
    let row: Option<DbEnvironment> = sqlx::query_as(
        "SELECT name, host, credentials_ref, is_current, created_at, updated_at FROM environments WHERE name = ?",
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("Failed to get environment '{}'", name))?;

    if let Some(row) = row {
        Ok(Some(ApiEnvironment {
            name: row.name,
            host: row.host,
            credentials_ref: row.credentials_ref,
        }))
    } else {
        Ok(None)
    }
}

/// List all environment names
pub async fn list(pool: &SqlitePool) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM environments ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .context("Failed to list environments")?;

    Ok(rows.into_iter().map(|(name,)| name).collect())
}

/// Delete environment by name
pub async fn delete(pool: &SqlitePool, name: &str) -> Result<()> {
    let mut tx = pool.begin().await.context("Failed to start transaction")?;

    // Delete associated tokens first
    sqlx::query("DELETE FROM tokens WHERE environment_name = ?")
        .bind(name)
        .execute(&mut *tx)
        .await
        .context("Failed to delete associated tokens")?;

    // Delete environment
    let result = sqlx::query("DELETE FROM environments WHERE name = ?")
        .bind(name)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("Failed to delete environment '{}'", name))?;

    if result.rows_affected() == 0 {
        anyhow::bail!("Environment '{}' not found", name);
    }

    tx.commit().await.context("Failed to commit transaction")?;

    log::info!("Deleted environment: {}", name);
    Ok(())
}

/// Rename environment
pub async fn rename(pool: &SqlitePool, old_name: &str, new_name: String) -> Result<()> {
    let mut tx = pool.begin().await.context("Failed to start transaction")?;

    // Check if old environment exists
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM environments WHERE name = ?")
        .bind(old_name)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to check if environment exists")?;

    if exists == 0 {
        anyhow::bail!("Environment '{}' not found", old_name);
    }

    // Check if new name already exists
    let new_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM environments WHERE name = ?")
        .bind(&new_name)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to check if new environment name exists")?;

    if new_exists > 0 {
        anyhow::bail!("Environment '{}' already exists", new_name);
    }

    // Update environment name
    sqlx::query("UPDATE environments SET name = ?, updated_at = CURRENT_TIMESTAMP WHERE name = ?")
        .bind(&new_name)
        .bind(old_name)
        .execute(&mut *tx)
        .await
        .context("Failed to update environment name")?;

    // Update token references
    sqlx::query("UPDATE tokens SET environment_name = ?, updated_at = CURRENT_TIMESTAMP WHERE environment_name = ?")
        .bind(&new_name)
        .bind(old_name)
        .execute(&mut *tx)
        .await
        .context("Failed to update token references")?;

    tx.commit().await.context("Failed to commit transaction")?;

    log::info!("Renamed environment: {} -> {}", old_name, new_name);
    Ok(())
}

/// Get current environment name
pub async fn get_current(pool: &SqlitePool) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM environments WHERE is_current = TRUE LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .context("Failed to get current environment")?;

    Ok(row.map(|(name,)| name))
}

/// Set current environment
pub async fn set_current(pool: &SqlitePool, name: String) -> Result<()> {
    let mut tx = pool.begin().await.context("Failed to start transaction")?;

    // Check if environment exists
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM environments WHERE name = ?")
        .bind(&name)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to check if environment exists")?;

    if exists == 0 {
        anyhow::bail!("Environment '{}' not found", name);
    }

    // Clear current flag from all environments
    sqlx::query("UPDATE environments SET is_current = FALSE WHERE is_current = TRUE")
        .execute(&mut *tx)
        .await
        .context("Failed to clear current environment flags")?;

    // Set current flag on specified environment
    sqlx::query("UPDATE environments SET is_current = TRUE, updated_at = CURRENT_TIMESTAMP WHERE name = ?")
        .bind(&name)
        .execute(&mut *tx)
        .await
        .context("Failed to set current environment")?;

    tx.commit().await.context("Failed to commit transaction")?;

    log::info!("Set current environment: {}", name);
    Ok(())
}

/// Get environments using specific credentials
pub async fn list_by_credentials(pool: &SqlitePool, credentials_ref: &str) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM environments WHERE credentials_ref = ? ORDER BY name",
    )
    .bind(credentials_ref)
    .fetch_all(pool)
    .await
    .with_context(|| format!("Failed to list environments using credentials '{}'", credentials_ref))?;

    Ok(rows.into_iter().map(|(name,)| name).collect())
}

/// Check if environment exists
pub async fn exists(pool: &SqlitePool, name: &str) -> Result<bool> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM environments WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .with_context(|| format!("Failed to check if environment '{}' exists", name))?;

    Ok(count > 0)
}

/// Get environment details with credentials info
pub async fn get_with_credentials_info(pool: &SqlitePool, name: &str) -> Result<Option<(ApiEnvironment, String)>> {
    let row: Option<(String, String, String, String)> = sqlx::query_as(
        r#"
        SELECT e.name, e.host, e.credentials_ref, c.type
        FROM environments e
        JOIN credentials c ON e.credentials_ref = c.name
        WHERE e.name = ?
        "#,
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("Failed to get environment '{}' with credentials info", name))?;

    if let Some((env_name, host, credentials_ref, cred_type)) = row {
        let environment = ApiEnvironment {
            name: env_name,
            host,
            credentials_ref,
        };
        Ok(Some((environment, cred_type)))
    } else {
        Ok(None)
    }
}