//! SQLite-based configuration module for Dynamics CLI
//!
//! This module provides persistent storage for:
//! - Authentication credentials (shared across environments)
//! - Environment configurations (host + credential reference)
//! - Token caching per environment
//! - Legacy configuration features (entity mappings, migrations, etc.)
//! - Options system for type-safe key-value settings

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;

pub mod db;
pub mod models;
pub mod repository;
pub mod migrations;
pub mod migration;
pub mod compat;
pub mod options;

pub use models::*;
pub use repository::migrations::{SavedMigration, SavedComparison};

use crate::api::models::{Environment as ApiEnvironment, CredentialSet as ApiCredentialSet};

/// Main configuration manager using SQLite backend
pub struct Config {
    pub(crate) pool: sqlx::SqlitePool,
    config_path: PathBuf,

    /// Options system for type-safe settings
    pub options: options::Options,
}

impl Config {
    /// Get the path to the SQLite database file
    pub fn get_db_path() -> Result<PathBuf> {
        let config_dir = if cfg!(target_os = "linux") {
            dirs::config_dir()
                .context("Failed to get XDG config directory")?
                .join("dynamics-cli")
        } else {
            dirs::home_dir()
                .context("Failed to get home directory")?
                .join(".dynamics-cli")
        };

        // Ensure the directory exists
        if !config_dir.exists() {
            std::fs::create_dir_all(&config_dir)
                .with_context(|| format!("Failed to create config directory: {:?}", config_dir))?;
            log::info!("Created config directory: {:?}", config_dir);
        }

        Ok(config_dir.join("config.db"))
    }

    /// Load configuration from SQLite database
    pub async fn load() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        log::debug!("Loading config from: {:?}", db_path);

        // Check if we need to migrate from TOML
        let toml_path = db_path.with_file_name("config.toml");
        let migrated = if !db_path.exists() && toml_path.exists() {
            log::info!("Migrating from TOML config to SQLite");
            migration::migrate_from_toml(&toml_path, &db_path).await?;
            true
        } else {
            false
        };

        // Connect to database
        let pool = db::connect(&db_path).await?;

        // Run migrations
        db::run_migrations(&pool).await?;

        // Initialize options with the global registry
        let options = options::Options::new(pool.clone(), crate::options_registry());

        let config = Self {
            pool,
            config_path: db_path,
            options,
        };

        // Print migration message only if migration actually happened
        if migrated {
            println!("✓ Migrated legacy TOML configuration to SQLite");
        }

