//! Data models for configuration database

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::time::SystemTime;

/// Database representation of credentials
#[derive(Debug, Clone, FromRow)]
pub struct DbCredential {
    pub name: String,
    pub r#type: String,
    pub data: String, // JSON
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Database representation of environment
#[derive(Debug, Clone, FromRow)]
pub struct DbEnvironment {
    pub name: String,
    pub host: String,
    pub credentials_ref: String,
    pub is_current: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Database representation of token
#[derive(Debug, Clone, FromRow)]
pub struct DbToken {
    pub environment_name: String,
    pub access_token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub refresh_token: Option<String>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Database representation of entity mapping
#[derive(Debug, Clone, FromRow)]
pub struct DbEntityMapping {
    pub singular_name: String,
    pub plural_name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Database representation of field mapping
#[derive(Debug, Clone, FromRow)]
pub struct DbFieldMapping {
    pub id: i64,
    pub source_entity: String,
    pub target_entity: String,
    pub source_field: String,
    pub target_field: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Database representation of prefix mapping
#[derive(Debug, Clone, FromRow)]
pub struct DbPrefixMapping {
    pub id: i64,
    pub source_entity: String,
    pub target_entity: String,
    pub source_prefix: String,
    pub target_prefix: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Database representation of migration
#[derive(Debug, Clone, FromRow)]
pub struct DbMigration {
    pub name: String,
    pub source_env: String,
    pub target_env: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used: chrono::DateTime<chrono::Utc>,
}

/// Database representation of comparison
#[derive(Debug, Clone, FromRow)]
pub struct DbComparison {
    pub id: i64,
    pub migration_name: String,
    pub name: String,
    pub source_entity: String,             // Legacy: first source entity
    pub target_entity: String,             // Legacy: first target entity
    pub source_entities: Option<String>,   // JSON array: ["contact", "account"]
    pub target_entities: Option<String>,   // JSON array: ["lead", "customer"]
    pub entity_comparison: Option<String>, // JSON
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used: chrono::DateTime<chrono::Utc>,
}

/// Database representation of view comparison
#[derive(Debug, Clone, FromRow)]
pub struct DbViewComparison {
    pub id: i64,
    pub comparison_id: i64,
    pub source_view_name: String,
    pub target_view_name: String,
    pub column_mappings: Option<String>, // JSON
    pub filter_mappings: Option<String>, // JSON
    pub sort_mappings: Option<String>,   // JSON
}

/// Database representation of example pair
#[derive(Debug, Clone, FromRow)]
pub struct DbExamplePair {
    pub id: String,
    pub source_entity: String,
    pub target_entity: String,
    pub source_uuid: String,
    pub target_uuid: String,
    pub label: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Database representation of setting
#[derive(Debug, Clone, FromRow)]
pub struct DbSetting {
    pub key: String,
    pub value: String,
    pub r#type: String,
}

/// Serializable credential data for JSON storage
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CredentialData {
    UsernamePassword {
        username: String,
        password: String,
        client_id: String,
        client_secret: String,
    },
    ClientCredentials {
        client_id: String,
        client_secret: String,
        tenant_id: String,
    },
    DeviceCode {
        client_id: String,
        tenant_id: String,
    },
    Certificate {
        client_id: String,
        tenant_id: String,
        cert_path: String,
    },
}

impl From<crate::api::models::CredentialSet> for CredentialData {
    fn from(creds: crate::api::models::CredentialSet) -> Self {
        match creds {
            crate::api::models::CredentialSet::UsernamePassword {
                username,
                password,
                client_id,
                client_secret,
            } => CredentialData::UsernamePassword {
                username,
                password,
                client_id,
                client_secret,
            },
            crate::api::models::CredentialSet::ClientCredentials {
                client_id,
                client_secret,
                tenant_id,
            } => CredentialData::ClientCredentials {
                client_id,
                client_secret,
                tenant_id,
            },
            crate::api::models::CredentialSet::DeviceCode {
                client_id,
                tenant_id,
            } => CredentialData::DeviceCode {
                client_id,
                tenant_id,
            },
            crate::api::models::CredentialSet::Certificate {
                client_id,
                tenant_id,
                cert_path,
            } => CredentialData::Certificate {
                client_id,
                tenant_id,
                cert_path,
            },
        }
    }
}

impl From<CredentialData> for crate::api::models::CredentialSet {
    fn from(data: CredentialData) -> Self {
        match data {
            CredentialData::UsernamePassword {
                username,
                password,
                client_id,
                client_secret,
            } => crate::api::models::CredentialSet::UsernamePassword {
                username,
                password,
                client_id,
                client_secret,
            },
            CredentialData::ClientCredentials {
                client_id,
                client_secret,
                tenant_id,
            } => crate::api::models::CredentialSet::ClientCredentials {
                client_id,
                client_secret,
                tenant_id,
            },
            CredentialData::DeviceCode {
                client_id,
                tenant_id,
            } => crate::api::models::CredentialSet::DeviceCode {
                client_id,
                tenant_id,
            },
            CredentialData::Certificate {
                client_id,
                tenant_id,
                cert_path,
            } => crate::api::models::CredentialSet::Certificate {
                client_id,
                tenant_id,
                cert_path,
            },
        }
    }
}

// Helper function to convert SystemTime to chrono::DateTime<Utc>
pub fn system_time_to_chrono(time: SystemTime) -> chrono::DateTime<chrono::Utc> {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| {
            chrono::DateTime::from_timestamp(duration.as_secs() as i64, duration.subsec_nanos())
        })
        .unwrap_or(None)
        .unwrap_or_else(chrono::Utc::now)
}

// Helper function to convert chrono::DateTime<Utc> to SystemTime
pub fn chrono_to_system_time(time: chrono::DateTime<chrono::Utc>) -> SystemTime {
    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(time.timestamp() as u64)
}
