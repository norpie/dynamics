//! Repository for credential operations

use crate::api::models::CredentialSet as ApiCredentialSet;
use crate::config::models::{CredentialData, DbCredential};
use anyhow::{Context, Result};
use sqlx::SqlitePool;

/// Insert or update credentials
pub async fn insert(pool: &SqlitePool, name: String, credentials: ApiCredentialSet) -> Result<()> {
    let credential_data: CredentialData = credentials.into();
    let type_str = match credential_data {
        CredentialData::UsernamePassword { .. } => "username_password",
        CredentialData::ClientCredentials { .. } => "client_credentials",
        CredentialData::DeviceCode { .. } => "device_code",
        CredentialData::Certificate { .. } => "certificate",
    };

    let data_json =
        serde_json::to_string(&credential_data).context("Failed to serialize credential data")?;

    sqlx::query(
        r#"
        INSERT OR REPLACE INTO credentials (name, type, data, updated_at)
        VALUES (?, ?, ?, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(&name)
    .bind(type_str)
    .bind(&data_json)
    .execute(pool)
    .await
    .with_context(|| format!("Failed to insert credentials '{}'", name))?;

    log::info!("Saved credentials: {}", name);
    Ok(())
}

/// Get credentials by name
pub async fn get(pool: &SqlitePool, name: &str) -> Result<Option<ApiCredentialSet>> {
    let row: Option<DbCredential> = sqlx::query_as(
        "SELECT name, type, data, created_at, updated_at FROM credentials WHERE name = ?",
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("Failed to get credentials '{}'", name))?;

    if let Some(row) = row {
        let credential_data: CredentialData =
            serde_json::from_str(&row.data).context("Failed to deserialize credential data")?;
        Ok(Some(credential_data.into()))
    } else {
        Ok(None)
    }
}

/// List all credential names
pub async fn list(pool: &SqlitePool) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT name FROM credentials ORDER BY name")
        .fetch_all(pool)
        .await
        .context("Failed to list credentials")?;

    Ok(rows.into_iter().map(|(name,)| name).collect())
}

/// Delete credentials by name
pub async fn delete(pool: &SqlitePool, name: &str) -> Result<()> {
    let result = sqlx::query("DELETE FROM credentials WHERE name = ?")
        .bind(name)
        .execute(pool)
        .await
        .with_context(|| format!("Failed to delete credentials '{}'", name))?;

    if result.rows_affected() == 0 {
        anyhow::bail!("Credentials '{}' not found", name);
    }

    log::info!("Deleted credentials: {}", name);
    Ok(())
}

/// Rename credentials
pub async fn rename(pool: &SqlitePool, old_name: &str, new_name: String) -> Result<()> {
    let mut tx = pool.begin().await.context("Failed to start transaction")?;

    // Check if old credentials exist
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM credentials WHERE name = ?")
        .bind(old_name)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to check if credentials exist")?;

    if exists == 0 {
        anyhow::bail!("Credentials '{}' not found", old_name);
    }

    // Check if new name already exists
    let new_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM credentials WHERE name = ?")
        .bind(&new_name)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to check if new credentials name exists")?;

    if new_exists > 0 {
        anyhow::bail!("Credentials '{}' already exists", new_name);
    }

    // Update credentials name
    sqlx::query("UPDATE credentials SET name = ?, updated_at = CURRENT_TIMESTAMP WHERE name = ?")
        .bind(&new_name)
        .bind(old_name)
        .execute(&mut *tx)
        .await
        .context("Failed to update credentials name")?;

    // Update references in environments table
    sqlx::query("UPDATE environments SET credentials_ref = ?, updated_at = CURRENT_TIMESTAMP WHERE credentials_ref = ?")
        .bind(&new_name)
        .bind(old_name)
        .execute(&mut *tx)
        .await
        .context("Failed to update environment references")?;

    tx.commit().await.context("Failed to commit transaction")?;

    log::info!("Renamed credentials: {} -> {}", old_name, new_name);
    Ok(())
}

/// Get credentials by type
pub async fn list_by_type(pool: &SqlitePool, credential_type: &str) -> Result<Vec<String>> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM credentials WHERE type = ? ORDER BY name")
            .bind(credential_type)
            .fetch_all(pool)
            .await
            .with_context(|| format!("Failed to list credentials by type '{}'", credential_type))?;

    Ok(rows.into_iter().map(|(name,)| name).collect())
}

/// Check if credentials exist
pub async fn exists(pool: &SqlitePool, name: &str) -> Result<bool> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM credentials WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .with_context(|| format!("Failed to check if credentials '{}' exist", name))?;

    Ok(count > 0)
}
