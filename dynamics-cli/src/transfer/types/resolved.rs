//! Resolved transfer state after transforms have been applied

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Value;
use super::lookup::LookupBindingContext;

/// A fully resolved transfer ready for queue/execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedTransfer {
    /// Config name this was generated from
    pub config_name: String,
    /// Source environment
    pub source_env: String,
    /// Target environment
    pub target_env: String,
    /// Resolved entities in priority order
    pub entities: Vec<ResolvedEntity>,
}

impl ResolvedTransfer {
    /// Create a new resolved transfer
    pub fn new(
        config_name: impl Into<String>,
        source_env: impl Into<String>,
        target_env: impl Into<String>,
    ) -> Self {
        ResolvedTransfer {
            config_name: config_name.into(),
            source_env: source_env.into(),
            target_env: target_env.into(),
            entities: Vec::new(),
        }
    }

    /// Add a resolved entity
    pub fn add_entity(&mut self, entity: ResolvedEntity) {
        self.entities.push(entity);
    }

    /// Get total record count across all entities
    pub fn total_records(&self) -> usize {
        self.entities.iter().map(|e| e.records.len()).sum()
    }

    /// Get count of records by action
    pub fn count_by_action(&self, action: RecordAction) -> usize {
        self.entities
            .iter()
            .flat_map(|e| e.records.iter())
            .filter(|r| r.action == action)
            .count()
    }

    /// Get total create count (new records to insert)
    pub fn create_count(&self) -> usize {
        self.count_by_action(RecordAction::Create)
    }

    /// Get total update count (existing records to modify)
    pub fn update_count(&self) -> usize {
        self.count_by_action(RecordAction::Update)
    }

    /// Get total delete count
    pub fn delete_count(&self) -> usize {
        self.count_by_action(RecordAction::Delete)
    }

    /// Get total deactivate count
    pub fn deactivate_count(&self) -> usize {
        self.count_by_action(RecordAction::Deactivate)
    }

    /// Get total no-change count (records that match target)
    pub fn nochange_count(&self) -> usize {
        self.count_by_action(RecordAction::NoChange)
    }

    /// Get total target-only count (records in target but not source)
    pub fn target_only_count(&self) -> usize {
        self.count_by_action(RecordAction::TargetOnly)
    }

    /// Get total skip count
    pub fn skip_count(&self) -> usize {
        self.count_by_action(RecordAction::Skip)
    }

    /// Get total error count
    pub fn error_count(&self) -> usize {
        self.count_by_action(RecordAction::Error)
    }

    /// Check if there are any errors blocking execution
    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }

    /// Find entity by name
    pub fn find_entity(&self, entity_name: &str) -> Option<&ResolvedEntity> {
        self.entities.iter().find(|e| e.entity_name == entity_name)
    }

    /// Find entity by name (mutable)
    pub fn find_entity_mut(&mut self, entity_name: &str) -> Option<&mut ResolvedEntity> {
        self.entities
            .iter_mut()
            .find(|e| e.entity_name == entity_name)
    }
}

/// Resolved records for a single entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedEntity {
    /// Target entity logical name
    pub entity_name: String,
    /// Execution priority (from config)
    pub priority: u32,
    /// Primary key field name for upsert
    pub primary_key_field: String,
    /// All field names in order (for table display)
    pub field_names: Vec<String>,
    /// Filter controlling which operations are executed
    #[serde(default)]
    pub operation_filter: super::OperationFilter,
    /// Resolved records
    pub records: Vec<ResolvedRecord>,
    /// Set of record IDs that have been manually edited
    #[serde(default)]
    pub dirty_record_ids: HashSet<Uuid>,
    /// Lookup binding context for @odata.bind generation
    /// Not serialized - rebuilt from metadata when needed
    #[serde(skip)]
    pub lookup_context: Option<LookupBindingContext>,
    /// Entity set name for API calls (e.g., "cgk_requests" vs logical name "cgk_request")
    /// Not serialized - set from metadata when building queue items
    #[serde(skip)]
    pub entity_set_name: Option<String>,
}

