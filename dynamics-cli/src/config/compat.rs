//! TOML compatibility layer for export/import

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::Path;

/// Export SQLite database to TOML format
pub async fn export_to_toml(pool: &SqlitePool, path: &Path) -> Result<()> {
    log::info!("Exporting database to TOML: {:?}", path);

    let mut config = ExportConfig::default();

    // Export current environment
    if let Some(current_env) = get_current_environment(pool).await? {
        config.current_environment = Some(current_env);
    }

    // Export environments and credentials
    let environments = get_all_environments_with_credentials(pool).await?;
    for (env_name, host, username, password, client_id, client_secret) in environments {
        config.environments.insert(
            env_name,
            ExportAuthConfig {
                host,
                username,
                password,
                client_id,
                client_secret,
            },
        );
    }

    // Export entity mappings
    config.entity_mappings = get_all_entity_mappings(pool).await?;

    // Export settings
    config.settings.default_query_limit = get_default_query_limit(pool).await?;
    config.settings.field_mappings = get_all_field_mappings(pool).await?;
    config.settings.prefix_mappings = get_all_prefix_mappings(pool).await?;
    config.settings.examples = get_all_examples(pool).await?;

    // Export migrations
    config.migrations = get_all_migrations(pool).await?;

    // Serialize to TOML
    let toml_content =
        toml::to_string_pretty(&config).context("Failed to serialize config to TOML")?;

    std::fs::write(path, toml_content)
        .with_context(|| format!("Failed to write TOML file: {:?}", path))?;

    log::info!("Successfully exported database to TOML");
    Ok(())
}

