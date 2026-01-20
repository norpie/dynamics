//! Database-backed storage for options with validation

use super::registry::OptionsRegistry;
use super::types::{OptionType, OptionValue};
use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::sync::Arc;

/// Database-backed options store with type validation
pub struct Options {
    pool: SqlitePool,
    registry: Arc<OptionsRegistry>,
}

impl Options {
    /// Create a new options store
    pub fn new(pool: SqlitePool, registry: Arc<OptionsRegistry>) -> Self {
        Self { pool, registry }
    }

    /// Get a reference to the options registry
    pub fn registry(&self) -> &OptionsRegistry {
        &self.registry
    }

    /// Get option value with type checking and default fallback
    pub async fn get(&self, key: &str) -> Result<OptionValue> {
        // Get definition from registry
        let def = self
            .registry
            .get(key)
            .ok_or_else(|| anyhow::anyhow!("Option '{}' is not registered", key))?;

        // Try to load from database
        if let Some(raw_value) = self.get_raw(key).await? {
            // Parse based on expected type
            self.parse_value(&raw_value, &def.ty)
        } else {
            // Return default if not in DB
            Ok(def.default.clone())
        }
    }

    /// Set option value with validation
    pub async fn set(&self, key: &str, value: OptionValue) -> Result<()> {
        // Get definition
        let def = self
            .registry
            .get(key)
            .ok_or_else(|| anyhow::anyhow!("Option '{}' is not registered", key))?;

        // Validate type and constraints
        def.validate(&value)?;

        // Serialize and save
        let raw_value = self.serialize_value(&value);
        self.set_raw(key, &raw_value).await
    }

    /// Get bool value
    pub async fn get_bool(&self, key: &str) -> Result<bool> {
        self.get(key).await?.as_bool()
    }

    /// Get uint value
    pub async fn get_uint(&self, key: &str) -> Result<u64> {
        self.get(key).await?.as_uint()
    }

    /// Get int value
    pub async fn get_int(&self, key: &str) -> Result<i64> {
        self.get(key).await?.as_int()
    }

    /// Get float value
    pub async fn get_float(&self, key: &str) -> Result<f64> {
        self.get(key).await?.as_float()
    }

    /// Get string value
    pub async fn get_string(&self, key: &str) -> Result<String> {
        self.get(key).await?.as_string()
    }

    /// Set bool value
    pub async fn set_bool(&self, key: &str, value: bool) -> Result<()> {
        self.set(key, OptionValue::Bool(value)).await
    }

    /// Set uint value
    pub async fn set_uint(&self, key: &str, value: u64) -> Result<()> {
        self.set(key, OptionValue::UInt(value)).await
    }

    /// Set int value
    pub async fn set_int(&self, key: &str, value: i64) -> Result<()> {
        self.set(key, OptionValue::Int(value)).await
    }

    /// Set float value
    pub async fn set_float(&self, key: &str, value: f64) -> Result<()> {
        self.set(key, OptionValue::Float(value)).await
    }

    /// Set string value
    pub async fn set_string(&self, key: &str, value: String) -> Result<()> {
        self.set(key, OptionValue::String(value)).await
    }

    /// Delete an option by key
    pub async fn delete(&self, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM options WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await
            .context("Failed to delete option")?;
        Ok(())
    }

    /// Parse raw string value based on expected type
    fn parse_value(&self, raw: &str, ty: &OptionType) -> Result<OptionValue> {
        match ty {
            OptionType::Bool => {
                let value = raw.parse::<bool>().context("Failed to parse as bool")?;
                Ok(OptionValue::Bool(value))
            }
            OptionType::Int { .. } => {
                let value = raw.parse::<i64>().context("Failed to parse as int")?;
                Ok(OptionValue::Int(value))
            }
            OptionType::UInt { .. } => {
                let value = raw.parse::<u64>().context("Failed to parse as uint")?;
                Ok(OptionValue::UInt(value))
            }
            OptionType::Float { .. } => {
                let value = raw.parse::<f64>().context("Failed to parse as float")?;
                Ok(OptionValue::Float(value))
            }
            OptionType::String { .. } | OptionType::Enum { .. } => {
                Ok(OptionValue::String(raw.to_string()))
            }
        }
    }

