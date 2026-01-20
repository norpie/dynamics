//! Core data types for the Entity Sync App
//!
//! These types define the data model for syncing entities between
//! Dynamics 365 environments, including schema diffing, dependency
//! ordering, and operation generation.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Represents the category of an entity in the dependency graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DependencyCategory {
    /// Entity has no lookups to other selected entities
    Standalone,
    /// Entity has lookups to other selected entities
    Dependent,
    /// Entity has lookups to 2+ selected entities (N:M relationship table)
    Junction,
}

impl DependencyCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Standalone => "Standalone",
            Self::Dependent => "Dependent",
            Self::Junction => "Junction",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Standalone => "○",
            Self::Dependent => "→",
            Self::Junction => "⬌",
        }
    }
}

/// Information about a lookup field on an entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupInfo {
    /// The field logical name (e.g., "nrq_fund")
    pub field_name: String,
    /// The field schema name with proper casing (e.g., "nrq_Fund")
    /// Used for @odata.bind annotations which require schema name casing
    pub schema_name: String,
    /// The target entity (e.g., "account")
    pub target_entity: String,
    /// Whether the target entity is in the selected set
    pub is_internal: bool,
}

/// Information about an incoming reference (another entity has a lookup pointing to us)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingReferenceInfo {
    /// The entity that has the lookup field
    pub referencing_entity: String,
    /// The lookup field name on the referencing entity
    pub referencing_attribute: String,
    /// Whether the referencing entity is in the selected set
    pub is_internal: bool,
}

/// N:N relationship metadata for junction entities (is_intersect=true)
/// Used to build AssociateRef operations during sync execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NNRelationshipInfo {
    /// Navigation property name for AssociateRef (e.g., "accountleads_association")
    pub navigation_property: String,
    /// Parent entity logical name (Entity1 in the relationship)
    pub parent_entity: String,
    /// Parent entity set name for API URL (e.g., "accounts")
    pub parent_entity_set: String,
    /// FK field name in junction for parent (Entity1IntersectAttribute, e.g., "accountid")
    pub parent_fk_field: String,
    /// Target entity logical name (Entity2 in the relationship)
    pub target_entity: String,
    /// Target entity set name for API URL (e.g., "leads")
    pub target_entity_set: String,
    /// FK field name in junction for target (Entity2IntersectAttribute, e.g., "leadid")
    pub target_fk_field: String,
}

/// Metadata about an entity in the sync context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEntityInfo {
    /// Entity logical name
    pub logical_name: String,
    /// Entity display name
    pub display_name: Option<String>,
    /// Entity set name for API URLs (e.g., "accounts" for "account")
    pub entity_set_name: String,
    /// Primary name attribute (field used for display name of records)
    pub primary_name_attribute: Option<String>,
    /// Dependency category (standalone, dependent, junction)
    pub category: DependencyCategory,
    /// Lookup fields on this entity (outgoing references)
    pub lookups: Vec<LookupInfo>,
    /// Incoming references (other entities that have lookups pointing to this entity)
    pub incoming_references: Vec<IncomingReferenceInfo>,
    /// Entities that depend on this entity (have lookups to it)
    pub dependents: Vec<String>,
    /// Priority in the topological sort (lower = process first for inserts)
    pub insert_priority: u32,
    /// Priority in the topological sort for deletes (reverse of insert)
    pub delete_priority: u32,
    /// N:N relationship info (only for junction entities with is_intersect=true)
    pub nn_relationship: Option<NNRelationshipInfo>,
}

/// Represents the status of a field across two schemas
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldSyncStatus {
    /// Field exists in both schemas with matching type
    InBoth,
    /// Field exists only in origin (will be added to target)
    OriginOnly,
    /// Field exists only in target (will be reported for manual review)
    TargetOnly,
    /// Field exists in both but with different types (requires attention)
    TypeMismatch {
        origin_type: String,
        target_type: String,
    },
}