impl ResolvedEntity {
    /// Create a new resolved entity
    pub fn new(
        entity_name: impl Into<String>,
        priority: u32,
        primary_key_field: impl Into<String>,
    ) -> Self {
        ResolvedEntity {
            entity_name: entity_name.into(),
            priority,
            primary_key_field: primary_key_field.into(),
            field_names: Vec::new(),
            operation_filter: super::OperationFilter::default(),
            records: Vec::new(),
            dirty_record_ids: HashSet::new(),
            lookup_context: None,
            entity_set_name: None,
        }
    }

    /// Set the operation filter
    pub fn set_operation_filter(&mut self, filter: super::OperationFilter) {
        self.operation_filter = filter;
    }

    /// Set the lookup binding context
    pub fn set_lookup_context(&mut self, ctx: LookupBindingContext) {
        self.lookup_context = Some(ctx);
    }

    /// Set the entity set name for API calls
    pub fn set_entity_set_name(&mut self, name: String) {
        self.entity_set_name = Some(name);
    }

    /// Add a resolved record
    pub fn add_record(&mut self, record: ResolvedRecord) {
        self.records.push(record);
    }

    /// Set field names (column order)
    pub fn set_field_names(&mut self, names: Vec<String>) {
        self.field_names = names;
    }

    /// Get count by action
    pub fn count_by_action(&self, action: RecordAction) -> usize {
        self.records.iter().filter(|r| r.action == action).count()
    }

    /// Get create count (new records)
    pub fn create_count(&self) -> usize {
        self.count_by_action(RecordAction::Create)
    }

    /// Get update count (existing records to modify)
    pub fn update_count(&self) -> usize {
        self.count_by_action(RecordAction::Update)
    }

    /// Get delete count
    pub fn delete_count(&self) -> usize {
        self.count_by_action(RecordAction::Delete)
    }

    /// Get deactivate count
    pub fn deactivate_count(&self) -> usize {
        self.count_by_action(RecordAction::Deactivate)
    }

    /// Get no-change count
    pub fn nochange_count(&self) -> usize {
        self.count_by_action(RecordAction::NoChange)
    }

    /// Get target-only count (records in target but not source)
    pub fn target_only_count(&self) -> usize {
        self.count_by_action(RecordAction::TargetOnly)
    }

    /// Get skip count
    pub fn skip_count(&self) -> usize {
        self.count_by_action(RecordAction::Skip)
    }

    /// Get error count
    pub fn error_count(&self) -> usize {
        self.count_by_action(RecordAction::Error)
    }

    /// Find record by source ID
    pub fn find_record(&self, source_id: Uuid) -> Option<&ResolvedRecord> {
        self.records.iter().find(|r| r.source_id == source_id)
    }

    /// Find record by source ID (mutable)
    pub fn find_record_mut(&mut self, source_id: Uuid) -> Option<&mut ResolvedRecord> {
        self.records.iter_mut().find(|r| r.source_id == source_id)
    }

    /// Mark a record as dirty (user-edited)
    pub fn mark_dirty(&mut self, source_id: Uuid) {
        self.dirty_record_ids.insert(source_id);
    }

    /// Check if a record is dirty
    pub fn is_dirty(&self, source_id: Uuid) -> bool {
        self.dirty_record_ids.contains(&source_id)
    }

    /// Get all dirty records
    pub fn dirty_records(&self) -> Vec<&ResolvedRecord> {
        self.records
            .iter()
            .filter(|r| self.dirty_record_ids.contains(&r.source_id))
            .collect()
    }

    /// Clear dirty state for a record
    pub fn clear_dirty(&mut self, source_id: Uuid) {
        self.dirty_record_ids.remove(&source_id);
    }

    /// Clear all dirty states
    pub fn clear_all_dirty(&mut self) {
        self.dirty_record_ids.clear();
    }
}