    /// Serialize option value to string for database storage
    fn serialize_value(&self, value: &OptionValue) -> String {
        match value {
            OptionValue::Bool(v) => v.to_string(),
            OptionValue::Int(v) => v.to_string(),
            OptionValue::UInt(v) => v.to_string(),
            OptionValue::Float(v) => v.to_string(),
            OptionValue::String(v) => v.clone(),
        }
    }

    /// Get raw value from database
    async fn get_raw(&self, key: &str) -> Result<Option<String>> {
        sqlx::query_scalar("SELECT value FROM options WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to get option from database")
    }

    /// Set raw value in database
    async fn set_raw(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO options (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = CURRENT_TIMESTAMP",
        )
        .bind(key)
        .bind(value)
        .bind(value)
        .execute(&self.pool)
        .await
        .context("Failed to set option in database")?;

        log::debug!("Set option: {} = {}", key, value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::options::types::OptionDefinition;

    async fn setup_test_store() -> (Options, Arc<OptionsRegistry>) {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

        // Create options table
        sqlx::query(
            "CREATE TABLE options (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        let registry = Arc::new(OptionsRegistry::new());
        let store = Options::new(pool, registry.clone());

        (store, registry)
    }

    #[tokio::test]
    async fn test_bool_roundtrip() {
        let (store, registry) = setup_test_store().await;

        registry
            .register(OptionDefinition {
                key: "test.bool".to_string(),
                namespace: "test".to_string(),
                local_key: "bool".to_string(),
                display_name: "Test Bool".to_string(),
                description: "".to_string(),
                ty: OptionType::Bool,
                default: OptionValue::Bool(false),
            })
            .unwrap();

        // Should return default
        assert_eq!(store.get_bool("test.bool").await.unwrap(), false);

        // Set and retrieve
        store.set_bool("test.bool", true).await.unwrap();
        assert_eq!(store.get_bool("test.bool").await.unwrap(), true);
    }

    #[tokio::test]
    async fn test_uint_validation() {
        let (store, registry) = setup_test_store().await;

        registry
            .register(OptionDefinition {
                key: "test.uint".to_string(),
                namespace: "test".to_string(),
                local_key: "uint".to_string(),
                display_name: "Test UInt".to_string(),
                description: "".to_string(),
                ty: OptionType::UInt {
                    min: Some(1),
                    max: Some(10),
                },
                default: OptionValue::UInt(5),
            })
            .unwrap();

        // Should accept valid value
        store.set_uint("test.uint", 7).await.unwrap();
        assert_eq!(store.get_uint("test.uint").await.unwrap(), 7);

        // Should reject value below min
        let result = store.set_uint("test.uint", 0).await;
        assert!(result.is_err());

        // Should reject value above max
        let result = store.set_uint("test.uint", 11).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_enum_validation() {
        let (store, registry) = setup_test_store().await;

        registry
            .register(OptionDefinition {
                key: "test.enum".to_string(),
                namespace: "test".to_string(),
                local_key: "enum".to_string(),
                display_name: "Test Enum".to_string(),
                description: "".to_string(),
                ty: OptionType::Enum {
                    variants: vec!["option1".to_string(), "option2".to_string()],
                },
                default: OptionValue::String("option1".to_string()),
            })
            .unwrap();

        // Should accept valid variant
        store
            .set_string("test.enum", "option2".to_string())
            .await
            .unwrap();
        assert_eq!(store.get_string("test.enum").await.unwrap(), "option2");

        // Should reject invalid variant
        let result = store.set_string("test.enum", "invalid".to_string()).await;
        assert!(result.is_err());
    }
}
