//! Types for Lua transform mode
//!
//! Defines the data structures used for Lua script declarations and operations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Declaration returned by M.declare() in Lua scripts
/// Specifies what data to fetch from source and target environments
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Declaration {
    /// Entities to fetch from source environment
    #[serde(default)]
    pub source: HashMap<String, EntityDeclaration>,
    /// Entities to fetch from target environment
    #[serde(default)]
    pub target: HashMap<String, EntityDeclaration>,
}

/// Declaration for a single entity's data requirements
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityDeclaration {
    /// Fields to select
    #[serde(default)]
    pub fields: Vec<String>,
    /// Navigation properties to expand
    #[serde(default)]
    pub expand: Vec<String>,
    /// OData filter expression
    #[serde(default)]
    pub filter: Option<String>,
    /// Maximum number of records to fetch
    #[serde(default)]
    pub top: Option<usize>,
}

/// Operation type returned by M.transform() in Lua scripts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationType {
    /// Create a new record
    Create,
    /// Update an existing record
    Update,
    /// Delete a record
    Delete,
    /// Deactivate a record (set statecode = 1)
    Deactivate,
    /// Skip this record (for audit purposes)
    Skip,
    /// Mark as error (for audit purposes)
    Error,
}

impl OperationType {
    /// Get display label
    pub fn label(&self) -> &'static str {
        match self {
            OperationType::Create => "Create",
            OperationType::Update => "Update",
            OperationType::Delete => "Delete",
            OperationType::Deactivate => "Deactivate",
            OperationType::Skip => "Skip",
            OperationType::Error => "Error",
        }
    }

    /// Parse from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "create" => Some(OperationType::Create),
            "update" => Some(OperationType::Update),
            "delete" => Some(OperationType::Delete),
            "deactivate" => Some(OperationType::Deactivate),
            "skip" => Some(OperationType::Skip),
            "error" => Some(OperationType::Error),
            _ => None,
        }
    }
}

/// A single operation returned by M.transform()
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuaOperation {
    /// Target entity logical name
    pub entity: String,
    /// Operation type
    pub operation: OperationType,
    /// Record ID (required for update/delete/deactivate, optional for create)
    #[serde(default)]
    pub id: Option<Uuid>,
    /// Field values (required for create/update)
    #[serde(default)]
    pub fields: HashMap<String, serde_json::Value>,
    /// Reason for skip operation
    #[serde(default)]
    pub reason: Option<String>,
    /// Error message for error operation
    #[serde(default)]
    pub error: Option<String>,
}

impl LuaOperation {
    /// Create a new create operation
    pub fn create(entity: impl Into<String>, fields: HashMap<String, serde_json::Value>) -> Self {
        LuaOperation {
            entity: entity.into(),
            operation: OperationType::Create,
            id: None,
            fields,
            reason: None,
            error: None,
        }
    }

    /// Create a new update operation
    pub fn update(
        entity: impl Into<String>,
        id: Uuid,
        fields: HashMap<String, serde_json::Value>,
    ) -> Self {
        LuaOperation {
            entity: entity.into(),
            operation: OperationType::Update,
            id: Some(id),
            fields,
            reason: None,
            error: None,
        }
    }

    /// Create a new delete operation
    pub fn delete(entity: impl Into<String>, id: Uuid) -> Self {
        LuaOperation {
            entity: entity.into(),
            operation: OperationType::Delete,
            id: Some(id),
            fields: HashMap::new(),
            reason: None,
            error: None,
        }
    }

    /// Create a new deactivate operation
    pub fn deactivate(entity: impl Into<String>, id: Uuid) -> Self {
        LuaOperation {
            entity: entity.into(),
            operation: OperationType::Deactivate,
            id: Some(id),
            fields: HashMap::new(),
            reason: None,
            error: None,
        }
    }

    /// Create a skip operation
    pub fn skip(entity: impl Into<String>, reason: Option<String>) -> Self {
        LuaOperation {
            entity: entity.into(),
            operation: OperationType::Skip,
            id: None,
            fields: HashMap::new(),
            reason,
            error: None,
        }
    }

    /// Create an error operation
    pub fn error(entity: impl Into<String>, error: impl Into<String>) -> Self {
        LuaOperation {
            entity: entity.into(),
            operation: OperationType::Error,
            id: None,
            fields: HashMap::new(),
            reason: None,
            error: Some(error.into()),
        }
    }

    /// Validate that the operation has required fields
    pub fn validate(&self) -> Result<(), String> {
        match self.operation {
            OperationType::Create => {
                if self.fields.is_empty() {
                    return Err("Create operation requires fields".to_string());
                }
            }
            OperationType::Update => {
                if self.id.is_none() {
                    return Err("Update operation requires id".to_string());
                }
                if self.fields.is_empty() {
                    return Err("Update operation requires fields".to_string());
                }
            }
            OperationType::Delete | OperationType::Deactivate => {
                if self.id.is_none() {
                    return Err(format!("{} operation requires id", self.operation.label()));
                }
            }
            OperationType::Skip | OperationType::Error => {
                // No required fields
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_type_from_str() {
        assert_eq!(
            OperationType::from_str("create"),
            Some(OperationType::Create)
        );
        assert_eq!(
            OperationType::from_str("CREATE"),
            Some(OperationType::Create)
        );
        assert_eq!(
            OperationType::from_str("Update"),
            Some(OperationType::Update)
        );
        assert_eq!(OperationType::from_str("invalid"), None);
    }

    #[test]
    fn test_operation_validation() {
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), serde_json::json!("Test"));

        // Valid create
        let op = LuaOperation::create("account", fields.clone());
        assert!(op.validate().is_ok());

        // Invalid create (no fields)
        let op = LuaOperation::create("account", HashMap::new());
        assert!(op.validate().is_err());

        // Valid update
        let id = Uuid::new_v4();
        let op = LuaOperation::update("account", id, fields.clone());
        assert!(op.validate().is_ok());

        // Invalid update (no id)
        let mut op = LuaOperation::update("account", id, fields);
        op.id = None;
        assert!(op.validate().is_err());

        // Valid delete
        let op = LuaOperation::delete("account", id);
        assert!(op.validate().is_ok());

        // Invalid delete (no id)
        let mut op = LuaOperation::delete("account", id);
        op.id = None;
        assert!(op.validate().is_err());
    }
}
