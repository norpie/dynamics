//! Repository for token operations

use crate::api::models::TokenInfo;
use crate::config::models::{DbToken, chrono_to_system_time, system_time_to_chrono};
use anyhow::{Context, Result};
use sqlx::SqlitePool;

/// Save or update token for environment
pub async fn save(pool: &SqlitePool, env_name: String, token: TokenInfo) -> Result<()> {
    let expires_at = system_time_to_chrono(token.expires_at);

    sqlx::query(
        r#"
        INSERT OR REPLACE INTO tokens (environment_name, access_token, expires_at, refresh_token, updated_at)
        VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(&env_name)
    .bind(&token.access_token)
    .bind(expires_at)
    .bind(&token.refresh_token)
    .execute(pool)
    .await
    .with_context(|| format!("Failed to save token for environment '{}'", env_name))?;

    log::debug!("Saved token for environment: {}", env_name);
    Ok(())
}

/// Get token for environment
pub async fn get(pool: &SqlitePool, env_name: &str) -> Result<Option<TokenInfo>> {
    let row: Option<DbToken> = sqlx::query_as(
        "SELECT environment_name, access_token, expires_at, refresh_token, updated_at FROM tokens WHERE environment_name = ?",
    )
    .bind(env_name)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("Failed to get token for environment '{}'", env_name))?;

    if let Some(row) = row {
        let expires_at = chrono_to_system_time(row.expires_at);
        Ok(Some(TokenInfo {
            access_token: row.access_token,
            expires_at,
            refresh_token: row.refresh_token,
        }))
    } else {
        Ok(None)
    }
}

/// Delete token for environment
pub async fn delete(pool: &SqlitePool, env_name: &str) -> Result<()> {
    let result = sqlx::query("DELETE FROM tokens WHERE environment_name = ?")
        .bind(env_name)
        .execute(pool)
        .await
        .with_context(|| format!("Failed to delete token for environment '{}'", env_name))?;

    if result.rows_affected() > 0 {
        log::debug!("Deleted token for environment: {}", env_name);
    }

    Ok(())
}

/// List all environments with tokens
pub async fn list_environments_with_tokens(pool: &SqlitePool) -> Result<Vec<String>> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT environment_name FROM tokens ORDER BY environment_name")
            .fetch_all(pool)
            .await
            .context("Failed to list environments with tokens")?;

    Ok(rows.into_iter().map(|(name,)| name).collect())
}

/// Check if token exists and is not expired
pub async fn is_valid(pool: &SqlitePool, env_name: &str) -> Result<bool> {
    let row: Option<(chrono::DateTime<chrono::Utc>,)> =
        sqlx::query_as("SELECT expires_at FROM tokens WHERE environment_name = ?")
            .bind(env_name)
            .fetch_optional(pool)
            .await
            .with_context(|| {
                format!(
                    "Failed to check token validity for environment '{}'",
                    env_name
                )
            })?;

    if let Some((expires_at,)) = row {
        Ok(expires_at > chrono::Utc::now())
    } else {
        Ok(false)
    }
}

/// Clean up expired tokens
pub async fn cleanup_expired(pool: &SqlitePool) -> Result<u64> {
    let result = sqlx::query("DELETE FROM tokens WHERE expires_at <= CURRENT_TIMESTAMP")
        .execute(pool)
        .await
        .context("Failed to cleanup expired tokens")?;

    let deleted_count = result.rows_affected();
    if deleted_count > 0 {
        log::info!("Cleaned up {} expired tokens", deleted_count);
    }

    Ok(deleted_count)
}

/// Get token expiration info
pub async fn get_expiration(
    pool: &SqlitePool,
    env_name: &str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    let row: Option<(chrono::DateTime<chrono::Utc>,)> =
        sqlx::query_as("SELECT expires_at FROM tokens WHERE environment_name = ?")
            .bind(env_name)
            .fetch_optional(pool)
            .await
            .with_context(|| {
                format!(
                    "Failed to get token expiration for environment '{}'",
                    env_name
                )
            })?;

    Ok(row.map(|(expires_at,)| expires_at))
}

/// Update token expiration
pub async fn update_expiration(
    pool: &SqlitePool,
    env_name: &str,
    new_expires_at: std::time::SystemTime,
) -> Result<()> {
    let expires_at = system_time_to_chrono(new_expires_at);

    let result = sqlx::query(
        "UPDATE tokens SET expires_at = ?, updated_at = CURRENT_TIMESTAMP WHERE environment_name = ?",
    )
    .bind(expires_at)
    .bind(env_name)
    .execute(pool)
    .await
    .with_context(|| format!("Failed to update token expiration for environment '{}'", env_name))?;

    if result.rows_affected() == 0 {
        anyhow::bail!("No token found for environment '{}'", env_name);
    }

    log::debug!("Updated token expiration for environment: {}", env_name);
    Ok(())
}

/// Refresh token if possible
pub async fn update_access_token(
    pool: &SqlitePool,
    env_name: &str,
    new_access_token: String,
    new_expires_at: std::time::SystemTime,
    new_refresh_token: Option<String>,
) -> Result<()> {
    let expires_at = system_time_to_chrono(new_expires_at);

    let result = sqlx::query(
        r#"
        UPDATE tokens
        SET access_token = ?, expires_at = ?, refresh_token = ?, updated_at = CURRENT_TIMESTAMP
        WHERE environment_name = ?
        "#,
    )
    .bind(&new_access_token)
    .bind(expires_at)
    .bind(&new_refresh_token)
    .bind(env_name)
    .execute(pool)
    .await
    .with_context(|| {
        format!(
            "Failed to update access token for environment '{}'",
            env_name
        )
    })?;

    if result.rows_affected() == 0 {
        anyhow::bail!("No token found for environment '{}'", env_name);
    }

    log::debug!("Updated access token for environment: {}", env_name);
    Ok(())
}
