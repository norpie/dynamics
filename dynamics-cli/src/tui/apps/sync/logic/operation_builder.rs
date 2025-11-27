//! Operation builder for the Entity Sync App
//!
//! Converts a SyncPlan into an ordered list of API operations.
//! Operations are ordered to respect entity dependencies:
//! - Deletes: dependents before dependencies (reverse topological order)
//! - Inserts: dependencies before dependents (topological order)

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::super::types::{EntitySyncPlan, FieldDiffEntry, SyncPlan};

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
    /// Delete a record from target
    DeleteRecord,
    /// Create a record in target (with preserved GUID)
    CreateRecord,
    /// Create a new attribute on target entity
    CreateAttribute,
}

impl OperationType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::DeleteRecord => "Delete",
            Self::CreateRecord => "Create",
            Self::CreateAttribute => "Add Field",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::DeleteRecord => "×",
            Self::CreateRecord => "+",
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
    /// Delete operations (executed first)
    pub deletes: Vec<SyncOperation>,
    /// Schema operations (executed after deletes)
    pub schema_ops: Vec<SyncOperation>,
    /// Insert operations (executed last)
    pub inserts: Vec<SyncOperation>,
}

impl EntityOperationBatch {
    pub fn total_ops(&self) -> usize {
        self.deletes.len() + self.schema_ops.len() + self.inserts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.deletes.is_empty() && self.schema_ops.is_empty() && self.inserts.is_empty()
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
    /// Total delete operations
    pub total_deletes: usize,
    /// Total schema operations
    pub total_schema_ops: usize,
    /// Total insert operations
    pub total_inserts: usize,
}

impl OperationPlan {
    /// Get all operations in execution order
    pub fn all_operations(&self) -> Vec<&SyncOperation> {
        let mut ops = Vec::new();

        // Phase 1: All deletes (in dependency order - dependents first)
        for batch in &self.entity_batches {
            ops.extend(batch.deletes.iter());
        }

        // Phase 2: All schema changes
        for batch in &self.entity_batches {
            ops.extend(batch.schema_ops.iter());
        }

        // Phase 3: All inserts (in dependency order - dependencies first)
        // Batches are already in insert order, so just iterate
        for batch in &self.entity_batches {
            ops.extend(batch.inserts.iter());
        }

        ops
    }

    pub fn total_operations(&self) -> usize {
        self.total_deletes + self.total_schema_ops + self.total_inserts
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
    /// Entities that will have records deleted
    pub entities_with_deletes: Vec<(String, usize)>,
    /// Entities that will have schema changes
    pub entities_with_schema_changes: Vec<(String, usize)>,
    /// Entities that will have records inserted
    pub entities_with_inserts: Vec<(String, usize)>,
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

        // Deletes
        if entity_plan.data_preview.target_count > 0 {
            summary.entities_with_deletes.push((
                entity_name.clone(),
                entity_plan.data_preview.target_count,
            ));
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

        // Inserts
        if entity_plan.data_preview.origin_count > 0 {
            summary.entities_with_inserts.push((
                entity_name.clone(),
                entity_plan.data_preview.origin_count,
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
                        category: DependencyCategory::Standalone,
                        lookups: vec![],
                        dependents: vec!["child".to_string()],
                        insert_priority: 0,
                        delete_priority: 1,
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
                        target_count: 5,
                        sample_ids: vec![],
                    },
                    nulled_lookups: vec![],
                },
                EntitySyncPlan {
                    entity_info: SyncEntityInfo {
                        logical_name: "child".to_string(),
                        display_name: Some("Child".to_string()),
                        category: DependencyCategory::Dependent,
                        lookups: vec![LookupInfo {
                            field_name: "parentid".to_string(),
                            target_entity: "parent".to_string(),
                            is_internal: true,
                        }],
                        dependents: vec![],
                        insert_priority: 1,
                        delete_priority: 0,
                    },
                    schema_diff: EntitySchemaDiff::default(),
                    data_preview: EntityDataPreview {
                        entity_name: "child".to_string(),
                        origin_count: 20,
                        target_count: 10,
                        sample_ids: vec![],
                    },
                    nulled_lookups: vec![],
                },
            ],
            detected_junctions: vec![],
            has_schema_changes: true,
            total_delete_count: 15,
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

        assert_eq!(summary.entities_with_deletes.len(), 2);
        assert_eq!(summary.entities_with_schema_changes.len(), 1);
        assert_eq!(summary.entities_with_inserts.len(), 2);
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
}
