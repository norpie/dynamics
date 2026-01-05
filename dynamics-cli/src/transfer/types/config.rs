//! Transfer configuration types

use serde::{Deserialize, Serialize};

use super::{Condition, FieldPath, Resolver, Transform};

/// Mode for transfer configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferMode {
    /// Declarative field mappings (default)
    #[default]
    Declarative,
    /// Lua script-based transformation
    Lua,
}

impl TransferMode {
    /// Get display label for UI
    pub fn label(&self) -> &'static str {
        match self {
            TransferMode::Declarative => "Declarative",
            TransferMode::Lua => "Lua Script",
        }
    }

    /// Get all variants for UI selection
    pub fn all_variants() -> &'static [TransferMode] {
        &[TransferMode::Declarative, TransferMode::Lua]
    }

    /// Convert from index (for UI selection)
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => TransferMode::Declarative,
            1 => TransferMode::Lua,
            _ => TransferMode::Declarative,
        }
    }

    /// Convert to index (for UI selection)
    pub fn to_index(&self) -> usize {
        match self {
            TransferMode::Declarative => 0,
            TransferMode::Lua => 1,
        }
    }

    /// Convert from database string
    pub fn from_db_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "lua" => TransferMode::Lua,
            _ => TransferMode::Declarative,
        }
    }

    /// Convert to database string
    pub fn to_db_str(&self) -> &'static str {
        match self {
            TransferMode::Declarative => "declarative",
            TransferMode::Lua => "lua",
        }
    }
}

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

    /// Convert to OperationFilter (for migration)
    pub fn to_operation_filter(&self) -> OperationFilter {
        match self {
            OrphanHandling::Ignore => OperationFilter::default(),
            OrphanHandling::Delete => OperationFilter {
                deletes: true,
                ..Default::default()
            },
            OrphanHandling::Deactivate => OperationFilter {
                deactivates: true,
                ..Default::default()
            },
        }
    }
}

/// Filter controlling which operation types are executed for an entity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationFilter {
    /// Allow creating new records in target
    #[serde(default = "default_true")]
    pub creates: bool,
    /// Allow updating existing records in target
    #[serde(default = "default_true")]
    pub updates: bool,
    /// Allow deleting target-only records (orphans)
    #[serde(default)]
    pub deletes: bool,
    /// Allow deactivating target-only records (orphans) - statecode = 1
    #[serde(default)]
    pub deactivates: bool,
}

fn default_true() -> bool {
    true
}

impl Default for OperationFilter {
    fn default() -> Self {
        OperationFilter {
            creates: true,
            updates: true,
            deletes: false,
            deactivates: false,
        }
    }
}

impl OperationFilter {
    /// Get a human-readable label summarizing the filter
    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if self.creates {
            parts.push("C");
        }
        if self.updates {
            parts.push("U");
        }
        if self.deletes {
            parts.push("D");
        }
        if self.deactivates {
            parts.push("X");
        }
        if parts.is_empty() {
            "None".to_string()
        } else {
            parts.join("/")
        }
    }

    /// Check if any target-only (orphan) operation is enabled
    pub fn has_orphan_action(&self) -> bool {
        self.deletes || self.deactivates
    }

    /// Get the orphan action type (deletes takes precedence)
    pub fn orphan_action(&self) -> Option<OrphanAction> {
        if self.deletes {
            Some(OrphanAction::Delete)
        } else if self.deactivates {
            Some(OrphanAction::Deactivate)
        } else {
            None
        }
    }
}

/// Action to take on orphan (target-only) records
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrphanAction {
    Delete,
    Deactivate,
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
    /// Transform mode (declarative or lua)
    #[serde(default)]
    pub mode: TransferMode,
    /// Lua script content (only used when mode == Lua)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lua_script: Option<String>,
    /// Original file path for Lua script (for "refresh from file" feature)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lua_script_path: Option<String>,
    /// Entity mappings (resolvers are now per-entity, only used when mode == Declarative)
    pub entity_mappings: Vec<EntityMapping>,
}

impl TransferConfig {
    /// Create a new transfer config (declarative mode)
    pub fn new(name: impl Into<String>, source_env: impl Into<String>, target_env: impl Into<String>) -> Self {
        TransferConfig {
            id: None,
            name: name.into(),
            source_env: source_env.into(),
            target_env: target_env.into(),
            mode: TransferMode::Declarative,
            lua_script: None,
            lua_script_path: None,
            entity_mappings: Vec::new(),
        }
    }