/// A single resolved record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedRecord {
    /// Action to take for this record
    pub action: RecordAction,
    /// Source record ID (also used as target ID since IDs are preserved)
    pub source_id: Uuid,
    /// Resolved field values
    pub fields: HashMap<String, Value>,
    /// Fields that differ from target (for Update action - enables partial updates)
    /// When Some, only these fields are sent in the update payload
    /// When None, all fields are sent (backwards compatible)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_fields: Option<HashSet<String>>,
    /// Error message if transform failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ResolvedRecord {
    /// Create a new record to be created in target (doesn't exist yet)
    pub fn create(source_id: Uuid, fields: HashMap<String, Value>) -> Self {
        ResolvedRecord {
            action: RecordAction::Create,
            source_id,
            fields,
            changed_fields: None,
            error: None,
        }
    }

    /// Create a new record to be updated in target (exists but differs)
    /// Sends all fields in the update payload
    pub fn update(source_id: Uuid, fields: HashMap<String, Value>) -> Self {
        ResolvedRecord {
            action: RecordAction::Update,
            source_id,
            fields,
            changed_fields: None,
            error: None,
        }
    }

    /// Create a new record to be updated with only specific changed fields
    /// Only the changed fields will be sent in the update payload (partial update)
    pub fn update_partial(
        source_id: Uuid,
        fields: HashMap<String, Value>,
        changed_fields: HashSet<String>,
    ) -> Self {
        ResolvedRecord {
            action: RecordAction::Update,
            source_id,
            fields,
            changed_fields: Some(changed_fields),
            error: None,
        }
    }

    /// Create an error record
    pub fn error(source_id: Uuid, error: impl Into<String>) -> Self {
        ResolvedRecord {
            action: RecordAction::Error,
            source_id,
            fields: HashMap::new(),
            changed_fields: None,
            error: Some(error.into()),
        }
    }

    /// Create an error record with partial fields
    pub fn error_with_fields(
        source_id: Uuid,
        fields: HashMap<String, Value>,
        error: impl Into<String>,
    ) -> Self {
        ResolvedRecord {
            action: RecordAction::Error,
            source_id,
            fields,
            changed_fields: None,
            error: Some(error.into()),
        }
    }

    /// Create a delete record
    pub fn delete(source_id: Uuid) -> Self {
        ResolvedRecord {
            action: RecordAction::Delete,
            source_id,
            fields: HashMap::new(),
            changed_fields: None,
            error: None,
        }
    }

    /// Create a deactivate record
    pub fn deactivate(source_id: Uuid) -> Self {
        ResolvedRecord {
            action: RecordAction::Deactivate,
            source_id,
            fields: HashMap::new(),
            changed_fields: None,
            error: None,
        }
    }

    /// Create a skipped record
    pub fn skip(source_id: Uuid, fields: HashMap<String, Value>) -> Self {
        ResolvedRecord {
            action: RecordAction::Skip,
            source_id,
            fields,
            changed_fields: None,
            error: None,
        }
    }

    /// Create a no-change record (target already matches)
    pub fn nochange(source_id: Uuid, fields: HashMap<String, Value>) -> Self {
        ResolvedRecord {
            action: RecordAction::NoChange,
            source_id,
            fields,
            changed_fields: None,
            error: None,
        }
    }

    /// Create a target-only record (exists in target but not source)
    pub fn target_only(target_id: Uuid, fields: HashMap<String, Value>) -> Self {
        ResolvedRecord {
            action: RecordAction::TargetOnly,
            source_id: target_id, // Using source_id field to store target ID
            fields,
            changed_fields: None,
            error: None,
        }
    }

    /// Check if this record will be created
    pub fn is_create(&self) -> bool {
        self.action == RecordAction::Create
    }

    /// Check if this record will be updated
    pub fn is_update(&self) -> bool {
        self.action == RecordAction::Update
    }

    /// Check if this record will be deleted
    pub fn is_delete(&self) -> bool {
        self.action == RecordAction::Delete
    }

    /// Check if this record will be deactivated
    pub fn is_deactivate(&self) -> bool {
        self.action == RecordAction::Deactivate
    }

    /// Check if this record has no changes
    pub fn is_nochange(&self) -> bool {
        self.action == RecordAction::NoChange
    }

    /// Check if this record exists only in target
    pub fn is_target_only(&self) -> bool {
        self.action == RecordAction::TargetOnly
    }

    /// Check if this record has an error
    pub fn is_error(&self) -> bool {
        self.action == RecordAction::Error
    }

    /// Check if this record is skipped
    pub fn is_skip(&self) -> bool {
        self.action == RecordAction::Skip
    }

    /// Get a field value
    pub fn get_field(&self, field: &str) -> Option<&Value> {
        self.fields.get(field)
    }

    /// Set a field value
    pub fn set_field(&mut self, field: impl Into<String>, value: Value) {
        self.fields.insert(field.into(), value);
    }

    /// Mark as skip
    pub fn mark_skip(&mut self) {
        self.action = RecordAction::Skip;
        self.error = None;
    }

    /// Mark as create (clears error)
    pub fn mark_create(&mut self) {
        self.action = RecordAction::Create;
        self.error = None;
    }

    /// Mark as update (clears error)
    pub fn mark_update(&mut self) {
        self.action = RecordAction::Update;
        self.error = None;
    }

    /// Convert fields to JSON for API
    pub fn to_json(&self) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        for (key, value) in &self.fields {
            obj.insert(key.clone(), value.to_json());
        }
        serde_json::Value::Object(obj)
    }
}