impl FieldSyncStatus {
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::InBoth => "✓",
            Self::OriginOnly => "+",
            Self::TargetOnly => "⚠",
            Self::TypeMismatch { .. } => "⚡",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::InBoth => "Match",
            Self::OriginOnly => "Add",
            Self::TargetOnly => "Manual",
            Self::TypeMismatch { .. } => "Type Mismatch",
        }
    }
}

/// Represents a field in the schema diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDiffEntry {
    /// Field logical name
    pub logical_name: String,
    /// Field display name
    pub display_name: Option<String>,
    /// Field type as string
    pub field_type: String,
    /// Sync status
    pub status: FieldSyncStatus,
    /// Whether this is a system field (should be skipped)
    pub is_system_field: bool,
    /// Full attribute metadata from origin (for CreateAttribute operation)
    pub origin_metadata: Option<Value>,
}

/// Schema diff result for a single entity
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntitySchemaDiff {
    /// Entity logical name
    pub entity_name: String,
    /// Fields that exist in both schemas
    pub fields_in_both: Vec<FieldDiffEntry>,
    /// Fields only in origin (will be added)
    pub fields_to_add: Vec<FieldDiffEntry>,
    /// Fields only in target (report for manual review)
    pub fields_target_only: Vec<FieldDiffEntry>,
    /// Fields with type mismatches
    pub fields_type_mismatch: Vec<FieldDiffEntry>,
}

impl EntitySchemaDiff {
    /// Check if there are any changes needed
    pub fn has_changes(&self) -> bool {
        !self.fields_to_add.is_empty()
            || !self.fields_target_only.is_empty()
            || !self.fields_type_mismatch.is_empty()
    }

    /// Count of fields that will be added
    pub fn add_count(&self) -> usize {
        self.fields_to_add.len()
    }

    /// Count of fields that need manual attention
    pub fn manual_count(&self) -> usize {
        self.fields_target_only.len() + self.fields_type_mismatch.len()
    }
}

/// A target record with ID and display name (for deletion preview)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TargetRecord {
    /// Record ID
    pub id: String,
    /// Display name from primary name attribute
    pub name: Option<String>,
    /// For junction entities: FK value to parent entity (Entity1)
    /// Used for DisassociateRef operations
    pub junction_parent_id: Option<String>,
    /// For junction entities: FK value to target entity (Entity2)
    /// Used for DisassociateRef operations
    pub junction_target_id: Option<String>,
}

/// Data preview for an entity
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityDataPreview {
    /// Entity logical name
    pub entity_name: String,
    /// Number of active records in origin
    pub origin_count: usize,
    /// Number of records in target (all states)
    pub target_count: usize,
    /// Actual records from origin (active only) - to be inserted
    pub origin_records: Vec<Value>,
    /// Records from target - to be deleted (with ID and name)
    pub target_records: Vec<TargetRecord>,
    /// For junction entities: raw target records with FK values
    /// Used to build DisassociateRef operations (need FK values, not just record ID)
    pub junction_target_raw: Vec<Value>,
}

/// Information about an external lookup that will be nulled
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NulledLookupInfo {
    /// The entity containing the lookup
    pub entity_name: String,
    /// The field name
    pub field_name: String,
    /// The target entity (which is not in the sync set)
    pub target_entity: String,
    /// Number of records that have this lookup set
    pub affected_count: usize,
}

/// Complete sync plan for a single entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySyncPlan {
    /// Entity info with dependency category
    pub entity_info: SyncEntityInfo,
    /// Schema diff results
    pub schema_diff: EntitySchemaDiff,
    /// Data preview (record counts)
    pub data_preview: EntityDataPreview,
    /// External lookups that will be nulled
    pub nulled_lookups: Vec<NulledLookupInfo>,
}

/// Overall sync plan for all selected entities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncPlan {
    /// Origin environment name
    pub origin_env: String,
    /// Target environment name
    pub target_env: String,
    /// Plans for each entity, ordered by insert priority
    pub entity_plans: Vec<EntitySyncPlan>,
    /// Junction entity candidates that were auto-detected
    pub detected_junctions: Vec<String>,
    /// Whether schema changes are needed
    pub has_schema_changes: bool,
    /// Total records to delete
    pub total_delete_count: usize,
    /// Total records to insert
    pub total_insert_count: usize,
}

