//! Operation builder for the Entity Sync App
//!
//! Converts a SyncPlan into an ordered list of API operations.
//! Operations are ordered to respect entity dependencies:
//! - Deletes: dependents before dependencies (reverse topological order)
//! - Inserts: dependencies before dependents (topological order)

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::operations::Operation;
use super::super::types::{EntitySyncPlan, FieldDiffEntry, NulledLookupInfo, SyncPlan, SYSTEM_FIELDS};

/// Context for cleaning records before insertion
pub struct InsertCleaningContext<'a> {
    /// Map from lookup field name to (schema_name, entity_set_name)
    /// e.g., "nrq_fund" -> ("nrq_Fund", "nrq_funds")
    /// The schema_name is used for @odata.bind which requires proper casing
    pub internal_lookups: HashMap<String, (String, String)>,
    /// External lookups to null
    pub nulled_lookups: &'a [NulledLookupInfo],
    /// Fields that exist in target schema (only these will be included in payload)
    pub target_fields: HashSet<String>,
}

/// A single sync operation to be executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOperation {
    /// Operation ID for tracking
    pub id: String,
    /// Entity logical name
    pub entity_name: String,
    /// Type of operation
    pub operation_type: OperationType,
    /// Priority (lower = execute first)
    pub priority: u32,
    /// Record ID (for delete operations)
    pub record_id: Option<String>,
    /// Record data (for create operations)
    pub record_data: Option<Value>,
    /// Field metadata (for CreateAttribute operations)
    pub field_metadata: Option<Value>,
}

/// Type of sync operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationType {
    /// Delete a record from target (only for junction entities)
    DeleteRecord,
    /// Deactivate a record in target (PATCH statecode: 1)
    DeactivateRecord,
    /// Create a record in target (with preserved GUID)
    CreateRecord,
    /// Update a record in target (PATCH with origin data)
    UpdateRecord,
    /// Create a new attribute on target entity
    CreateAttribute,
}

impl OperationType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::DeleteRecord => "Delete",
            Self::DeactivateRecord => "Deactivate",
            Self::CreateRecord => "Create",
            Self::UpdateRecord => "Update",
            Self::CreateAttribute => "Add Field",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::DeleteRecord => "×",
            Self::DeactivateRecord => "○",
            Self::CreateRecord => "+",
            Self::UpdateRecord => "~",
            Self::CreateAttribute => "⊕",
        }
    }
}

/// Batch of operations for a single entity
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityOperationBatch {
    /// Entity logical name
    pub entity_name: String,
    /// Entity display name
    pub display_name: Option<String>,
    /// Delete operations - only for junction entities (executed first)
    pub deletes: Vec<SyncOperation>,
    /// Deactivate operations - for regular entities target-only records (executed first)
    pub deactivates: Vec<SyncOperation>,
    /// Schema operations (executed after deletes/deactivates)
    pub schema_ops: Vec<SyncOperation>,
    /// Update operations - for records existing in both origin and target
    pub updates: Vec<SyncOperation>,
    /// Insert/Create operations - for origin-only records (executed last)
    pub inserts: Vec<SyncOperation>,
}

impl EntityOperationBatch {
    pub fn total_ops(&self) -> usize {
        self.deletes.len() + self.deactivates.len() + self.schema_ops.len() + self.updates.len() + self.inserts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.deletes.is_empty() && self.deactivates.is_empty() && self.schema_ops.is_empty() && self.updates.is_empty() && self.inserts.is_empty()
    }
}

/// Complete operation list for sync execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OperationPlan {
    /// Origin environment
    pub origin_env: String,
    /// Target environment
    pub target_env: String,
    /// Entity batches in execution order
    pub entity_batches: Vec<EntityOperationBatch>,
    /// Total delete operations (junction entities only)
    pub total_deletes: usize,
    /// Total deactivate operations (regular entities, target-only records)
    pub total_deactivates: usize,
    /// Total schema operations
    pub total_schema_ops: usize,
    /// Total update operations (records in both origin and target)
    pub total_updates: usize,
    /// Total insert/create operations (origin-only records)
    pub total_inserts: usize,
}

impl OperationPlan {
    /// Get all operations in execution order
    pub fn all_operations(&self) -> Vec<&SyncOperation> {
        let mut ops = Vec::new();

        // Phase 1: All deletes (junction entities, in dependency order - dependents first)
        for batch in &self.entity_batches {
            ops.extend(batch.deletes.iter());
        }

        // Phase 2: All deactivates (regular entities, target-only records)
        for batch in &self.entity_batches {
            ops.extend(batch.deactivates.iter());
        }

        // Phase 3: All schema changes
        for batch in &self.entity_batches {
            ops.extend(batch.schema_ops.iter());
        }

        // Phase 4: All updates (records in both origin and target)
        for batch in &self.entity_batches {
            ops.extend(batch.updates.iter());
        }

        // Phase 5: All inserts (origin-only records, in dependency order - dependencies first)
        for batch in &self.entity_batches {
            ops.extend(batch.inserts.iter());
        }

        ops
    }

    pub fn total_operations(&self) -> usize {
        self.total_deletes + self.total_deactivates + self.total_schema_ops + self.total_updates + self.total_inserts
    }
}

/// Build an operation plan from a sync plan
pub fn build_operation_plan(sync_plan: &SyncPlan) -> OperationPlan {
    let mut plan = OperationPlan {
        origin_env: sync_plan.origin_env.clone(),
        target_env: sync_plan.target_env.clone(),
        ..Default::default()
    };

    // Build entity batches in insert order (dependencies first)
    // The sync_plan.entity_plans are already ordered by insert_priority
    let insert_ordered: Vec<&EntitySyncPlan> = sync_plan.insert_order();

    for entity_plan in insert_ordered {
        let batch = build_entity_batch(entity_plan);

        plan.total_deletes += batch.deletes.len();
        plan.total_schema_ops += batch.schema_ops.len();
        plan.total_inserts += batch.inserts.len();

        if !batch.is_empty() {
            plan.entity_batches.push(batch);
        }
    }

    plan
}

/// Build operations for a single entity
fn build_entity_batch(entity_plan: &EntitySyncPlan) -> EntityOperationBatch {
    let entity_name = &entity_plan.entity_info.logical_name;
    let mut batch = EntityOperationBatch {
        entity_name: entity_name.clone(),
        display_name: entity_plan.entity_info.display_name.clone(),
        ..Default::default()
    };

    let mut op_counter = 0u32;

    // Delete operations - all target records will be deleted
    // In a real implementation, we'd fetch record IDs from target
    // For now, we represent this as a single bulk delete marker
    if entity_plan.data_preview.target_count > 0 {
        batch.deletes.push(SyncOperation {
            id: format!("{}-delete-bulk", entity_name),
            entity_name: entity_name.clone(),
            operation_type: OperationType::DeleteRecord,
            priority: entity_plan.entity_info.delete_priority,
            record_id: None, // Bulk delete - no specific ID
            record_data: None,
            field_metadata: None,
        });
        op_counter += 1;
    }

    // Schema operations - add new fields
    for field in &entity_plan.schema_diff.fields_to_add {
        if field.is_system_field {
            continue;
        }

        batch.schema_ops.push(build_create_attribute_op(
            entity_name,
            field,
            op_counter,
        ));
        op_counter += 1;
    }

    // Insert operations - all origin records will be inserted
    // In a real implementation, we'd have the actual record data
    // For now, we represent this as a single bulk insert marker
    if entity_plan.data_preview.origin_count > 0 {
        batch.inserts.push(SyncOperation {
            id: format!("{}-insert-bulk", entity_name),
            entity_name: entity_name.clone(),
            operation_type: OperationType::CreateRecord,
            priority: entity_plan.entity_info.insert_priority,
            record_id: None, // Will be set from origin records
            record_data: None,
            field_metadata: None,
        });
    }

    batch
}

/// Build a CreateAttribute operation for a field
fn build_create_attribute_op(
    entity_name: &str,
    field: &FieldDiffEntry,
    _counter: u32,
) -> SyncOperation {
    SyncOperation {
        id: format!("{}-attr-{}", entity_name, field.logical_name),
        entity_name: entity_name.to_string(),
        operation_type: OperationType::CreateAttribute,
        priority: 0, // Schema ops happen between deletes and inserts
        record_id: None,
        record_data: None,
        field_metadata: field.origin_metadata.clone(),
    }
}

