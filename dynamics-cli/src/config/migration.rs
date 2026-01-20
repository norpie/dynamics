//! TOML to SQLite migration logic

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::api::models::{CredentialSet, Environment};
use crate::config::db;

/// Legacy TOML configuration structure for migration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct LegacyConfig {
    pub current_environment: Option<String>,
    pub environments: HashMap<String, LegacyAuthConfig>,
    #[serde(default)]
    pub entity_mappings: HashMap<String, String>,
    #[serde(default)]
    pub settings: LegacySettings,
    #[serde(default)]
    pub migrations: HashMap<String, LegacySavedMigration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyAuthConfig {
    pub host: String,
    pub username: String,
    pub password: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct LegacySettings {
    #[serde(default = "default_query_limit")]
    pub default_query_limit: u32,
    #[serde(default)]
    pub field_mappings: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub prefix_mappings: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub examples: HashMap<String, Vec<LegacyConfigExamplePair>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacySavedMigration {
    pub name: String,
    pub source_env: String,
    pub target_env: String,
    pub comparisons: Vec<LegacySavedComparison>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub last_used: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacySavedComparison {
    pub name: String,
    pub source_entity: String,
    pub target_entity: String,
    #[serde(default)]
    pub entity_comparison: LegacyEntityComparison,
    #[serde(default)]
    pub view_comparisons: Vec<LegacyViewComparison>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub last_used: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct LegacyEntityComparison {
    #[serde(default)]
    pub field_mappings: HashMap<String, String>,
    #[serde(default)]
    pub prefix_mappings: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyViewComparison {
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
struct LegacyConfigExamplePair {
    pub id: String,
    pub source_uuid: String,
    pub target_uuid: String,
    pub label: Option<String>,
}

fn default_query_limit() -> u32 {
    100
}

/// Migrate from TOML config to SQLite database
pub async fn migrate_from_toml(toml_path: &Path, db_path: &Path) -> Result<()> {
    log::info!("Starting migration from {:?} to {:?}", toml_path, db_path);

    // Read TOML config
    let toml_content = std::fs::read_to_string(toml_path)
        .with_context(|| format!("Failed to read TOML config: {:?}", toml_path))?;

    let legacy_config: LegacyConfig = toml::from_str(&toml_content)
        .with_context(|| format!("Failed to parse TOML config: {:?}", toml_path))?;

    log::info!(
        "Parsed legacy config with {} environments",
        legacy_config.environments.len()
    );

    // Connect to new SQLite database
    let pool = db::connect(db_path).await?;
    db::run_migrations(&pool).await?;

    // Start migration transaction
    let mut tx = pool
        .begin()
        .await
        .context("Failed to start migration transaction")?;

    // 1. Migrate credentials and environments
    let mut credential_name_map: HashMap<String, String> = HashMap::new();

    for (env_name, auth_config) in &legacy_config.environments {
        // Create a credential name (reuse if same credentials exist)
        let credential_key = format!(
            "{}:{}:{}:{}",
            auth_config.username,
            auth_config.client_id,
            auth_config.client_secret,
            auth_config.host
        );

        let credential_name = if let Some(existing_name) = credential_name_map.get(&credential_key)
        {
            existing_name.clone()
        } else {
            // Create new credential name
            let cred_name = format!("{}_creds", env_name);
            credential_name_map.insert(credential_key, cred_name.clone());

            // Insert credential
            let credential_data = crate::config::models::CredentialData::UsernamePassword {
                username: auth_config.username.clone(),
                password: auth_config.password.clone(),
                client_id: auth_config.client_id.clone(),
                client_secret: auth_config.client_secret.clone(),
            };

            let data_json = serde_json::to_string(&credential_data)
                .context("Failed to serialize credential data")?;

            sqlx::query("INSERT INTO credentials (name, type, data) VALUES (?, ?, ?)")
                .bind(&cred_name)
                .bind("username_password")
                .bind(&data_json)
                .execute(&mut *tx)
                .await
                .context("Failed to insert credential")?;

            log::debug!("Created credential: {}", cred_name);
            cred_name
        };

        // Insert environment
        let is_current = legacy_config.current_environment.as_ref() == Some(env_name);

        sqlx::query(
            "INSERT INTO environments (name, host, credentials_ref, is_current) VALUES (?, ?, ?, ?)",
        )
        .bind(env_name)
        .bind(&auth_config.host)
        .bind(&credential_name)
        .bind(is_current)
        .execute(&mut *tx)
        .await
        .context("Failed to insert environment")?;

        log::debug!("Created environment: {} -> {}", env_name, credential_name);
    }

    // 2. Migrate entity mappings
    for (singular, plural) in &legacy_config.entity_mappings {
        sqlx::query("INSERT INTO entity_mappings (singular_name, plural_name) VALUES (?, ?)")
            .bind(singular)
            .bind(plural)
            .execute(&mut *tx)
            .await
            .context("Failed to insert entity mapping")?;
    }
    log::info!(
        "Migrated {} entity mappings",
        legacy_config.entity_mappings.len()
    );

    // 3. Migrate settings
    let settings = &legacy_config.settings;

    // Default query limit
    sqlx::query("INSERT INTO settings (key, value, type) VALUES (?, ?, ?)")
        .bind("default_query_limit")
        .bind(settings.default_query_limit.to_string())
        .bind("integer")
        .execute(&mut *tx)
        .await
        .context("Failed to insert default_query_limit setting")?;

    // Field mappings
    for (entity_pair, mappings) in &settings.field_mappings {
        if let Some((source_entity, target_entity)) = entity_pair.split_once(':') {
            for (source_field, target_field) in mappings {
                sqlx::query(
                    "INSERT INTO field_mappings (source_entity, target_entity, source_field, target_field) VALUES (?, ?, ?, ?)",
                )
                .bind(source_entity)
                .bind(target_entity)
                .bind(source_field)
                .bind(target_field)
                .execute(&mut *tx)
                .await
                .context("Failed to insert field mapping")?;
            }
        }
    }
    log::info!(
        "Migrated field mappings for {} entity pairs",
        settings.field_mappings.len()
    );

    // Prefix mappings
    for (entity_pair, mappings) in &settings.prefix_mappings {
        if let Some((source_entity, target_entity)) = entity_pair.split_once(':') {
            for (source_prefix, target_prefix) in mappings {
                sqlx::query(
                    "INSERT INTO prefix_mappings (source_entity, target_entity, source_prefix, target_prefix) VALUES (?, ?, ?, ?)",
                )
                .bind(source_entity)
                .bind(target_entity)
                .bind(source_prefix)
                .bind(target_prefix)
                .execute(&mut *tx)
                .await
                .context("Failed to insert prefix mapping")?;
            }
        }
    }
    log::info!(
        "Migrated prefix mappings for {} entity pairs",
        settings.prefix_mappings.len()
    );

    // Example pairs
    for (entity_pair, examples) in &settings.examples {
        if let Some((source_entity, target_entity)) = entity_pair.split_once(':') {
            for example in examples {
                sqlx::query(
                    "INSERT INTO example_pairs (id, source_entity, target_entity, source_uuid, target_uuid, label) VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(&example.id)
                .bind(source_entity)
                .bind(target_entity)
                .bind(&example.source_uuid)
                .bind(&example.target_uuid)
                .bind(&example.label)
                .execute(&mut *tx)
                .await
                .context("Failed to insert example pair")?;
            }
        }
    }
    log::info!(
        "Migrated example pairs for {} entity pairs",
        settings.examples.len()
    );

    // 4. Migrate saved migrations
    for (migration_name, migration) in &legacy_config.migrations {
        // Parse timestamps if available
        let created_at = if migration.created_at.is_empty() {
            chrono::Utc::now()
        } else {
            chrono::DateTime::parse_from_rfc3339(&migration.created_at)
                .unwrap_or_else(|_| chrono::Utc::now().into())
                .with_timezone(&chrono::Utc)
        };

        let last_used = if migration.last_used.is_empty() {
            created_at
        } else {
            chrono::DateTime::parse_from_rfc3339(&migration.last_used)
                .unwrap_or_else(|_| created_at.into())
                .with_timezone(&chrono::Utc)
        };

        sqlx::query(
            "INSERT INTO migrations (name, source_env, target_env, created_at, last_used) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(migration_name)
        .bind(&migration.source_env)
        .bind(&migration.target_env)
        .bind(created_at)
        .bind(last_used)
        .execute(&mut *tx)
        .await
        .context("Failed to insert migration")?;

        // Migrate comparisons
        for comparison in &migration.comparisons {
            let comp_created_at = if comparison.created_at.is_empty() {
                created_at
            } else {
                chrono::DateTime::parse_from_rfc3339(&comparison.created_at)
                    .unwrap_or_else(|_| created_at.into())
                    .with_timezone(&chrono::Utc)
            };

            let comp_last_used = if comparison.last_used.is_empty() {
                comp_created_at
            } else {
                chrono::DateTime::parse_from_rfc3339(&comparison.last_used)
                    .unwrap_or_else(|_| comp_created_at.into())
                    .with_timezone(&chrono::Utc)
            };

            let entity_comparison_json = serde_json::to_string(&comparison.entity_comparison)
                .context("Failed to serialize entity comparison")?;

            let result = sqlx::query(
                "INSERT INTO comparisons (migration_name, name, source_entity, target_entity, entity_comparison, created_at, last_used) VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(migration_name)
            .bind(&comparison.name)
            .bind(&comparison.source_entity)
            .bind(&comparison.target_entity)
            .bind(&entity_comparison_json)
            .bind(comp_created_at)
            .bind(comp_last_used)
            .execute(&mut *tx)
            .await
            .context("Failed to insert comparison")?;

            let comparison_id = result.last_insert_rowid();

            // Migrate view comparisons
            for view_comp in &comparison.view_comparisons {
                let column_mappings = if view_comp.column_mappings.is_empty() {
                    None
                } else {
                    Some(
                        serde_json::to_string(&view_comp.column_mappings)
                            .context("Failed to serialize column mappings")?,
                    )
                };

                let filter_mappings = if view_comp.filter_mappings.is_empty() {
                    None
                } else {
                    Some(
                        serde_json::to_string(&view_comp.filter_mappings)
                            .context("Failed to serialize filter mappings")?,
                    )
                };

                let sort_mappings = if view_comp.sort_mappings.is_empty() {
                    None
                } else {
                    Some(
                        serde_json::to_string(&view_comp.sort_mappings)
                            .context("Failed to serialize sort mappings")?,
                    )
                };

                sqlx::query(
                    "INSERT INTO view_comparisons (comparison_id, source_view_name, target_view_name, column_mappings, filter_mappings, sort_mappings) VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(comparison_id)
                .bind(&view_comp.source_view_name)
                .bind(&view_comp.target_view_name)
                .bind(&column_mappings)
                .bind(&filter_mappings)
                .bind(&sort_mappings)
                .execute(&mut *tx)
                .await
                .context("Failed to insert view comparison")?;
            }
        }
    }
    log::info!(
        "Migrated {} saved migrations",
        legacy_config.migrations.len()
    );

    // Commit transaction
    tx.commit()
        .await
        .context("Failed to commit migration transaction")?;

    // Backup original TOML file
    let backup_path = toml_path.with_extension("toml.backup");
    std::fs::rename(toml_path, &backup_path)
        .with_context(|| format!("Failed to backup TOML config to {:?}", backup_path))?;

    log::info!(
        "Migration completed successfully. Original config backed up to {:?}",
        backup_path
    );
    log::info!("Migrated:");
    log::info!(
        "  - {} environments with credentials",
        legacy_config.environments.len()
    );
    log::info!(
        "  - {} entity mappings",
        legacy_config.entity_mappings.len()
    );
    log::info!(
        "  - {} field mapping pairs",
        legacy_config.settings.field_mappings.len()
    );
    log::info!(
        "  - {} prefix mapping pairs",
        legacy_config.settings.prefix_mappings.len()
    );
    log::info!(
        "  - {} example pairs",
        legacy_config
            .settings
            .examples
            .values()
            .map(|v| v.len())
            .sum::<usize>()
    );
    log::info!("  - {} saved migrations", legacy_config.migrations.len());

    Ok(())
}