/// Import TOML format into SQLite database (for restore)
pub async fn import_from_toml(pool: &SqlitePool, path: &Path) -> Result<()> {
    log::info!("Importing TOML to database: {:?}", path);

    let toml_content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read TOML file: {:?}", path))?;

    let import_config: ExportConfig = toml::from_str(&toml_content)
        .with_context(|| format!("Failed to parse TOML file: {:?}", path))?;

    // Clear existing data (be careful!)
    clear_database(pool).await?;

    // Import using manual insertion (similar to migration logic)
    import_config_data(pool, import_config).await?;

    log::info!("Successfully imported TOML to database");
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ExportConfig {
    pub current_environment: Option<String>,
    pub environments: HashMap<String, ExportAuthConfig>,
    #[serde(default)]
    pub entity_mappings: HashMap<String, String>,
    #[serde(default)]
    pub settings: ExportSettings,
    #[serde(default)]
    pub migrations: HashMap<String, ExportMigration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportAuthConfig {
    pub host: String,
    pub username: String,
    pub password: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ExportSettings {
    #[serde(default = "default_query_limit")]
    pub default_query_limit: u32,
    #[serde(default)]
    pub field_mappings: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub prefix_mappings: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub examples: HashMap<String, Vec<ExportExamplePair>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportMigration {
    pub name: String,
    pub source_env: String,
    pub target_env: String,
    pub comparisons: Vec<ExportComparison>,
    pub created_at: String,
    pub last_used: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportComparison {
    pub name: String,
    pub source_entity: String,
    pub target_entity: String,
    pub entity_comparison: ExportEntityComparison,
    pub view_comparisons: Vec<ExportViewComparison>,
    pub created_at: String,
    pub last_used: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ExportEntityComparison {
    #[serde(default)]
    pub field_mappings: HashMap<String, String>,
    #[serde(default)]
    pub prefix_mappings: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportViewComparison {
    pub source_view_name: String,
    pub target_view_name: String,
    #[serde(default)]
    pub column_mappings: HashMap<String, String>,
    #[serde(default)]
    pub filter_mappings: HashMap<String, String>,
    #[serde(default)]
    pub sort_mappings: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportExamplePair {
    pub id: String,
    pub source_uuid: String,
    pub target_uuid: String,
    pub label: Option<String>,
}

fn default_query_limit() -> u32 {
    100
}

// Helper functions for database queries

async fn get_current_environment(pool: &SqlitePool) -> Result<Option<String>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT name FROM environments WHERE is_current = TRUE LIMIT 1")
            .fetch_optional(pool)
            .await
            .context("Failed to get current environment")?;

    Ok(row.map(|(name,)| name))
}

async fn get_all_environments_with_credentials(
    pool: &SqlitePool,
) -> Result<Vec<(String, String, String, String, String, String)>> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT e.name, e.host, c.data
        FROM environments e
        JOIN credentials c ON e.credentials_ref = c.name
        WHERE c.type = 'username_password'
        ORDER BY e.name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to get environments with credentials")?;

    let mut result = Vec::new();
    for (env_name, host, credential_data) in rows {
        let cred_data: crate::config::models::CredentialData =
            serde_json::from_str(&credential_data).context("Failed to parse credential data")?;

        if let crate::config::models::CredentialData::UsernamePassword {
            username,
            password,
            client_id,
            client_secret,
        } = cred_data
        {
            result.push((env_name, host, username, password, client_id, client_secret));
        }
    }

    Ok(result)
}

async fn get_all_entity_mappings(pool: &SqlitePool) -> Result<HashMap<String, String>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT singular_name, plural_name FROM entity_mappings ORDER BY singular_name",
    )
    .fetch_all(pool)
    .await
    .context("Failed to get entity mappings")?;

    Ok(rows.into_iter().collect())
}

async fn get_default_query_limit(pool: &SqlitePool) -> Result<u32> {
    let value: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = 'default_query_limit'")
            .fetch_optional(pool)
            .await
            .context("Failed to get default query limit")?;

    if let Some((value_str,)) = value {
        value_str
            .parse()
            .context("Invalid default query limit value")
    } else {
        Ok(100) // default
    }
}

async fn get_all_field_mappings(
    pool: &SqlitePool,
) -> Result<HashMap<String, HashMap<String, String>>> {
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT source_entity, target_entity, source_field, target_field FROM field_mappings ORDER BY source_entity, target_entity",
    )
    .fetch_all(pool)
    .await
    .context("Failed to get field mappings")?;

    let mut result: HashMap<String, HashMap<String, String>> = HashMap::new();
    for (source_entity, target_entity, source_field, target_field) in rows {
        let key = format!("{}:{}", source_entity, target_entity);
        result
            .entry(key)
            .or_default()
            .insert(source_field, target_field);
    }

    Ok(result)
}

async fn get_all_prefix_mappings(
    pool: &SqlitePool,
) -> Result<HashMap<String, HashMap<String, String>>> {
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT source_entity, target_entity, source_prefix, target_prefix FROM prefix_mappings ORDER BY source_entity, target_entity",
    )
    .fetch_all(pool)
    .await
    .context("Failed to get prefix mappings")?;

    let mut result: HashMap<String, HashMap<String, String>> = HashMap::new();
    for (source_entity, target_entity, source_prefix, target_prefix) in rows {
        let key = format!("{}:{}", source_entity, target_entity);
        result
            .entry(key)
            .or_default()
            .insert(source_prefix, target_prefix);
    }

    Ok(result)
}

async fn get_all_examples(pool: &SqlitePool) -> Result<HashMap<String, Vec<ExportExamplePair>>> {
    let rows: Vec<(String, String, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT source_entity, target_entity, id, source_uuid, target_uuid, label FROM example_pairs ORDER BY source_entity, target_entity",
    )
    .fetch_all(pool)
    .await
    .context("Failed to get example pairs")?;

    let mut result: HashMap<String, Vec<ExportExamplePair>> = HashMap::new();
    for (source_entity, target_entity, id, source_uuid, target_uuid, label) in rows {
        let key = format!("{}:{}", source_entity, target_entity);
        result.entry(key).or_default().push(ExportExamplePair {
            id,
            source_uuid,
            target_uuid,
            label,
        });
    }

    Ok(result)
}

async fn get_all_migrations(pool: &SqlitePool) -> Result<HashMap<String, ExportMigration>> {
    let migration_rows: Vec<(
        String,
        String,
        String,
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT name, source_env, target_env, created_at, last_used FROM migrations ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .context("Failed to get migrations")?;

    let mut result = HashMap::new();

    for (name, source_env, target_env, created_at, last_used) in migration_rows {
        // Get comparisons for this migration
        let comparison_rows: Vec<(i64, String, String, String, Option<String>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
            "SELECT id, name, source_entity, target_entity, entity_comparison, created_at, last_used FROM comparisons WHERE migration_name = ? ORDER BY name",
        )
        .bind(&name)
        .fetch_all(pool)
        .await
        .context("Failed to get comparisons")?;

        let mut comparisons = Vec::new();
        for (
            comp_id,
            comp_name,
            source_entity,
            target_entity,
            entity_comparison,
            comp_created_at,
            comp_last_used,
        ) in comparison_rows
        {
            let entity_comp: ExportEntityComparison = if let Some(json_str) = entity_comparison {
                serde_json::from_str(&json_str).unwrap_or_default()
            } else {
                ExportEntityComparison::default()
            };

            // Get view comparisons
            let view_rows: Vec<(String, String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
                "SELECT source_view_name, target_view_name, column_mappings, filter_mappings, sort_mappings FROM view_comparisons WHERE comparison_id = ?",
            )
            .bind(comp_id)
            .fetch_all(pool)
            .await
            .context("Failed to get view comparisons")?;

            let mut view_comparisons = Vec::new();
            for (source_view, target_view, col_map_json, filter_map_json, sort_map_json) in
                view_rows
            {
                let column_mappings = col_map_json
                    .map(|json| serde_json::from_str(&json).unwrap_or_default())
                    .unwrap_or_default();
                let filter_mappings = filter_map_json
                    .map(|json| serde_json::from_str(&json).unwrap_or_default())
                    .unwrap_or_default();
                let sort_mappings = sort_map_json
                    .map(|json| serde_json::from_str(&json).unwrap_or_default())
                    .unwrap_or_default();

                view_comparisons.push(ExportViewComparison {
                    source_view_name: source_view,
                    target_view_name: target_view,
                    column_mappings,
                    filter_mappings,
                    sort_mappings,
                });
            }

            comparisons.push(ExportComparison {
                name: comp_name,
                source_entity,
                target_entity,
                entity_comparison: entity_comp,
                view_comparisons,
                created_at: comp_created_at.to_rfc3339(),
                last_used: comp_last_used.to_rfc3339(),
            });
        }

        result.insert(
            name.clone(),
            ExportMigration {
                name,
                source_env,
                target_env,
                comparisons,
                created_at: created_at.to_rfc3339(),
                last_used: last_used.to_rfc3339(),
            },
        );
    }

    Ok(result)
}

async fn clear_database(pool: &SqlitePool) -> Result<()> {
    let mut tx = pool.begin().await.context("Failed to start transaction")?;

    // Clear in correct order to respect foreign keys
    sqlx::query("DELETE FROM view_comparisons")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM comparisons")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM migrations")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM example_pairs")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM prefix_mappings")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM field_mappings")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM settings")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM entity_mappings")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM tokens").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM environments")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM credentials")
        .execute(&mut *tx)
        .await?;

    tx.commit()
        .await
        .context("Failed to commit clear transaction")?;

    log::info!("Cleared database for import");
    Ok(())
}