/// Summary of planned operations
#[derive(Debug, Clone, Default)]
pub struct OperationSummary {
    /// Entities that will have records deleted (junction entities only)
    pub entities_with_deletes: Vec<(String, usize)>,
    /// Entities that will have records deactivated (regular entities, target-only)
    pub entities_with_deactivates: Vec<(String, usize)>,
    /// Entities that will have schema changes
    pub entities_with_schema_changes: Vec<(String, usize)>,
    /// Entities that will have records updated (both origin and target)
    pub entities_with_updates: Vec<(String, usize)>,
    /// Entities that will have records created (origin-only)
    pub entities_with_creates: Vec<(String, usize)>,
    /// Fields that need manual review (type mismatches)
    pub fields_needing_review: Vec<(String, String, String)>, // (entity, field, reason)
    /// External lookups that will be nulled
    pub lookups_to_null: Vec<(String, String, String, usize)>, // (entity, field, target, count)
}

/// Build a human-readable summary from an operation plan
pub fn build_operation_summary(sync_plan: &SyncPlan) -> OperationSummary {
    let mut summary = OperationSummary::default();

    for entity_plan in &sync_plan.entity_plans {
        let entity_name = &entity_plan.entity_info.logical_name;
        let is_junction = entity_plan.entity_info.nn_relationship.is_some();
        let pk_field = format!("{}id", entity_name);

        // Build sets for GUID comparison
        let origin_guids: HashSet<String> = entity_plan
            .data_preview
            .origin_records
            .iter()
            .filter_map(|r| r.get(&pk_field).and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();

        let target_guids: HashSet<String> = entity_plan
            .data_preview
            .target_records
            .iter()
            .map(|r| r.id.clone())
            .collect();

        // Count by operation type
        let creates = origin_guids.iter().filter(|g| !target_guids.contains(*g)).count();
        let updates = origin_guids.iter().filter(|g| target_guids.contains(*g)).count();
        let target_only = target_guids.iter().filter(|g| !origin_guids.contains(*g)).count();

        if is_junction {
            // Junction entities: target-only records are deleted
            if target_only > 0 {
                summary.entities_with_deletes.push((entity_name.clone(), target_only));
            }
        } else {
            // Regular entities: target-only records are deactivated
            if target_only > 0 {
                summary.entities_with_deactivates.push((entity_name.clone(), target_only));
            }
        }

        // Updates (records in both)
        if updates > 0 {
            summary.entities_with_updates.push((entity_name.clone(), updates));
        }

        // Creates (origin-only records)
        if creates > 0 {
            summary.entities_with_creates.push((entity_name.clone(), creates));
        }

        // Schema changes
        let schema_changes = entity_plan.schema_diff.fields_to_add
            .iter()
            .filter(|f| !f.is_system_field)
            .count();
        if schema_changes > 0 {
            summary.entities_with_schema_changes.push((
                entity_name.clone(),
                schema_changes,
            ));
        }

        // Fields needing review (type mismatches + target-only)
        for field in &entity_plan.schema_diff.fields_type_mismatch {
            let reason = if let super::super::types::FieldSyncStatus::TypeMismatch {
                origin_type,
                target_type,
            } = &field.status
            {
                format!("Type mismatch: {} vs {}", origin_type, target_type)
            } else {
                "Type mismatch".to_string()
            };

            summary.fields_needing_review.push((
                entity_name.clone(),
                field.logical_name.clone(),
                reason,
            ));
        }

        for field in &entity_plan.schema_diff.fields_target_only {
            if !field.is_system_field {
                summary.fields_needing_review.push((
                    entity_name.clone(),
                    field.logical_name.clone(),
                    "Only in target (consider deletion)".to_string(),
                ));
            }
        }

        // Nulled lookups
        for nulled in &entity_plan.nulled_lookups {
            summary.lookups_to_null.push((
                entity_name.clone(),
                nulled.field_name.clone(),
                nulled.target_entity.clone(),
                nulled.affected_count,
            ));
        }
    }

    summary
}

// =============================================================================
// Queue Operation Builders
// =============================================================================
// These functions convert SyncPlan data into the actual Operation types
// used by the queue system for execution.
//
// Sync Strategy:
// - Compare origin and target records by primary key (GUID)
// - Origin-only records → Create (POST with GUID in body)
// - Both exist → Update (PATCH with origin data, reactivates if inactive)
// - Target-only records → Deactivate (PATCH statecode: 1) for regular entities
// - Junction entities use DisassociateRef (DELETE on $ref, not DELETE on entity)

/// Build delete operations for junction entities only.
/// Uses DisassociateRef instead of Delete (Dynamics 365 intersect entities don't support DELETE).
/// Regular entities use deactivation instead of deletion.
/// Returns operations in delete order (dependents before dependencies).
pub fn build_delete_operations(plan: &SyncPlan) -> Vec<Operation> {
    let mut operations = Vec::new();

    // Get entities in delete order (higher delete_priority = delete first)
    for entity_plan in plan.delete_order() {
        // Only process junction/intersect entities (they don't have statecode)
        let Some(ref nn_info) = entity_plan.entity_info.nn_relationship else {
            continue;
        };

        // Use raw junction target records to get FK values
        // (target_records only has id + name, but we need the FK GUIDs)
        for record in &entity_plan.data_preview.junction_target_raw {
            // Extract FK values using the relationship metadata
            let parent_id = record.get(&nn_info.parent_fk_field).and_then(|v| v.as_str());
            let target_id = record.get(&nn_info.target_fk_field).and_then(|v| v.as_str());

            match (parent_id, target_id) {
                (Some(parent_id), Some(target_id)) => {
                    // Use DisassociateRef: DELETE /parent_entity(parent_id)/navigation_property(target_id)/$ref
                    operations.push(Operation::DisassociateRef {
                        entity: nn_info.parent_entity_set.clone(),
                        entity_ref: parent_id.to_string(),
                        navigation_property: nn_info.navigation_property.clone(),
                        target_id: target_id.to_string(),
                    });
                }
                _ => {
                    log::warn!(
                        "Junction record missing FK fields (parent={}, target={}): {:?}",
                        nn_info.parent_fk_field, nn_info.target_fk_field, record
                    );
                }
            }
        }
    }

    operations
}

/// Build deactivate operations for target-only records in regular entities.
/// These are records that exist in target but not in origin - they get deactivated.
/// Returns operations in delete order (dependents before dependencies).
pub fn build_deactivate_operations(plan: &SyncPlan) -> Vec<Operation> {
    let mut operations = Vec::new();

    // Get entities in delete order (higher delete_priority = process first)
    for entity_plan in plan.delete_order() {
        // Skip junction entities - they use delete, not deactivate
        if entity_plan.entity_info.nn_relationship.is_some() {
            continue;
        }

        let entity_set = &entity_plan.entity_info.entity_set_name;
        let pk_field = format!("{}id", entity_plan.entity_info.logical_name);

        // Build set of origin GUIDs
        let origin_guids: HashSet<String> = entity_plan
            .data_preview
            .origin_records
            .iter()
            .filter_map(|r| r.get(&pk_field).and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();

        // Deactivate target records not in origin
        for target_record in &entity_plan.data_preview.target_records {
            if !origin_guids.contains(&target_record.id) {
                operations.push(Operation::Update {
                    entity: entity_set.clone(),
                    id: target_record.id.clone(),
                    data: serde_json::json!({"statecode": 1}),
                });
            }
        }
    }

    operations
}

/// Build schema operations for adding new fields to target.
/// Returns CreateAttribute operations followed by PublishAllXml.
/// Order doesn't matter for schema operations (no dependencies between fields).
pub fn build_schema_operations(plan: &SyncPlan, solution_name: Option<&str>) -> Vec<Operation> {
    let mut operations = Vec::new();

    for entity_plan in &plan.entity_plans {
        let entity_name = &entity_plan.entity_info.logical_name;

        for field in &entity_plan.schema_diff.fields_to_add {
            // Skip system fields
            if field.is_system_field {
                continue;
            }

            // Skip fields without raw metadata (can't create without it)
            let Some(ref attr_data) = field.origin_metadata else {
                log::warn!(
                    "Skipping field {}.{} - no raw attribute metadata available",
                    entity_name, field.logical_name
                );
                continue;
            };

            operations.push(Operation::CreateAttribute {
                entity: entity_name.clone(),
                attribute_data: attr_data.clone(),
                solution_name: solution_name.map(|s| s.to_string()),
            });
        }
    }

    // Add PublishAllXml at the end if any schema changes were made
    if !operations.is_empty() {
        operations.push(Operation::PublishAllXml);
    }

    operations
}

/// Build insert operations for origin-only records (records not in target).
/// Returns operations in insert order (dependencies before dependents).
/// Skips junction entities (handled by build_junction_operations).
pub fn build_insert_operations(plan: &SyncPlan) -> Vec<Operation> {
    let mut operations = Vec::new();

    // Build entity_set lookup map from all entity plans
    let entity_set_map: HashMap<String, String> = plan
        .entity_plans
        .iter()
        .map(|p| {
            (
                p.entity_info.logical_name.clone(),
                p.entity_info.entity_set_name.clone(),
            )
        })
        .collect();

    // Get entities in insert order (lower insert_priority = insert first)
    for entity_plan in plan.insert_order() {
        // Skip junction entities - they use AssociateRef, not Create
        if entity_plan.entity_info.nn_relationship.is_some() {
            continue;
        }

        let pk_field = format!("{}id", entity_plan.entity_info.logical_name);

        // Build set of target GUIDs
        let target_guids: HashSet<String> = entity_plan
            .data_preview
            .target_records
            .iter()
            .map(|r| r.id.clone())
            .collect();

        // Build internal lookups map for this entity
        // Maps field_name -> (schema_name, entity_set_name)
        let internal_lookups: HashMap<String, (String, String)> = entity_plan
            .entity_info
            .lookups
            .iter()
            .filter(|l| l.is_internal)
            .filter_map(|l| {
                entity_set_map
                    .get(&l.target_entity)
                    .map(|entity_set| (l.field_name.clone(), (l.schema_name.clone(), entity_set.clone())))
            })
            .collect();

        // Build set of fields that exist in target schema
        let target_fields: HashSet<String> = entity_plan
            .schema_diff
            .fields_in_both
            .iter()
            .map(|f| f.logical_name.clone())
            .collect();

        let ctx = InsertCleaningContext {
            internal_lookups,
            nulled_lookups: &entity_plan.nulled_lookups,
            target_fields,
        };

        let entity_set = &entity_plan.entity_info.entity_set_name;

        // Only create records that don't exist in target
        for record in &entity_plan.data_preview.origin_records {
            let Some(guid) = record.get(&pk_field).and_then(|v| v.as_str()) else {
                log::warn!("Origin record missing primary key field '{}': {:?}", pk_field, record);
                continue;
            };

            // Skip if already exists in target (will be updated instead)
            if target_guids.contains(guid) {
                continue;
            }

            let cleaned = clean_record_for_insert(record, &ctx);
            operations.push(Operation::Create {
                entity: entity_set.clone(),
                data: cleaned,
            });
        }
    }

    operations
}

/// Build update operations for records that exist in both origin and target.
/// Uses origin data to update target records (also reactivates inactive records).
/// Returns operations in insert order (dependencies before dependents).
/// Skips junction entities (handled by build_junction_operations).
pub fn build_update_operations(plan: &SyncPlan) -> Vec<Operation> {
    let mut operations = Vec::new();

    // Build entity_set lookup map from all entity plans
    let entity_set_map: HashMap<String, String> = plan
        .entity_plans
        .iter()
        .map(|p| {
            (
                p.entity_info.logical_name.clone(),
                p.entity_info.entity_set_name.clone(),
            )
        })
        .collect();

    // Get entities in insert order (lower insert_priority = process first)
    for entity_plan in plan.insert_order() {
        // Skip junction entities - they use AssociateRef, not Update
        if entity_plan.entity_info.nn_relationship.is_some() {
            continue;
        }

        let pk_field = format!("{}id", entity_plan.entity_info.logical_name);

        // Build set of target GUIDs
        let target_guids: HashSet<String> = entity_plan
            .data_preview
            .target_records
            .iter()
            .map(|r| r.id.clone())
            .collect();

        // Build internal lookups map for this entity
        // Maps field_name -> (schema_name, entity_set_name)
        let internal_lookups: HashMap<String, (String, String)> = entity_plan
            .entity_info
            .lookups
            .iter()
            .filter(|l| l.is_internal)
            .filter_map(|l| {
                entity_set_map
                    .get(&l.target_entity)
                    .map(|entity_set| (l.field_name.clone(), (l.schema_name.clone(), entity_set.clone())))
            })
            .collect();

        // Build set of fields that exist in target schema
        let target_fields: HashSet<String> = entity_plan
            .schema_diff
            .fields_in_both
            .iter()
            .map(|f| f.logical_name.clone())
            .collect();

        let ctx = InsertCleaningContext {
            internal_lookups,
            nulled_lookups: &entity_plan.nulled_lookups,
            target_fields,
        };

        let entity_set = &entity_plan.entity_info.entity_set_name;

        // Only update records that exist in both origin and target
        for record in &entity_plan.data_preview.origin_records {
            let Some(guid) = record.get(&pk_field).and_then(|v| v.as_str()) else {
                continue;
            };

            // Skip if doesn't exist in target (will be created instead)
            if !target_guids.contains(guid) {
                continue;
            }

            // Clean the record (same as for insert, includes statecode for reactivation)
            let cleaned = clean_record_for_insert(record, &ctx);
            operations.push(Operation::Update {
                entity: entity_set.clone(),
                id: guid.to_string(),
                data: cleaned,
            });
        }
    }

    operations
}

/// Clean a record for insertion by filtering out API response metadata and converting lookups.
///
/// - Filters out OData annotations (@odata.*, @OData.*, @Microsoft.*)
/// - Filters out navigation property values (_*_value fields)
/// - Removes system fields (createdby, modifiedon, etc.)
/// - Converts internal lookups to @odata.bind format
/// - Nulls external lookups (lookups to entities not in sync set)
pub fn clean_record_for_insert(record: &Value, ctx: &InsertCleaningContext) -> Value {
    let Some(obj) = record.as_object() else {
        return record.clone();
    };

    let mut cleaned = serde_json::Map::new();

    for (key, value) in obj {
        // Skip null values - no need to send them
        if value.is_null() {
            continue;
        }

        // Skip OData annotations (contains @odata, @OData, or @Microsoft)
        if key.contains("@odata") || key.contains("@OData") || key.contains("@Microsoft") {
            continue;
        }

        // Skip navigation property values (_*_value fields)
        if key.starts_with('_') && key.ends_with("_value") {
            continue;
        }

        // Skip system fields
        if SYSTEM_FIELDS.contains(&key.as_str()) {
            continue;
        }

        // Skip fields that don't exist in target schema
        if !ctx.target_fields.is_empty() && !ctx.target_fields.contains(key) {
            continue;
        }

        // Keep the field
        cleaned.insert(key.clone(), value.clone());
    }

    // Add internal lookups as @odata.bind
    // Use schema_name for the bind key (OData requires proper casing)
    for (field_name, (schema_name, entity_set_name)) in &ctx.internal_lookups {
        let value_key = format!("_{}_value", field_name);
        if let Some(guid) = obj.get(&value_key).and_then(|v| v.as_str()) {
            if !guid.is_empty() {
                let bind_key = format!("{}@odata.bind", schema_name);
                let bind_value = format!("/{}({})", entity_set_name, guid);
                cleaned.insert(bind_key, Value::String(bind_value));
            }
        }
    }

    // Note: External lookups are tracked in ctx.nulled_lookups for reporting,
    // but we don't need to explicitly null them in the payload since:
    // 1. We're doing delete-then-insert (no existing values to clear)
    // 2. Original values come in as _*_value navigation properties which we filter out
    // 3. Omitting the field is equivalent to setting it to null for new records

    Value::Object(cleaned)
}

/// Build junction operations for N:N relationships.
/// Returns AssociateRef operations for entities with nn_relationship metadata.
/// These create the associations between already-inserted records.
pub fn build_junction_operations(plan: &SyncPlan) -> Vec<Operation> {
    let mut operations = Vec::new();

    for entity_plan in plan.insert_order() {
        let Some(ref nn_info) = entity_plan.entity_info.nn_relationship else {
            continue;
        };

        for record in &entity_plan.data_preview.origin_records {
            // Extract parent and target IDs from junction record
            let Some(parent_id) = record.get(&nn_info.parent_fk_field).and_then(|v| v.as_str()) else {
                log::warn!(
                    "Junction record missing parent FK field '{}': {:?}",
                    nn_info.parent_fk_field, record
                );
                continue;
            };

            let Some(target_id) = record.get(&nn_info.target_fk_field).and_then(|v| v.as_str()) else {
                log::warn!(
                    "Junction record missing target FK field '{}': {:?}",
                    nn_info.target_fk_field, record
                );
                continue;
            };

            operations.push(Operation::AssociateRef {
                entity: nn_info.parent_entity_set.clone(),
                entity_ref: parent_id.to_string(),
                navigation_property: nn_info.navigation_property.clone(),
                target_ref: format!("/{}({})", nn_info.target_entity_set, target_id),
            });
        }
    }

    operations
}

/// Default batch size for queue operations (Dynamics 365 batch limit)
pub const DEFAULT_BATCH_SIZE: usize = 50;

/// Chunk operations into batches for queue submission.
/// Preserves operation order - operations within each batch maintain their relative order.
pub fn chunk_operations(ops: Vec<Operation>, chunk_size: usize) -> Vec<Vec<Operation>> {
    if ops.is_empty() {
        return vec![];
    }

    ops.chunks(chunk_size)
        .map(|chunk| chunk.to_vec())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::apps::sync::types::*;

    fn make_test_plan() -> SyncPlan {
        SyncPlan {
            origin_env: "dev".to_string(),
            target_env: "test".to_string(),
            entity_plans: vec![
                EntitySyncPlan {
                    entity_info: SyncEntityInfo {
                        logical_name: "parent".to_string(),
                        display_name: Some("Parent".to_string()),
                        entity_set_name: "parents".to_string(),
                        primary_name_attribute: Some("name".to_string()),
                        category: DependencyCategory::Standalone,
                        lookups: vec![],
                        incoming_references: vec![],
                        dependents: vec!["child".to_string()],
                        insert_priority: 0,
                        delete_priority: 1, // Lower = delete later (parent deleted after child)
                        nn_relationship: None,
                    },
                    schema_diff: EntitySchemaDiff {
                        entity_name: "parent".to_string(),
                        fields_in_both: vec![],
                        fields_to_add: vec![FieldDiffEntry {
                            logical_name: "new_field".to_string(),
                            display_name: Some("New Field".to_string()),
                            field_type: "String".to_string(),
                            status: FieldSyncStatus::OriginOnly,
                            is_system_field: false,
                            origin_metadata: None,
                        }],
                        fields_target_only: vec![],
                        fields_type_mismatch: vec![],
                    },
                    data_preview: EntityDataPreview {
                        entity_name: "parent".to_string(),
                        origin_count: 10,
                        target_count: 2,
                        origin_records: vec![],
                        target_records: vec![
                            TargetRecord { id: "parent-1".to_string(), name: Some("Parent 1".to_string()), junction_parent_id: None, junction_target_id: None },
                            TargetRecord { id: "parent-2".to_string(), name: Some("Parent 2".to_string()), junction_parent_id: None, junction_target_id: None },
                        ],
                        junction_target_raw: vec![],
                    },
                    nulled_lookups: vec![],
                },
                EntitySyncPlan {
                    entity_info: SyncEntityInfo {
                        logical_name: "child".to_string(),
                        display_name: Some("Child".to_string()),
                        entity_set_name: "children".to_string(),
                        primary_name_attribute: Some("name".to_string()),
                        category: DependencyCategory::Dependent,
                        lookups: vec![LookupInfo {
                            field_name: "parentid".to_string(),
                            schema_name: "parentid".to_string(),
                            target_entity: "parent".to_string(),
                            is_internal: true,
                        }],
                        incoming_references: vec![],
                        dependents: vec![],
                        insert_priority: 1,
                        delete_priority: 2, // Higher = delete first (child deleted before parent)
                        nn_relationship: None,
                    },
                    schema_diff: EntitySchemaDiff::default(),
                    data_preview: EntityDataPreview {
                        entity_name: "child".to_string(),
                        origin_count: 20,
                        target_count: 3,
                        origin_records: vec![],
                        target_records: vec![
                            TargetRecord { id: "child-1".to_string(), name: Some("Child 1".to_string()), junction_parent_id: None, junction_target_id: None },
                            TargetRecord { id: "child-2".to_string(), name: Some("Child 2".to_string()), junction_parent_id: None, junction_target_id: None },
                            TargetRecord { id: "child-3".to_string(), name: Some("Child 3".to_string()), junction_parent_id: None, junction_target_id: None },
                        ],
                        junction_target_raw: vec![],
                    },
                    nulled_lookups: vec![],
                },
            ],
            detected_junctions: vec![],
            has_schema_changes: true,
            total_delete_count: 5,
            total_insert_count: 30,
        }
    }

    #[test]
    fn test_build_operation_plan() {
        let sync_plan = make_test_plan();
        let op_plan = build_operation_plan(&sync_plan);

        assert_eq!(op_plan.origin_env, "dev");
        assert_eq!(op_plan.target_env, "test");
        assert_eq!(op_plan.entity_batches.len(), 2);
    }

    #[test]
    fn test_operation_counts() {
        let sync_plan = make_test_plan();
        let op_plan = build_operation_plan(&sync_plan);

        // Both entities have deletes
        assert_eq!(op_plan.total_deletes, 2);
        // Only parent has schema changes
        assert_eq!(op_plan.total_schema_ops, 1);
        // Both entities have inserts
        assert_eq!(op_plan.total_inserts, 2);
    }

    #[test]
    fn test_entity_batch_order() {
        let sync_plan = make_test_plan();
        let op_plan = build_operation_plan(&sync_plan);

        // Parent should come before child (insert order)
        let parent_idx = op_plan.entity_batches.iter()
            .position(|b| b.entity_name == "parent")
            .unwrap();
        let child_idx = op_plan.entity_batches.iter()
            .position(|b| b.entity_name == "child")
            .unwrap();

        assert!(parent_idx < child_idx);
    }

    #[test]
    fn test_operation_summary() {
        let sync_plan = make_test_plan();
        let summary = build_operation_summary(&sync_plan);

        // make_test_plan has target_records but empty origin_records
        // So all targets become deactivates (not deletes - not junctions)
        assert_eq!(summary.entities_with_deletes.len(), 0); // No junction entities
        assert_eq!(summary.entities_with_deactivates.len(), 2); // parent + child (target-only)
        assert_eq!(summary.entities_with_updates.len(), 0); // No overlap (empty origin_records)
        assert_eq!(summary.entities_with_creates.len(), 0); // No origin_records
        assert_eq!(summary.entities_with_schema_changes.len(), 1); // parent has new_field
    }

    #[test]
    fn test_system_fields_excluded() {
        let mut sync_plan = make_test_plan();
        sync_plan.entity_plans[0].schema_diff.fields_to_add.push(FieldDiffEntry {
            logical_name: "createdon".to_string(),
            display_name: Some("Created On".to_string()),
            field_type: "DateTime".to_string(),
            status: FieldSyncStatus::OriginOnly,
            is_system_field: true,
            origin_metadata: None,
        });

        let op_plan = build_operation_plan(&sync_plan);

        // System field should not generate an operation
        assert_eq!(op_plan.total_schema_ops, 1);
    }

    #[test]
    fn test_build_delete_operations_only_junctions() {
        // Regular entities (non-junction) should NOT generate delete operations
        let sync_plan = make_test_plan();
        let delete_ops = build_delete_operations(&sync_plan);

        // make_test_plan has no junction entities, so no deletes
        assert_eq!(delete_ops.len(), 0);
    }

    #[test]
    fn test_build_deactivate_operations_target_only() {
        let sync_plan = make_test_plan();
        let deactivate_ops = build_deactivate_operations(&sync_plan);

        // All 5 target records should be deactivated (2 parent + 3 child)
        // because make_test_plan has empty origin_records
        assert_eq!(deactivate_ops.len(), 5);

        // All should be Update operations with statecode: 1
        for op in &deactivate_ops {
            match op {
                Operation::Update { data, .. } => {
                    assert_eq!(data["statecode"], 1);
                }
                _ => panic!("Expected Update operation for deactivation"),
            }
        }
    }

    #[test]
    fn test_build_deactivate_operations_priority_order() {
        let sync_plan = make_test_plan();
        let deactivate_ops = build_deactivate_operations(&sync_plan);

        // Child has higher delete_priority (2) than parent (1)
        // So child records should come FIRST in the deactivate order
        let entity_order: Vec<&str> = deactivate_ops
            .iter()
            .map(|op| match op {
                Operation::Update { entity, .. } => entity.as_str(),
                _ => panic!("Expected Update operation"),
            })
            .collect();

        // First 3 should be children (deactivated first)
        assert_eq!(entity_order[0], "children");
        assert_eq!(entity_order[1], "children");
        assert_eq!(entity_order[2], "children");

        // Last 2 should be parents (deactivated after children)
        assert_eq!(entity_order[3], "parents");
        assert_eq!(entity_order[4], "parents");
    }

    #[test]
    fn test_build_schema_operations() {
        let mut sync_plan = make_test_plan();

        // Add a field with origin_metadata
        sync_plan.entity_plans[0].schema_diff.fields_to_add.push(FieldDiffEntry {
            logical_name: "new_custom_field".to_string(),
            display_name: Some("New Custom Field".to_string()),
            field_type: "String".to_string(),
            status: FieldSyncStatus::OriginOnly,
            is_system_field: false,
            origin_metadata: Some(serde_json::json!({
                "@odata.type": "Microsoft.Dynamics.CRM.StringAttributeMetadata",
                "LogicalName": "new_custom_field",
                "SchemaName": "new_CustomField"
            })),
        });

        // Add a system field (should be excluded)
        sync_plan.entity_plans[0].schema_diff.fields_to_add.push(FieldDiffEntry {
            logical_name: "createdby".to_string(),
            display_name: Some("Created By".to_string()),
            field_type: "Lookup".to_string(),
            status: FieldSyncStatus::OriginOnly,
            is_system_field: true,
            origin_metadata: Some(serde_json::json!({})),
        });

        // Add a field without origin_metadata (should be excluded)
        sync_plan.entity_plans[0].schema_diff.fields_to_add.push(FieldDiffEntry {
            logical_name: "missing_metadata".to_string(),
            display_name: Some("Missing Metadata".to_string()),
            field_type: "String".to_string(),
            status: FieldSyncStatus::OriginOnly,
            is_system_field: false,
            origin_metadata: None,
        });

        let schema_ops = build_schema_operations(&sync_plan, None);

        // Should have 2 operations: 1 CreateAttribute + 1 PublishAllXml
        // (parent already has new_field in make_test_plan, plus our new_custom_field)
        // Note: new_field has origin_metadata: None in make_test_plan, so only new_custom_field counts
        assert_eq!(schema_ops.len(), 2);

        // First should be CreateAttribute
        match &schema_ops[0] {
            Operation::CreateAttribute { entity, attribute_data, solution_name } => {
                // Should use logical_name, not entity_set_name
                assert_eq!(entity, "parent");
                assert!(attribute_data["LogicalName"].as_str() == Some("new_custom_field"));
                assert!(solution_name.is_none());
            }
            _ => panic!("Expected CreateAttribute operation"),
        }

        // Last should be PublishAllXml
        assert!(matches!(schema_ops.last(), Some(Operation::PublishAllXml)));
    }

    #[test]
    fn test_build_schema_operations_empty_when_no_changes() {
        let mut sync_plan = make_test_plan();

        // Clear all fields_to_add
        for entity_plan in &mut sync_plan.entity_plans {
            entity_plan.schema_diff.fields_to_add.clear();
        }

        let schema_ops = build_schema_operations(&sync_plan, None);

        // Should be empty (no PublishAllXml when no changes)
        assert!(schema_ops.is_empty());
    }

    fn make_test_plan_with_records() -> SyncPlan {
        SyncPlan {
            origin_env: "dev".to_string(),
            target_env: "test".to_string(),
            entity_plans: vec![
                EntitySyncPlan {
                    entity_info: SyncEntityInfo {
                        logical_name: "parent".to_string(),
                        display_name: Some("Parent".to_string()),
                        entity_set_name: "parents".to_string(),
                        primary_name_attribute: Some("name".to_string()),
                        category: DependencyCategory::Standalone,
                        lookups: vec![],
                        incoming_references: vec![],
                        dependents: vec!["child".to_string()],
                        insert_priority: 0, // Insert first (no dependencies)
                        delete_priority: 1,
                        nn_relationship: None,
                    },
                    schema_diff: EntitySchemaDiff::default(),
                    data_preview: EntityDataPreview {
                        entity_name: "parent".to_string(),
                        origin_count: 2,
                        target_count: 0,
                        origin_records: vec![
                            serde_json::json!({"parentid": "p1", "name": "Parent 1"}),
                            serde_json::json!({"parentid": "p2", "name": "Parent 2"}),
                        ],
                        target_records: vec![],
                        junction_target_raw: vec![],
                    },
                    nulled_lookups: vec![],
                },
                EntitySyncPlan {
                    entity_info: SyncEntityInfo {
                        logical_name: "child".to_string(),
                        display_name: Some("Child".to_string()),
                        entity_set_name: "children".to_string(),
                        primary_name_attribute: Some("name".to_string()),
                        category: DependencyCategory::Dependent,
                        lookups: vec![LookupInfo {
                            field_name: "parentid".to_string(),
                            schema_name: "parentid".to_string(),
                            target_entity: "parent".to_string(),
                            is_internal: true,
                        }],
                        incoming_references: vec![],
                        dependents: vec![],
                        insert_priority: 1, // Insert after parent
                        delete_priority: 2,
                        nn_relationship: None,
                    },
                    schema_diff: EntitySchemaDiff::default(),
                    data_preview: EntityDataPreview {
                        entity_name: "child".to_string(),
                        origin_count: 3,
                        target_count: 0,
                        origin_records: vec![
                            serde_json::json!({"childid": "c1", "name": "Child 1", "parentid": "p1"}),
                            serde_json::json!({"childid": "c2", "name": "Child 2", "parentid": "p1"}),
                            serde_json::json!({"childid": "c3", "name": "Child 3", "parentid": "p2"}),
                        ],
                        target_records: vec![],
                        junction_target_raw: vec![],
                    },
                    nulled_lookups: vec![],
                },
            ],
            detected_junctions: vec![],
            has_schema_changes: false,
            total_delete_count: 0,
            total_insert_count: 5,
        }
    }

    #[test]
    fn test_build_insert_operations_order() {
        let sync_plan = make_test_plan_with_records();
        let insert_ops = build_insert_operations(&sync_plan);

        // Should have 5 operations (2 parent + 3 child)
        assert_eq!(insert_ops.len(), 5);

        // Parent has lower insert_priority (0) than child (1)
        // So parent records should come FIRST
        let entity_order: Vec<&str> = insert_ops
            .iter()
            .map(|op| match op {
                Operation::Create { entity, .. } => entity.as_str(),
                _ => panic!("Expected Create operation"),
            })
            .collect();

        // First 2 should be parents
        assert_eq!(entity_order[0], "parents");
        assert_eq!(entity_order[1], "parents");

        // Last 3 should be children
        assert_eq!(entity_order[2], "children");
        assert_eq!(entity_order[3], "children");
        assert_eq!(entity_order[4], "children");
    }

    #[test]
    fn test_build_insert_operations_skips_junction() {
        let mut sync_plan = make_test_plan_with_records();

        // Add a junction entity
        sync_plan.entity_plans.push(EntitySyncPlan {
            entity_info: SyncEntityInfo {
                logical_name: "parent_child_junction".to_string(),
                display_name: Some("Junction".to_string()),
                entity_set_name: "parent_child_junctions".to_string(),
                primary_name_attribute: None,
                category: DependencyCategory::Junction,
                lookups: vec![],
                incoming_references: vec![],
                dependents: vec![],
                insert_priority: 2, // Insert last
                delete_priority: 3,
                nn_relationship: Some(NNRelationshipInfo {
                    navigation_property: "children".to_string(),
                    parent_entity: "parent".to_string(),
                    parent_entity_set: "parents".to_string(),
                    parent_fk_field: "parentid".to_string(),
                    target_entity: "child".to_string(),
                    target_entity_set: "children".to_string(),
                    target_fk_field: "childid".to_string(),
                }),
            },
            schema_diff: EntitySchemaDiff::default(),
            data_preview: EntityDataPreview {
                entity_name: "parent_child_junction".to_string(),
                origin_count: 2,
                target_count: 0,
                origin_records: vec![
                    serde_json::json!({"parentid": "p1", "childid": "c1"}),
                    serde_json::json!({"parentid": "p2", "childid": "c2"}),
                ],
                target_records: vec![],
                junction_target_raw: vec![],
            },
            nulled_lookups: vec![],
        });

        let insert_ops = build_insert_operations(&sync_plan);

        // Should still have only 5 operations (junction skipped)
        assert_eq!(insert_ops.len(), 5);

        // No operations for junction entity
        for op in &insert_ops {
            match op {
                Operation::Create { entity, .. } => {
                    assert_ne!(entity, "parent_child_junctions");
                }
                _ => panic!("Expected Create operation"),
            }
        }
    }

    #[test]
    fn test_build_insert_operations_empty_when_no_records() {
        let mut sync_plan = make_test_plan_with_records();

        // Clear all origin_records
        for entity_plan in &mut sync_plan.entity_plans {
            entity_plan.data_preview.origin_records.clear();
        }

        let insert_ops = build_insert_operations(&sync_plan);

        assert!(insert_ops.is_empty());
    }

    fn make_test_plan_with_junction() -> SyncPlan {
        SyncPlan {
            origin_env: "dev".to_string(),
            target_env: "test".to_string(),
            entity_plans: vec![
                EntitySyncPlan {
                    entity_info: SyncEntityInfo {
                        logical_name: "account".to_string(),
                        display_name: Some("Account".to_string()),
                        entity_set_name: "accounts".to_string(),
                        primary_name_attribute: Some("name".to_string()),
                        category: DependencyCategory::Standalone,
                        lookups: vec![],
                        incoming_references: vec![],
                        dependents: vec![],
                        insert_priority: 0,
                        delete_priority: 2,
                        nn_relationship: None,
                    },
                    schema_diff: EntitySchemaDiff::default(),
                    data_preview: EntityDataPreview {
                        entity_name: "account".to_string(),
                        origin_count: 2,
                        target_count: 0,
                        origin_records: vec![
                            serde_json::json!({"accountid": "acc-1", "name": "Account 1"}),
                            serde_json::json!({"accountid": "acc-2", "name": "Account 2"}),
                        ],
                        target_records: vec![],
                        junction_target_raw: vec![],
                    },
                    nulled_lookups: vec![],
                },
                EntitySyncPlan {
                    entity_info: SyncEntityInfo {
                        logical_name: "contact".to_string(),
                        display_name: Some("Contact".to_string()),
                        entity_set_name: "contacts".to_string(),
                        primary_name_attribute: Some("fullname".to_string()),
                        category: DependencyCategory::Standalone,
                        lookups: vec![],
                        incoming_references: vec![],
                        dependents: vec![],
                        insert_priority: 0,
                        delete_priority: 2,
                        nn_relationship: None,
                    },
                    schema_diff: EntitySchemaDiff::default(),
                    data_preview: EntityDataPreview {
                        entity_name: "contact".to_string(),
                        origin_count: 2,
                        target_count: 0,
                        origin_records: vec![
                            serde_json::json!({"contactid": "con-1", "fullname": "Contact 1"}),
                            serde_json::json!({"contactid": "con-2", "fullname": "Contact 2"}),
                        ],
                        target_records: vec![],
                        junction_target_raw: vec![],
                    },
                    nulled_lookups: vec![],
                },
                EntitySyncPlan {
                    entity_info: SyncEntityInfo {
                        logical_name: "accountcontact".to_string(),
                        display_name: Some("Account Contact".to_string()),
                        entity_set_name: "accountcontacts".to_string(),
                        primary_name_attribute: None,
                        category: DependencyCategory::Junction,
                        lookups: vec![],
                        incoming_references: vec![],
                        dependents: vec![],
                        insert_priority: 1, // After both account and contact
                        delete_priority: 0,
                        nn_relationship: Some(NNRelationshipInfo {
                            navigation_property: "contact_account_association".to_string(),
                            parent_entity: "account".to_string(),
                            parent_entity_set: "accounts".to_string(),
                            parent_fk_field: "accountid".to_string(),
                            target_entity: "contact".to_string(),
                            target_entity_set: "contacts".to_string(),
                            target_fk_field: "contactid".to_string(),
                        }),
                    },
                    schema_diff: EntitySchemaDiff::default(),
                    data_preview: EntityDataPreview {
                        entity_name: "accountcontact".to_string(),
                        origin_count: 3,
                        target_count: 0,
                        origin_records: vec![
                            serde_json::json!({"accountid": "acc-1", "contactid": "con-1"}),
                            serde_json::json!({"accountid": "acc-1", "contactid": "con-2"}),
                            serde_json::json!({"accountid": "acc-2", "contactid": "con-1"}),
                        ],
                        target_records: vec![],
                        junction_target_raw: vec![],
                    },
                    nulled_lookups: vec![],
                },
            ],
            detected_junctions: vec!["accountcontact".to_string()],
            has_schema_changes: false,
            total_delete_count: 0,
            total_insert_count: 7,
        }
    }

    #[test]
    fn test_build_junction_operations() {
        let sync_plan = make_test_plan_with_junction();
        let junction_ops = build_junction_operations(&sync_plan);

        // Should have 3 AssociateRef operations
        assert_eq!(junction_ops.len(), 3);

        // All should be AssociateRef
        for op in &junction_ops {
            match op {
                Operation::AssociateRef { entity, entity_ref, navigation_property, target_ref } => {
                    assert_eq!(entity, "accounts");
                    assert_eq!(navigation_property, "contact_account_association");
                    assert!(entity_ref.starts_with("acc-"));
                    assert!(target_ref.starts_with("/contacts(con-"));
                }
                _ => panic!("Expected AssociateRef operation"),
            }
        }
    }

    #[test]
    fn test_build_junction_operations_correct_refs() {
        let sync_plan = make_test_plan_with_junction();
        let junction_ops = build_junction_operations(&sync_plan);

        // Check first operation specifically
        match &junction_ops[0] {
            Operation::AssociateRef { entity_ref, target_ref, .. } => {
                assert_eq!(entity_ref, "acc-1");
                assert_eq!(target_ref, "/contacts(con-1)");
            }
            _ => panic!("Expected AssociateRef"),
        }

        // Check second operation
        match &junction_ops[1] {
            Operation::AssociateRef { entity_ref, target_ref, .. } => {
                assert_eq!(entity_ref, "acc-1");
                assert_eq!(target_ref, "/contacts(con-2)");
            }
            _ => panic!("Expected AssociateRef"),
        }

        // Check third operation
        match &junction_ops[2] {
            Operation::AssociateRef { entity_ref, target_ref, .. } => {
                assert_eq!(entity_ref, "acc-2");
                assert_eq!(target_ref, "/contacts(con-1)");
            }
            _ => panic!("Expected AssociateRef"),
        }
    }

    #[test]
    fn test_build_junction_operations_skips_non_junction() {
        let sync_plan = make_test_plan_with_junction();
        let junction_ops = build_junction_operations(&sync_plan);

        // Should only have junction operations, not regular entities
        for op in &junction_ops {
            match op {
                Operation::AssociateRef { entity, .. } => {
                    // Entity should be accounts (the parent), not contacts or accountcontacts
                    assert_eq!(entity, "accounts");
                }
                _ => panic!("Expected AssociateRef operation"),
            }
        }
    }

    #[test]
    fn test_build_junction_operations_empty_when_no_junctions() {
        let sync_plan = make_test_plan_with_records(); // No junction entities
        let junction_ops = build_junction_operations(&sync_plan);

        assert!(junction_ops.is_empty());
    }

    #[test]
    fn test_clean_record_removes_system_fields() {
        let record = serde_json::json!({
            "accountid": "abc-123",
            "name": "Test Account",
            "createdby": "user-1",
            "createdon": "2024-01-01",
            "modifiedby": "user-2",
            "modifiedon": "2024-01-02",
            "ownerid": "owner-1",
            "statecode": 0,
            "statuscode": 1
        });

        let ctx = InsertCleaningContext {
            internal_lookups: HashMap::new(),
            nulled_lookups: &[],
            target_fields: HashSet::new(), // Empty = no filtering
        };

        let cleaned = clean_record_for_insert(&record, &ctx);

        // Should keep business fields
        assert_eq!(cleaned["accountid"], "abc-123");
        assert_eq!(cleaned["name"], "Test Account");

        // Should remove system fields
        assert!(cleaned.get("createdby").is_none());
        assert!(cleaned.get("createdon").is_none());
        assert!(cleaned.get("modifiedby").is_none());
        assert!(cleaned.get("modifiedon").is_none());
        assert!(cleaned.get("ownerid").is_none());

        // statecode and statuscode should be KEPT (for activate/deactivate workflow)
        assert_eq!(cleaned["statecode"], 0);
        assert_eq!(cleaned["statuscode"], 1);
    }

    #[test]
    fn test_clean_record_filters_odata_annotations() {
        let record = serde_json::json!({
            "accountid": "abc-123",
            "name": "Test Account",
            "@odata.etag": "W/\"12345\"",
            "createdon@OData.Community.Display.V1.FormattedValue": "1/1/2024",
            "statecode@OData.Community.Display.V1.FormattedValue": "Active",
            "_createdby_value@Microsoft.Dynamics.CRM.lookuplogicalname": "systemuser"
        });

        let ctx = InsertCleaningContext {
            internal_lookups: HashMap::new(),
            nulled_lookups: &[],
            target_fields: HashSet::new(),
        };

        let cleaned = clean_record_for_insert(&record, &ctx);

        // Should keep business fields
        assert_eq!(cleaned["accountid"], "abc-123");
        assert_eq!(cleaned["name"], "Test Account");

        // Should remove OData annotations
        assert!(cleaned.get("@odata.etag").is_none());
        assert!(cleaned.get("createdon@OData.Community.Display.V1.FormattedValue").is_none());
        assert!(cleaned.get("statecode@OData.Community.Display.V1.FormattedValue").is_none());
        assert!(cleaned.get("_createdby_value@Microsoft.Dynamics.CRM.lookuplogicalname").is_none());
    }

    #[test]
    fn test_clean_record_filters_navigation_properties() {
        let record = serde_json::json!({
            "accountid": "abc-123",
            "name": "Test Account",
            "_createdby_value": "user-guid-123",
            "_modifiedby_value": "user-guid-456",
            "_ownerid_value": "owner-guid-789",
            "_parentaccountid_value": "parent-guid-000"
        });

        let ctx = InsertCleaningContext {
            internal_lookups: HashMap::new(),
            nulled_lookups: &[],
            target_fields: HashSet::new(),
        };

        let cleaned = clean_record_for_insert(&record, &ctx);

        // Should keep business fields
        assert_eq!(cleaned["accountid"], "abc-123");
        assert_eq!(cleaned["name"], "Test Account");

        // Should remove navigation property values
        assert!(cleaned.get("_createdby_value").is_none());
        assert!(cleaned.get("_modifiedby_value").is_none());
        assert!(cleaned.get("_ownerid_value").is_none());
        assert!(cleaned.get("_parentaccountid_value").is_none());
    }

    #[test]
    fn test_clean_record_skips_null_values() {
        let record = serde_json::json!({
            "accountid": "abc-123",
            "name": "Test Account",
            "description": null,
            "parentaccountid": null,
            "websiteurl": null,
            "revenue": 1000000
        });

        let ctx = InsertCleaningContext {
            internal_lookups: HashMap::new(),
            nulled_lookups: &[],
            target_fields: HashSet::new(),
        };

        let cleaned = clean_record_for_insert(&record, &ctx);

        // Should keep non-null business fields
        assert_eq!(cleaned["accountid"], "abc-123");
        assert_eq!(cleaned["name"], "Test Account");
        assert_eq!(cleaned["revenue"], 1000000);

        // Should skip null values entirely
        assert!(cleaned.get("description").is_none());
        assert!(cleaned.get("parentaccountid").is_none());
        assert!(cleaned.get("websiteurl").is_none());
    }

    #[test]
    fn test_clean_record_converts_internal_lookups() {
        let record = serde_json::json!({
            "contactid": "con-123",
            "fullname": "John Doe",
            "_parentcustomerid_value": "acc-456"
        });

        let mut internal_lookups = HashMap::new();
        // Map field_name -> (schema_name, entity_set_name)
        internal_lookups.insert("parentcustomerid".to_string(), ("ParentCustomerId".to_string(), "accounts".to_string()));

        let ctx = InsertCleaningContext {
            internal_lookups,
            nulled_lookups: &[],
            target_fields: HashSet::new(),
        };

        let cleaned = clean_record_for_insert(&record, &ctx);

        // Should keep business fields
        assert_eq!(cleaned["contactid"], "con-123");
        assert_eq!(cleaned["fullname"], "John Doe");

        // Should convert internal lookup to @odata.bind format using schema name casing
        assert_eq!(cleaned["ParentCustomerId@odata.bind"], "/accounts(acc-456)");

        // Navigation property should be removed
        assert!(cleaned.get("_parentcustomerid_value").is_none());
    }

    #[test]
    fn test_clean_record_external_lookups_from_nav_properties() {
        // External lookups come in as _*_value navigation properties
        // which get filtered out. We don't need to explicitly null them.
        let record = serde_json::json!({
            "contactid": "con-123",
            "fullname": "John Doe",
            "_parentcustomerid_value": "acc-456",  // External lookup as nav property
            "_owninguser_value": "user-1"          // System lookup as nav property
        });

        let nulled_lookups = vec![
            NulledLookupInfo {
                entity_name: "contact".to_string(),
                field_name: "parentcustomerid".to_string(),
                target_entity: "account".to_string(),
                affected_count: 10,
            },
        ];

        let ctx = InsertCleaningContext {
            internal_lookups: HashMap::new(),
            nulled_lookups: &nulled_lookups,
            target_fields: HashSet::new(),
        };

        let cleaned = clean_record_for_insert(&record, &ctx);

        // Should keep business fields
        assert_eq!(cleaned["contactid"], "con-123");
        assert_eq!(cleaned["fullname"], "John Doe");

        // Navigation properties should be filtered out (not present at all)
        assert!(cleaned.get("_parentcustomerid_value").is_none());
        assert!(cleaned.get("_owninguser_value").is_none());

        // No explicit null added for external lookups
        assert!(cleaned.get("parentcustomerid").is_none());
        assert!(cleaned.get("owninguser").is_none());
    }

    #[test]
    fn test_build_insert_operations_cleans_records() {
        let mut sync_plan = make_test_plan_with_records();

        // Add system fields and external lookup to test data
        sync_plan.entity_plans[0].data_preview.origin_records = vec![
            serde_json::json!({
                "parentid": "p1",
                "name": "Parent 1",
                "createdby": "user-1",
                "modifiedon": "2024-01-01",
                "ownerid": "owner-1"
            }),
        ];

        // Add a nulled lookup
        sync_plan.entity_plans[0].nulled_lookups = vec![
            NulledLookupInfo {
                entity_name: "parent".to_string(),
                field_name: "ownerid".to_string(),
                target_entity: "systemuser".to_string(),
                affected_count: 1,
            },
        ];

        let insert_ops = build_insert_operations(&sync_plan);

        // Find the parent operation
        let parent_op = insert_ops.iter().find(|op| {
            matches!(op, Operation::Create { entity, .. } if entity == "parents")
        });

        assert!(parent_op.is_some());

        if let Operation::Create { data, .. } = parent_op.unwrap() {
            // Business fields should be kept
            assert_eq!(data["parentid"], "p1");
            assert_eq!(data["name"], "Parent 1");

            // System fields should be removed (ownerid is a system field)
            assert!(data.get("createdby").is_none());
            assert!(data.get("modifiedon").is_none());
            assert!(data.get("ownerid").is_none());
        }
    }

    #[test]
    fn test_chunk_operations_preserves_order() {
        // Create operations with identifiable IDs
        let ops: Vec<Operation> = (0..7)
            .map(|i| Operation::Delete {
                entity: "accounts".to_string(),
                id: format!("id-{}", i),
            })
            .collect();

        let chunks = chunk_operations(ops, 3);

        // Should have 3 chunks: [0,1,2], [3,4,5], [6]
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 3);
        assert_eq!(chunks[1].len(), 3);
        assert_eq!(chunks[2].len(), 1);

        // Verify order is preserved
        let mut expected_id = 0;
        for chunk in &chunks {
            for op in chunk {
                match op {
                    Operation::Delete { id, .. } => {
                        assert_eq!(id, &format!("id-{}", expected_id));
                        expected_id += 1;
                    }
                    _ => panic!("Expected Delete operation"),
                }
            }
        }
        assert_eq!(expected_id, 7);
    }

    #[test]
    fn test_chunk_operations_empty() {
        let chunks = chunk_operations(vec![], 50);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_operations_smaller_than_chunk_size() {
        let ops: Vec<Operation> = (0..3)
            .map(|i| Operation::Delete {
                entity: "accounts".to_string(),
                id: format!("id-{}", i),
            })
            .collect();

        let chunks = chunk_operations(ops, 50);

        // Should have 1 chunk with all 3 operations
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 3);
    }

    #[test]
    fn test_chunk_operations_exact_multiple() {
        let ops: Vec<Operation> = (0..6)
            .map(|i| Operation::Delete {
                entity: "accounts".to_string(),
                id: format!("id-{}", i),
            })
            .collect();

        let chunks = chunk_operations(ops, 3);

        // Should have exactly 2 chunks of 3 each
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 3);
        assert_eq!(chunks[1].len(), 3);
    }

    // =========================================================================
    // Tests for Create/Update/Deactivate GUID-based comparison logic
    // =========================================================================

    fn make_test_plan_with_overlap() -> SyncPlan {
        // Creates a plan where:
        // - Origin has records: p1, p2, p3 (p3 is origin-only)
        // - Target has records: p1, p2, p4 (p4 is target-only)
        // Expected: p1, p2 -> Update, p3 -> Create, p4 -> Deactivate
        SyncPlan {
            origin_env: "dev".to_string(),
            target_env: "test".to_string(),
            entity_plans: vec![
                EntitySyncPlan {
                    entity_info: SyncEntityInfo {
                        logical_name: "parent".to_string(),
                        display_name: Some("Parent".to_string()),
                        entity_set_name: "parents".to_string(),
                        primary_name_attribute: Some("name".to_string()),
                        category: DependencyCategory::Standalone,
                        lookups: vec![],
                        incoming_references: vec![],
                        dependents: vec![],
                        insert_priority: 0,
                        delete_priority: 1,
                        nn_relationship: None,
                    },
                    schema_diff: EntitySchemaDiff::default(),
                    data_preview: EntityDataPreview {
                        entity_name: "parent".to_string(),
                        origin_count: 3,
                        target_count: 3,
                        origin_records: vec![
                            serde_json::json!({"parentid": "p1", "name": "Parent 1 Updated", "statecode": 0}),
                            serde_json::json!({"parentid": "p2", "name": "Parent 2 Updated", "statecode": 0}),
                            serde_json::json!({"parentid": "p3", "name": "Parent 3 New", "statecode": 0}),
                        ],
                        target_records: vec![
                            TargetRecord { id: "p1".to_string(), name: Some("Parent 1".to_string()), junction_parent_id: None, junction_target_id: None },
                            TargetRecord { id: "p2".to_string(), name: Some("Parent 2".to_string()), junction_parent_id: None, junction_target_id: None },
                            TargetRecord { id: "p4".to_string(), name: Some("Parent 4 ToDeactivate".to_string()), junction_parent_id: None, junction_target_id: None },
                        ],
                        junction_target_raw: vec![],
                    },
                    nulled_lookups: vec![],
                },
            ],
            detected_junctions: vec![],
            has_schema_changes: false,
            total_delete_count: 1,
            total_insert_count: 1,
        }
    }

    #[test]
    fn test_build_insert_operations_origin_only() {
        let sync_plan = make_test_plan_with_overlap();
        let insert_ops = build_insert_operations(&sync_plan);

        // Only p3 should be created (origin-only)
        assert_eq!(insert_ops.len(), 1);

        match &insert_ops[0] {
            Operation::Create { entity, data } => {
                assert_eq!(entity, "parents");
                assert_eq!(data["parentid"], "p3");
                assert_eq!(data["name"], "Parent 3 New");
            }
            _ => panic!("Expected Create operation"),
        }
    }

    #[test]
    fn test_build_update_operations_both_exist() {
        let sync_plan = make_test_plan_with_overlap();
        let update_ops = build_update_operations(&sync_plan);

        // p1 and p2 should be updated (exist in both)
        assert_eq!(update_ops.len(), 2);

        let ids: Vec<&str> = update_ops
            .iter()
            .map(|op| match op {
                Operation::Update { id, .. } => id.as_str(),
                _ => panic!("Expected Update operation"),
            })
            .collect();

        assert!(ids.contains(&"p1"));
        assert!(ids.contains(&"p2"));
    }

    #[test]
    fn test_build_update_operations_includes_statecode() {
        let sync_plan = make_test_plan_with_overlap();
        let update_ops = build_update_operations(&sync_plan);

        // Updates should include statecode: 0 to reactivate inactive records
        for op in &update_ops {
            match op {
                Operation::Update { data, .. } => {
                    assert_eq!(data["statecode"], 0);
                }
                _ => panic!("Expected Update operation"),
            }
        }
    }

    #[test]
    fn test_build_deactivate_operations_target_only_with_overlap() {
        let sync_plan = make_test_plan_with_overlap();
        let deactivate_ops = build_deactivate_operations(&sync_plan);

        // Only p4 should be deactivated (target-only)
        assert_eq!(deactivate_ops.len(), 1);

        match &deactivate_ops[0] {
            Operation::Update { entity, id, data } => {
                assert_eq!(entity, "parents");
                assert_eq!(id, "p4");
                assert_eq!(data["statecode"], 1);
            }
            _ => panic!("Expected Update operation for deactivation"),
        }
    }

    #[test]
    fn test_build_delete_operations_junction_with_target_records() {
        let mut sync_plan = make_test_plan_with_junction();

        // Add raw junction target records with FK values
        // (junction_target_raw is used, not target_records)
        sync_plan.entity_plans[2].data_preview.junction_target_raw = vec![
            serde_json::json!({"accountid": "acc-1", "contactid": "con-1"}),
            serde_json::json!({"accountid": "acc-2", "contactid": "con-2"}),
        ];

        let delete_ops = build_delete_operations(&sync_plan);

        // Should have 2 DisassociateRef operations for the junction entity
        assert_eq!(delete_ops.len(), 2);

        for op in &delete_ops {
            match op {
                Operation::DisassociateRef { entity, entity_ref, navigation_property, target_id } => {
                    // entity should be parent entity set (accounts)
                    assert_eq!(entity, "accounts");
                    assert_eq!(navigation_property, "contact_account_association");
                    // Check FK values are extracted correctly
                    assert!(entity_ref == "acc-1" || entity_ref == "acc-2");
                    assert!(target_id == "con-1" || target_id == "con-2");
                }
                _ => panic!("Expected DisassociateRef operation, got {:?}", op),
            }
        }
    }

    #[test]
    fn test_complete_sync_operation_counts() {
        // Test that the combination of all operation types covers all records correctly
        let sync_plan = make_test_plan_with_overlap();

        let insert_ops = build_insert_operations(&sync_plan);
        let update_ops = build_update_operations(&sync_plan);
        let deactivate_ops = build_deactivate_operations(&sync_plan);
        let delete_ops = build_delete_operations(&sync_plan);

        // Origin has 3 records: 2 overlap (update) + 1 origin-only (create)
        assert_eq!(update_ops.len() + insert_ops.len(), 3);

        // Target has 3 records: 2 overlap (update) + 1 target-only (deactivate)
        assert_eq!(update_ops.len() + deactivate_ops.len(), 3);

        // No junction entities, so no deletes
        assert_eq!(delete_ops.len(), 0);
    }
}