impl SyncPlan {
    /// Get entities in delete order (reverse of insert order)
    pub fn delete_order(&self) -> Vec<&EntitySyncPlan> {
        let mut plans: Vec<_> = self.entity_plans.iter().collect();
        plans.sort_by(|a, b| {
            b.entity_info
                .delete_priority
                .cmp(&a.entity_info.delete_priority)
        });
        plans
    }

    /// Get entities in insert order
    pub fn insert_order(&self) -> Vec<&EntitySyncPlan> {
        let mut plans: Vec<_> = self.entity_plans.iter().collect();
        plans.sort_by(|a, b| {
            a.entity_info
                .insert_priority
                .cmp(&b.entity_info.insert_priority)
        });
        plans
    }
}

/// System fields that should be skipped during sync
/// These are auto-populated by Dynamics 365
pub const SYSTEM_FIELDS: &[&str] = &[
    "createdby",
    "createdon",
    "createdonbehalfby",
    "modifiedby",
    "modifiedon",
    "modifiedonbehalfby",
    "ownerid",
    "owningbusinessunit",
    "owningteam",
    "owninguser",
    "versionnumber",
    "importsequencenumber",
    "overriddencreatedon",
    "timezoneruleversionnumber",
    "utcconversiontimezonecode",
];

/// Check if a field name is a system field
pub fn is_system_field(field_name: &str) -> bool {
    SYSTEM_FIELDS.contains(&field_name)
}

/// Represents the current step in the sync wizard
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncStep {
    #[default]
    EnvironmentSelect,
    EntitySelect,
    Analysis,
    DiffReview,
    Confirm,
}

impl SyncStep {
    pub fn number(&self) -> u8 {
        match self {
            Self::EnvironmentSelect => 1,
            Self::EntitySelect => 2,
            Self::Analysis => 3,
            Self::DiffReview => 4,
            Self::Confirm => 5,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::EnvironmentSelect => "Select Environments",
            Self::EntitySelect => "Select Entities",
            Self::Analysis => "Analyzing",
            Self::DiffReview => "Review Diff",
            Self::Confirm => "Confirm & Execute",
        }
    }

    pub fn next(&self) -> Option<Self> {
        match self {
            Self::EnvironmentSelect => Some(Self::EntitySelect),
            Self::EntitySelect => Some(Self::Analysis),
            Self::Analysis => Some(Self::DiffReview),
            Self::DiffReview => Some(Self::Confirm),
            Self::Confirm => None,
        }
    }

    pub fn prev(&self) -> Option<Self> {
        match self {
            Self::EnvironmentSelect => None,
            Self::EntitySelect => Some(Self::EnvironmentSelect),
            Self::Analysis => Some(Self::EntitySelect),
            Self::DiffReview => Some(Self::EntitySelect), // Skip analysis on back
            Self::Confirm => Some(Self::DiffReview),
        }
    }
}

/// Excel report data structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncReport {
    /// When the sync was executed
    pub sync_date: String,
    /// Origin environment
    pub origin_env: String,
    /// Target environment
    pub target_env: String,
    /// Entities that were synced
    pub synced_entities: Vec<String>,
    /// Summary record counts
    pub summary: SyncSummary,
    /// Fields in target that need manual review (candidate for deletion)
    pub manual_review_fields: Vec<ManualReviewField>,
    /// Lookups that were nulled
    pub nulled_lookups: Vec<NulledLookupInfo>,
    /// Errors that occurred
    pub errors: Vec<SyncError>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncSummary {
    pub entities_synced: usize,
    pub records_deleted: usize,
    pub records_inserted: usize,
    pub fields_added: usize,
    pub fields_needing_review: usize,
    pub lookups_nulled: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualReviewField {
    pub entity_name: String,
    pub field_name: String,
    pub field_type: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncError {
    pub entity_name: String,
    pub operation: String,
    pub record_id: Option<String>,
    pub error_message: String,
}