async fn import_config_data(pool: &SqlitePool, config: ExportConfig) -> Result<()> {
    let mut tx = pool
        .begin()
        .await
        .context("Failed to start import transaction")?;

    // Import credentials and environments
    for (env_name, auth_config) in &config.environments {
        // Create credential
        let credential_data = crate::config::models::CredentialData::UsernamePassword {
            username: auth_config.username.clone(),
            password: auth_config.password.clone(),
            client_id: auth_config.client_id.clone(),
            client_secret: auth_config.client_secret.clone(),
        };

        let data_json = serde_json::to_string(&credential_data)
            .context("Failed to serialize credential data")?;

        let credential_name = format!("{}_creds", env_name);

        sqlx::query("INSERT INTO credentials (name, type, data) VALUES (?, ?, ?)")
            .bind(&credential_name)
            .bind("username_password")
            .bind(&data_json)
            .execute(&mut *tx)
            .await
            .context("Failed to insert credential")?;

        // Create environment
        let is_current = config.current_environment.as_ref() == Some(env_name);

        sqlx::query("INSERT INTO environments (name, host, credentials_ref, is_current) VALUES (?, ?, ?, ?)")
            .bind(env_name)
            .bind(&auth_config.host)
            .bind(&credential_name)
            .bind(is_current)
            .execute(&mut *tx)
            .await
            .context("Failed to insert environment")?;
    }

    // Import entity mappings
    for (singular, plural) in &config.entity_mappings {
        sqlx::query("INSERT INTO entity_mappings (singular_name, plural_name) VALUES (?, ?)")
            .bind(singular)
            .bind(plural)
            .execute(&mut *tx)
            .await
            .context("Failed to insert entity mapping")?;
    }

    // Import settings
    sqlx::query("INSERT INTO settings (key, value, type) VALUES (?, ?, ?)")
        .bind("default_query_limit")
        .bind(config.settings.default_query_limit.to_string())
        .bind("integer")
        .execute(&mut *tx)
        .await
        .context("Failed to insert default_query_limit setting")?;

    tx.commit()
        .await
        .context("Failed to commit import transaction")?;
    Ok(())
}