    /// Create a new Lua mode transfer config
    pub fn new_lua(name: impl Into<String>, source_env: impl Into<String>, target_env: impl Into<String>) -> Self {
        TransferConfig {
            id: None,
            name: name.into(),
            source_env: source_env.into(),
            target_env: target_env.into(),
            mode: TransferMode::Lua,
            lua_script: None,
            lua_script_path: None,
            entity_mappings: Vec::new(),
        }
    }

    /// Check if this config uses Lua mode
    pub fn is_lua_mode(&self) -> bool {
        self.mode == TransferMode::Lua
    }

    /// Check if this config uses declarative mode
    pub fn is_declarative_mode(&self) -> bool {
        self.mode == TransferMode::Declarative
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
            mode: TransferMode::Declarative,
            lua_script: None,
            lua_script_path: None,
            entity_mappings: Vec::new(),
        }
    }
}

/// Filter for source records - only matching records are processed
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceFilter {
    /// Field to evaluate
    pub field_path: FieldPath,
    /// Condition to check
    pub condition: Condition,
}

impl SourceFilter {
    /// Create a new source filter
    pub fn new(field_path: FieldPath, condition: Condition) -> Self {
        SourceFilter { field_path, condition }
    }

    /// Evaluate filter against a record - returns true if record should be processed
    pub fn matches(&self, record: &serde_json::Value) -> bool {
        use crate::transfer::transform::resolve_path;
        let value = resolve_path(record, &self.field_path);
        let result = self.condition.evaluate(&value);
        log::debug!(
            "Filter check: field={} actual={:?} condition={} result={}",
            self.field_path,
            value,
            self.condition,
            result
        );
        result
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
    /// Filter controlling which operations are executed
    #[serde(default)]
    pub operation_filter: OperationFilter,
    /// Optional filter for source records - only matching records are processed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_filter: Option<SourceFilter>,
    /// Resolvers for lookup field resolution (scoped to this entity)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolvers: Vec<Resolver>,
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
            operation_filter: OperationFilter::default(),
            source_filter: None,
            resolvers: Vec::new(),
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

    /// Add a resolver
    pub fn add_resolver(&mut self, resolver: Resolver) {
        self.resolvers.push(resolver);
    }

    /// Find a resolver by name
    pub fn find_resolver(&self, name: &str) -> Option<&Resolver> {
        self.resolvers.iter().find(|r| r.name == name)
    }

    /// Find a resolver by name (mutable)
    pub fn find_resolver_mut(&mut self, name: &str) -> Option<&mut Resolver> {
        self.resolvers.iter_mut().find(|r| r.name == name)
    }

    /// Remove a resolver by name, returns true if found and removed
    pub fn remove_resolver(&mut self, name: &str) -> bool {
        let len_before = self.resolvers.len();
        self.resolvers.retain(|r| r.name != name);
        self.resolvers.len() != len_before
    }

    /// Rename a resolver and update all references in field mappings
    pub fn rename_resolver(&mut self, old_name: &str, new_name: &str) -> bool {
        // Find and rename the resolver
        let found = if let Some(resolver) = self.find_resolver_mut(old_name) {
            resolver.name = new_name.to_string();
            true
        } else {
            false
        };

        if found {
            // Update all references in field mappings within this entity
            for field_mapping in &mut self.field_mappings {
                if let super::Transform::Copy { resolver, .. } = &mut field_mapping.transform {
                    if resolver.as_deref() == Some(old_name) {
                        *resolver = Some(new_name.to_string());
                    }
                }
            }
        }

        found
    }

    /// Check if a resolver name is unique (not already used in this entity)
    pub fn is_resolver_name_unique(&self, name: &str) -> bool {
        !self.resolvers.iter().any(|r| r.name == name)
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
                resolver: None,
            },
        )
    }

    /// Create a copy mapping with different source and target fields
    pub fn copy_from(target_field: impl Into<String>, source_field: impl Into<String>) -> Self {
        FieldMapping::new(
            target_field,
            Transform::Copy {
                source_path: super::FieldPath::simple(source_field),
                resolver: None,
            },
        )
    }

    /// Create a copy mapping with a resolver
    pub fn copy_with_resolver(
        target_field: impl Into<String>,
        source_field: impl Into<String>,
        resolver_name: impl Into<String>,
    ) -> Self {
        FieldMapping::new(
            target_field,
            Transform::Copy {
                source_path: super::FieldPath::simple(source_field),
                resolver: Some(resolver_name.into()),
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