/// Action to take for a resolved record
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordAction {
    /// Create new record in target (doesn't exist yet)
    Create,
    /// Update existing record in target (exists but differs)
    Update,
    /// Delete record from target
    Delete,
    /// Deactivate record in target (set statecode = 1)
    Deactivate,
    /// No changes needed (target already matches)
    NoChange,
    /// Record exists only in target (not in source) - action depends on OperationFilter config
    TargetOnly,
    /// Skipped by user
    Skip,
    /// Transform error (cannot proceed)
    Error,
}

impl std::fmt::Display for RecordAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordAction::Create => write!(f, "create"),
            RecordAction::Update => write!(f, "update"),
            RecordAction::Delete => write!(f, "delete"),
            RecordAction::Deactivate => write!(f, "deactivate"),
            RecordAction::NoChange => write!(f, "nochange"),
            RecordAction::TargetOnly => write!(f, "target-only"),
            RecordAction::Skip => write!(f, "skip"),
            RecordAction::Error => write!(f, "error"),
        }
    }
}

impl Default for RecordAction {
    fn default() -> Self {
        RecordAction::Create
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolved_entity_counts() {
        let mut entity = ResolvedEntity::new("account", 1, "accountid");
        entity.add_record(ResolvedRecord::create(Uuid::new_v4(), HashMap::new()));
        entity.add_record(ResolvedRecord::update(Uuid::new_v4(), HashMap::new()));
        entity.add_record(ResolvedRecord::skip(Uuid::new_v4(), HashMap::new()));
        entity.add_record(ResolvedRecord::error(Uuid::new_v4(), "test error"));

        assert_eq!(entity.create_count(), 1);
        assert_eq!(entity.update_count(), 1);
        assert_eq!(entity.skip_count(), 1);
        assert_eq!(entity.error_count(), 1);
    }

    #[test]
    fn test_resolved_transfer_aggregates_across_entities() {
        let mut transfer = ResolvedTransfer::new("test", "dev", "prod");

        let mut accounts = ResolvedEntity::new("account", 1, "accountid");
        accounts.add_record(ResolvedRecord::create(Uuid::new_v4(), HashMap::new()));
        accounts.add_record(ResolvedRecord::update(Uuid::new_v4(), HashMap::new()));

        let mut contacts = ResolvedEntity::new("contact", 2, "contactid");
        contacts.add_record(ResolvedRecord::create(Uuid::new_v4(), HashMap::new()));
        contacts.add_record(ResolvedRecord::error(Uuid::new_v4(), "error"));

        transfer.add_entity(accounts);
        transfer.add_entity(contacts);

        assert_eq!(transfer.total_records(), 4);
        assert_eq!(transfer.create_count(), 2);
        assert_eq!(transfer.update_count(), 1);
        assert_eq!(transfer.error_count(), 1);
        assert!(transfer.has_errors());
    }

    #[test]
    fn test_dirty_tracking() {
        let mut entity = ResolvedEntity::new("account", 1, "accountid");
        let id = Uuid::new_v4();
        entity.add_record(ResolvedRecord::create(id, HashMap::new()));

        assert!(!entity.is_dirty(id));
        entity.mark_dirty(id);
        assert!(entity.is_dirty(id));
        entity.clear_dirty(id);
        assert!(!entity.is_dirty(id));
    }
}