        Ok(config)
    }

    /// Create a new config for testing (in-memory database)
    pub async fn new_test() -> Result<Self> {
        let pool = db::connect_memory().await?;
        db::run_migrations(&pool).await?;

        // Create a test registry for isolated testing
        let test_registry = Arc::new(options::OptionsRegistry::new());
        options::registrations::register_all(&test_registry)?;
        let options = options::Options::new(pool.clone(), test_registry);

        Ok(Self {
            pool,
            config_path: PathBuf::from(":memory:"),
            options,
        })
    }

    // Credential management methods
    pub async fn add_credentials(&self, name: String, credentials: ApiCredentialSet) -> Result<()> {
        repository::credentials::insert(&self.pool, name, credentials).await
    }

    pub async fn get_credentials(&self, name: &str) -> Result<Option<ApiCredentialSet>> {
        repository::credentials::get(&self.pool, name).await
    }

    pub async fn list_credentials(&self) -> Result<Vec<String>> {
        repository::credentials::list(&self.pool).await
    }

    pub async fn delete_credentials(&self, name: &str) -> Result<()> {
        repository::credentials::delete(&self.pool, name).await
    }

    pub async fn rename_credentials(&self, old_name: &str, new_name: String) -> Result<()> {
        repository::credentials::rename(&self.pool, old_name, new_name).await
    }

    // Environment management methods
    pub async fn add_environment(&self, environment: ApiEnvironment) -> Result<()> {
        repository::environments::insert(&self.pool, environment).await
    }

    pub async fn get_environment(&self, name: &str) -> Result<Option<ApiEnvironment>> {
        repository::environments::get(&self.pool, name).await
    }

    pub async fn list_environments(&self) -> Result<Vec<String>> {
        repository::environments::list(&self.pool).await
    }

    pub async fn delete_environment(&self, name: &str) -> Result<()> {
        repository::environments::delete(&self.pool, name).await
    }

    pub async fn rename_environment(&self, old_name: &str, new_name: String) -> Result<()> {
        repository::environments::rename(&self.pool, old_name, new_name).await
    }

    pub async fn get_current_environment(&self) -> Result<Option<String>> {
        repository::environments::get_current(&self.pool).await
    }

    pub async fn set_current_environment(&self, name: String) -> Result<()> {
        repository::environments::set_current(&self.pool, name).await
    }

    // Token management methods
    pub async fn save_token(&self, env_name: String, token: crate::api::models::TokenInfo) -> Result<()> {
        repository::tokens::save(&self.pool, env_name, token).await
    }

    pub async fn get_token(&self, env_name: &str) -> Result<Option<crate::api::models::TokenInfo>> {
        repository::tokens::get(&self.pool, env_name).await
    }

    pub async fn delete_token(&self, env_name: &str) -> Result<()> {
        repository::tokens::delete(&self.pool, env_name).await
    }

    // Legacy configuration methods (entity mappings, etc.)
    pub async fn add_entity_mapping(&self, singular: String, plural: String) -> Result<()> {
        repository::legacy::add_entity_mapping(&self.pool, singular, plural).await
    }

    pub async fn get_entity_mapping(&self, singular: &str) -> Result<Option<String>> {
        repository::legacy::get_entity_mapping(&self.pool, singular).await
    }

    pub async fn list_entity_mappings(&self) -> Result<Vec<(String, String)>> {
        repository::legacy::list_entity_mappings(&self.pool).await
    }

    pub async fn delete_entity_mapping(&self, singular: &str) -> Result<()> {
        repository::legacy::delete_entity_mapping(&self.pool, singular).await
    }

    // Settings methods
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        repository::legacy::get_setting(&self.pool, key).await
    }

    pub async fn set_setting(&self, key: String, value: String) -> Result<()> {
        repository::legacy::set_setting(&self.pool, key, value).await
    }

    pub async fn get_default_query_limit(&self) -> Result<u32> {
        let value = self.get_setting("default_query_limit").await?
            .unwrap_or_else(|| "100".to_string());
        value.parse().context("Invalid default_query_limit")
    }

    pub async fn set_default_query_limit(&self, limit: u32) -> Result<()> {
        self.set_setting("default_query_limit".to_string(), limit.to_string()).await
    }

    // Export to TOML for debugging/backup
    pub async fn export_toml(&self, path: &std::path::Path) -> Result<()> {
        compat::export_to_toml(&self.pool, path).await
    }

    // Import from TOML (for migration or restore)
    pub async fn import_toml(&self, path: &std::path::Path) -> Result<()> {
        compat::import_from_toml(&self.pool, path).await
    }

    // Migration management methods
    pub async fn add_migration(&self, migration: SavedMigration) -> Result<()> {
        repository::migrations::insert(&self.pool, migration).await
    }

    pub async fn get_migration(&self, name: &str) -> Result<Option<SavedMigration>> {
        repository::migrations::get(&self.pool, name).await
    }

    pub async fn list_migrations(&self) -> Result<Vec<SavedMigration>> {
        repository::migrations::list(&self.pool).await
    }

    pub async fn delete_migration(&self, name: &str) -> Result<()> {
        repository::migrations::delete(&self.pool, name).await
    }

    pub async fn touch_migration(&self, name: &str) -> Result<()> {
        repository::migrations::touch(&self.pool, name).await
    }

    // Comparison management methods
    pub async fn add_comparison(&self, comparison: SavedComparison) -> Result<i64> {
        repository::migrations::insert_comparison(&self.pool, comparison).await
    }

    pub async fn get_comparison_by_id(&self, id: i64) -> Result<Option<SavedComparison>> {
        repository::migrations::get_comparison_by_id(&self.pool, id).await
    }

    pub async fn get_comparisons(&self, migration_name: &str) -> Result<Vec<SavedComparison>> {
        repository::migrations::get_comparisons(&self.pool, migration_name).await
    }

    pub async fn delete_comparison(&self, id: i64) -> Result<()> {
        repository::migrations::delete_comparison(&self.pool, id).await
    }

    pub async fn rename_comparison(&self, id: i64, new_name: &str) -> Result<()> {
        repository::migrations::rename_comparison(&self.pool, id, new_name).await
    }

    // Entity cache methods
    pub async fn get_entity_cache(&self, environment_name: &str, max_age_hours: i64) -> Result<Option<Vec<String>>> {
        if let Some((entities, cached_at)) = repository::entity_cache::get(&self.pool, environment_name).await? {
            let age = chrono::Utc::now().signed_duration_since(cached_at);
            if age.num_hours() < max_age_hours {
                return Ok(Some(entities));
            }
        }
        Ok(None)
    }

    pub async fn set_entity_cache(&self, environment_name: &str, entities: Vec<String>) -> Result<()> {
        repository::entity_cache::set(&self.pool, environment_name, entities).await
    }

    pub async fn delete_entity_cache(&self, environment_name: &str) -> Result<()> {
        repository::entity_cache::delete(&self.pool, environment_name).await
    }

    // Entity metadata cache methods
    pub async fn get_entity_metadata_cache(
        &self,
        environment_name: &str,
        entity_name: &str,
        max_age_hours: i64,
    ) -> Result<Option<crate::api::EntityMetadata>> {
        if let Some((metadata, cached_at)) = repository::entity_metadata_cache::get(&self.pool, environment_name, entity_name).await? {
            let age = chrono::Utc::now().signed_duration_since(cached_at);
            if age.num_hours() < max_age_hours {
                return Ok(Some(metadata));
            }
        }
        Ok(None)
    }

    pub async fn set_entity_metadata_cache(
        &self,
        environment_name: &str,
        entity_name: &str,
        metadata: &crate::api::EntityMetadata,
    ) -> Result<()> {
        repository::entity_metadata_cache::set(&self.pool, environment_name, entity_name, metadata).await
    }

    pub async fn delete_entity_metadata_cache(&self, environment_name: &str, entity_name: &str) -> Result<()> {
        repository::entity_metadata_cache::delete(&self.pool, environment_name, entity_name).await
    }

    pub async fn delete_all_entity_metadata_cache(&self, environment_name: &str) -> Result<()> {
        repository::entity_metadata_cache::delete_all_for_environment(&self.pool, environment_name).await
    }

    // Entity data cache methods
    pub async fn get_entity_data_cache(
        &self,
        environment_name: &str,
        entity_name: &str,
        max_age_hours: i64,
    ) -> Result<Option<Vec<serde_json::Value>>> {
        if let Some((data, cached_at)) = repository::entity_data_cache::get(&self.pool, environment_name, entity_name).await? {
            let age = chrono::Utc::now().signed_duration_since(cached_at);
            if age.num_hours() < max_age_hours {
                return Ok(Some(data));
            }
        }
        Ok(None)
    }

    pub async fn set_entity_data_cache(
        &self,
        environment_name: &str,
        entity_name: &str,
        data: &[serde_json::Value],
    ) -> Result<()> {
        repository::entity_data_cache::set(&self.pool, environment_name, entity_name, data).await
    }

    pub async fn delete_entity_data_cache(&self, environment_name: &str, entity_name: &str) -> Result<()> {
        repository::entity_data_cache::delete(&self.pool, environment_name, entity_name).await
    }

    pub async fn delete_all_entity_data_cache(&self, environment_name: &str) -> Result<()> {
        repository::entity_data_cache::delete_all_for_environment(&self.pool, environment_name).await
    }

    // Field and prefix mapping methods
    pub async fn get_field_mappings(&self, source_entity: &str, target_entity: &str) -> Result<std::collections::HashMap<String, Vec<String>>> {
        repository::mappings::get_field_mappings(&self.pool, source_entity, target_entity).await
    }

    pub async fn set_field_mapping(&self, source_entity: &str, target_entity: &str, source_field: &str, target_field: &str) -> Result<()> {
        repository::mappings::set_field_mapping(&self.pool, source_entity, target_entity, source_field, target_field).await
    }

    pub async fn delete_field_mapping(&self, source_entity: &str, target_entity: &str, source_field: &str) -> Result<()> {
        repository::mappings::delete_field_mapping(&self.pool, source_entity, target_entity, source_field).await
    }

    pub async fn get_prefix_mappings(&self, source_entity: &str, target_entity: &str) -> Result<std::collections::HashMap<String, Vec<String>>> {
        repository::mappings::get_prefix_mappings(&self.pool, source_entity, target_entity).await
    }

    pub async fn set_prefix_mapping(&self, source_entity: &str, target_entity: &str, source_prefix: &str, target_prefix: &str) -> Result<()> {
        repository::mappings::set_prefix_mapping(&self.pool, source_entity, target_entity, source_prefix, target_prefix).await
    }

    pub async fn delete_prefix_mapping(&self, source_entity: &str, target_entity: &str, source_prefix: &str) -> Result<()> {
        repository::mappings::delete_prefix_mapping(&self.pool, source_entity, target_entity, source_prefix).await
    }

    pub async fn get_imported_mappings(&self, source_entity: &str, target_entity: &str) -> Result<(std::collections::HashMap<String, Vec<String>>, Option<String>)> {
        repository::mappings::get_imported_mappings(&self.pool, source_entity, target_entity).await
    }

    pub async fn set_imported_mappings(&self, source_entity: &str, target_entity: &str, mappings: &std::collections::HashMap<String, Vec<String>>, source_file: &str) -> Result<()> {
        repository::mappings::set_imported_mappings(&self.pool, source_entity, target_entity, mappings, source_file).await
    }

    pub async fn clear_imported_mappings(&self, source_entity: &str, target_entity: &str) -> Result<()> {
        repository::mappings::clear_imported_mappings(&self.pool, source_entity, target_entity).await
    }

    /// Get ignored items for entity comparison
    pub async fn get_ignored_items(&self, source_entity: &str, target_entity: &str) -> Result<std::collections::HashSet<String>> {
        repository::mappings::get_ignored_items(&self.pool, source_entity, target_entity).await
    }

    /// Set ignored items for entity comparison
    pub async fn set_ignored_items(&self, source_entity: &str, target_entity: &str, ignored: &std::collections::HashSet<String>) -> Result<()> {
        repository::mappings::set_ignored_items(&self.pool, source_entity, target_entity, ignored).await
    }

    /// Clear all ignored items for entity comparison
    pub async fn clear_ignored_items(&self, source_entity: &str, target_entity: &str) -> Result<()> {
        repository::mappings::clear_ignored_items(&self.pool, source_entity, target_entity).await
    }

    pub async fn get_negative_matches(&self, source_entity: &str, target_entity: &str) -> Result<std::collections::HashSet<String>> {
        repository::mappings::get_negative_matches(&self.pool, source_entity, target_entity).await
    }

    pub async fn add_negative_match(&self, source_entity: &str, target_entity: &str, source_field: &str) -> Result<()> {
        repository::mappings::add_negative_match(&self.pool, source_entity, target_entity, source_field).await
    }

    pub async fn delete_negative_match(&self, source_entity: &str, target_entity: &str, source_field: &str) -> Result<()> {
        repository::mappings::delete_negative_match(&self.pool, source_entity, target_entity, source_field).await
    }

    /// Get example pairs for entity comparison
    pub async fn get_example_pairs(&self, source_entity: &str, target_entity: &str) -> Result<Vec<crate::tui::apps::migration::entity_comparison::ExamplePair>> {
        repository::examples::get_example_pairs(&self.pool, source_entity, target_entity).await
    }

    /// Save example pair
    pub async fn save_example_pair(&self, source_entity: &str, target_entity: &str, pair: &crate::tui::apps::migration::entity_comparison::ExamplePair) -> Result<()> {
        repository::examples::save_example_pair(&self.pool, source_entity, target_entity, pair).await
    }

    /// Delete example pair
    pub async fn delete_example_pair(&self, pair_id: &str) -> Result<()> {
        repository::examples::delete_example_pair(&self.pool, pair_id).await
    }

    // Queue item methods
    pub async fn save_queue_item(&self, item: &crate::tui::apps::queue::models::QueueItem) -> Result<()> {
        repository::queue::save_queue_item(&self.pool, item).await
    }

    pub async fn get_queue_item(&self, id: &str) -> Result<Option<crate::tui::apps::queue::models::QueueItem>> {
        repository::queue::get_queue_item(&self.pool, id).await
    }

    pub async fn list_queue_items(&self) -> Result<Vec<crate::tui::apps::queue::models::QueueItem>> {
        repository::queue::list_queue_items(&self.pool).await
    }

    pub async fn update_queue_item_status(&self, id: &str, status: crate::tui::apps::queue::models::OperationStatus) -> Result<()> {
        repository::queue::update_queue_item_status(&self.pool, id, status).await
    }

    pub async fn update_queue_item_priority(&self, id: &str, priority: u8) -> Result<()> {
        repository::queue::update_queue_item_priority(&self.pool, id, priority).await
    }

    pub async fn update_queue_item_result(&self, id: &str, result: &crate::tui::apps::queue::models::QueueResult) -> Result<()> {
        repository::queue::update_queue_item_result(&self.pool, id, result).await
    }

    pub async fn mark_queue_item_interrupted(&self, id: &str, interrupted_at: chrono::DateTime<chrono::Utc>) -> Result<()> {
        repository::queue::mark_queue_item_interrupted(&self.pool, id, interrupted_at).await
    }

    pub async fn clear_queue_interruption_flag(&self, id: &str) -> Result<()> {
        repository::queue::clear_interruption_flag(&self.pool, id).await
    }

    pub async fn delete_queue_item(&self, id: &str) -> Result<()> {
        repository::queue::delete_queue_item(&self.pool, id).await
    }

    pub async fn clear_queue(&self) -> Result<()> {
        repository::queue::clear_queue(&self.pool).await
    }

    pub async fn get_queue_settings(&self) -> Result<repository::queue::QueueSettings> {
        repository::queue::get_queue_settings(&self.pool).await
    }

    pub async fn save_queue_settings(&self, settings: &repository::queue::QueueSettings) -> Result<()> {
        repository::queue::save_queue_settings(&self.pool, settings).await
    }
}