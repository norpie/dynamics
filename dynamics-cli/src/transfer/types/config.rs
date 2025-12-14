//! Transfer configuration types

use serde::{Deserialize, Serialize};

use super::Transform;

/// How to handle records that exist in target but not in source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrphanHandling {
    /// Show in preview but take no action (default)
    #[default]
    Ignore,
    /// Delete the record from target
    Delete,
    /// Deactivate the record (set statecode = 1)
    Deactivate,
}

impl OrphanHandling {
    /// Get display label for UI
    pub fn label(&self) -> &'static str {
        match self {
            OrphanHandling::Ignore => "Ignore",
            OrphanHandling::Delete => "Delete",
            OrphanHandling::Deactivate => "Deactivate",
        }
    }

    /// Get all variants for UI selection
    pub fn all_variants() -> &'static [OrphanHandling] {
        &[
            OrphanHandling::Ignore,
            OrphanHandling::Delete,
            OrphanHandling::Deactivate,
        ]
    }

    /// Convert from index (for UI selection)
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => OrphanHandling::Ignore,
            1 => OrphanHandling::Delete,
            2 => OrphanHandling::Deactivate,
            _ => OrphanHandling::Ignore,
        }
    }

    /// Convert to index (for UI selection)
    pub fn to_index(&self) -> usize {
        match self {
            OrphanHandling::Ignore => 0,
            OrphanHandling::Delete => 1,
            OrphanHandling::Deactivate => 2,
        }
    }
}

/// Top-level transfer configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferConfig {
    /// Database ID (None if not yet persisted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Human-readable name for this config
    pub name: String,
    /// Source environment name
    pub source_env: String,
    /// Target environment name
    pub target_env: String,
    /// Entity mappings
    pub entity_mappings: Vec<EntityMapping>,
}

impl TransferConfig {
    /// Create a new transfer config
    pub fn new(name: impl Into<String>, source_env: impl Into<String>, target_env: impl Into<String>) -> Self {
        TransferConfig {
            id: None,
            name: name.into(),
            source_env: source_env.into(),
            target_env: target_env.into(),
            entity_mappings: Vec::new(),
        }
    }

    /// Add an entity mapping
    pub fn add_entity_mapping(&mut self, mapping: EntityMapping) {
        self.entity_mappings.push(mapping);
    }

    /// Get entity mappings sorted by priority (lower priority first)
    pub fn entity_mappings_by_priority(&self) -> Vec<&EntityMapping> {
        let mut mappings: Vec<_> = self.entity_mappings.iter().collect();
        mappings.sort_by_key(|m| m.priority);
        mappings
    }

    /// Find an entity mapping by source entity name
    pub fn find_entity_mapping(&self, source_entity: &str) -> Option<&EntityMapping> {
        self.entity_mappings
            .iter()
            .find(|m| m.source_entity == source_entity)
    }

    /// Find an entity mapping by source entity name (mutable)
    pub fn find_entity_mapping_mut(&mut self, source_entity: &str) -> Option<&mut EntityMapping> {
        self.entity_mappings
            .iter_mut()
            .find(|m| m.source_entity == source_entity)
    }
}

impl Default for TransferConfig {
    fn default() -> Self {
        TransferConfig {
            id: None,
            name: String::new(),
            source_env: String::new(),
            target_env: String::new(),
            entity_mappings: Vec::new(),
        }
    }
}

/// Mapping from a source entity to a target entity
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityMapping {
    /// Database ID (None if not yet persisted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Source entity logical name
    pub source_entity: String,
    /// Target entity logical name
    pub target_entity: String,
    /// Execution priority (lower = runs first)
    /// Used to handle dependencies (e.g., accounts before contacts)
    pub priority: u32,
    /// How to handle records that exist in target but not in source
    #[serde(default)]
    pub orphan_handling: OrphanHandling,
    /// Field mappings for this entity
    pub field_mappings: Vec<FieldMapping>,
}

impl EntityMapping {
    /// Create a new entity mapping
    pub fn new(
        source_entity: impl Into<String>,
        target_entity: impl Into<String>,
        priority: u32,
    ) -> Self {
        EntityMapping {
            id: None,
            source_entity: source_entity.into(),
            target_entity: target_entity.into(),
            priority,
            orphan_handling: OrphanHandling::default(),
            field_mappings: Vec::new(),
        }
    }

    /// Create a same-entity mapping (source = target)
    pub fn same_entity(entity: impl Into<String>, priority: u32) -> Self {
        let entity = entity.into();
        EntityMapping::new(entity.clone(), entity, priority)
    }

    /// Add a field mapping
    pub fn add_field_mapping(&mut self, mapping: FieldMapping) {
        self.field_mappings.push(mapping);
    }

    /// Find a field mapping by target field name
    pub fn find_field_mapping(&self, target_field: &str) -> Option<&FieldMapping> {
        self.field_mappings
            .iter()
            .find(|m| m.target_field == target_field)
    }

    /// Find a field mapping by target field name (mutable)
    pub fn find_field_mapping_mut(&mut self, target_field: &str) -> Option<&mut FieldMapping> {
        self.field_mappings
            .iter_mut()
            .find(|m| m.target_field == target_field)
    }

    /// Get the number of field mappings
    pub fn field_count(&self) -> usize {
        self.field_mappings.len()
    }
}

/// Mapping for a single target field
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldMapping {
    /// Database ID (None if not yet persisted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Target field logical name
    pub target_field: String,
    /// Transform to produce the target value
    pub transform: Transform,
}

impl FieldMapping {
    /// Create a new field mapping
    pub fn new(target_field: impl Into<String>, transform: Transform) -> Self {
        FieldMapping {
            id: None,
            target_field: target_field.into(),
            transform,
        }
    }

    /// Create a simple copy mapping (source field = target field)
    pub fn copy(field: impl Into<String>) -> Self {
        let field = field.into();
        FieldMapping::new(
            field.clone(),
            Transform::Copy {
                source_path: super::FieldPath::simple(field),
            },
        )
    }

    /// Create a copy mapping with different source and target fields
    pub fn copy_from(target_field: impl Into<String>, source_field: impl Into<String>) -> Self {
        FieldMapping::new(
            target_field,
            Transform::Copy {
                source_path: super::FieldPath::simple(source_field),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_mappings_by_priority() {
        let mut config = TransferConfig::new("test", "dev", "prod");
        config.add_entity_mapping(EntityMapping::same_entity("contact", 2));
        config.add_entity_mapping(EntityMapping::same_entity("account", 1));
        config.add_entity_mapping(EntityMapping::same_entity("opportunity", 3));

        let sorted = config.entity_mappings_by_priority();
        assert_eq!(sorted[0].source_entity, "account");
        assert_eq!(sorted[1].source_entity, "contact");
        assert_eq!(sorted[2].source_entity, "opportunity");
    }
}
