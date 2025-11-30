//! Execution logic for the Entity Sync App
//!
//! This module builds QueueItems from a SyncPlan for execution via the queue system.

use std::collections::HashSet;

use crate::api::operations::{Operation, Operations};
use crate::tui::apps::queue::models::{QueueItem, QueueMetadata};

use super::operation_builder::{
    build_deactivate_operations, build_delete_operations, build_insert_operations,
    build_junction_operations, build_schema_operations, build_update_operations,
    chunk_operations, DEFAULT_BATCH_SIZE,
};
use super::super::types::SyncPlan;

/// Priority levels for sync operations (lower = higher priority)
pub mod priority {
    pub const DELETE: u8 = 16;      // Junction entities only
    pub const DEACTIVATE: u8 = 32;  // Regular entities, target-only records
    pub const SCHEMA: u8 = 64;
    pub const UPDATE: u8 = 80;      // Regular entities, records in both
    pub const INSERT: u8 = 96;      // Regular entities, origin-only records
    pub const JUNCTION: u8 = 128;   // N:N associations
}

/// Result of building sync queue items
#[derive(Debug, Clone)]
pub struct SyncQueueItems {
    /// Delete operation batches (junction entities only)
    pub delete_items: Vec<QueueItem>,
    /// Deactivate operation batches (regular entities, target-only records)
    pub deactivate_items: Vec<QueueItem>,
    /// Schema operation batches
    pub schema_items: Vec<QueueItem>,
    /// Update operation batches (regular entities, records in both origin and target)
    pub update_items: Vec<QueueItem>,
    /// Insert operation batches (regular entities, origin-only records)
    pub insert_items: Vec<QueueItem>,
    /// Junction operation batches (N:N associations)
    pub junction_items: Vec<QueueItem>,
}

impl SyncQueueItems {
    /// Get all queue items in priority order (for sending to queue)
    pub fn all_items(&self) -> Vec<QueueItem> {
        let mut items = Vec::new();
        items.extend(self.delete_items.clone());
        items.extend(self.deactivate_items.clone());
        items.extend(self.schema_items.clone());
        items.extend(self.update_items.clone());
        items.extend(self.insert_items.clone());
        items.extend(self.junction_items.clone());
        items
    }

    /// Get all queue item IDs grouped by phase
    pub fn item_ids(&self) -> SyncQueueItemIds {
        SyncQueueItemIds {
            delete_ids: self.delete_items.iter().map(|i| i.id.clone()).collect(),
            deactivate_ids: self.deactivate_items.iter().map(|i| i.id.clone()).collect(),
            schema_ids: self.schema_items.iter().map(|i| i.id.clone()).collect(),
            update_ids: self.update_items.iter().map(|i| i.id.clone()).collect(),
            insert_ids: self.insert_items.iter().map(|i| i.id.clone()).collect(),
            junction_ids: self.junction_items.iter().map(|i| i.id.clone()).collect(),
        }
    }

    /// Total number of operations across all batches
    pub fn total_operations(&self) -> usize {
        self.delete_items.iter().map(|i| i.operations.len()).sum::<usize>()
            + self.deactivate_items.iter().map(|i| i.operations.len()).sum::<usize>()
            + self.schema_items.iter().map(|i| i.operations.len()).sum::<usize>()
            + self.update_items.iter().map(|i| i.operations.len()).sum::<usize>()
            + self.insert_items.iter().map(|i| i.operations.len()).sum::<usize>()
            + self.junction_items.iter().map(|i| i.operations.len()).sum::<usize>()
    }
}

/// Queue item IDs grouped by phase
#[derive(Debug, Clone)]
pub struct SyncQueueItemIds {
    pub delete_ids: Vec<String>,
    pub deactivate_ids: Vec<String>,
    pub schema_ids: Vec<String>,
    pub update_ids: Vec<String>,
    pub insert_ids: Vec<String>,
    pub junction_ids: Vec<String>,
}

/// Build all queue items for executing a sync plan
pub fn build_sync_queue_items(
    plan: &SyncPlan,
    target_env: &str,
) -> SyncQueueItems {
    // Build operations for each phase
    // Phase 1: Delete (junction entities only)
    let delete_ops = build_delete_operations(plan);
    // Phase 2: Deactivate (regular entities, target-only records)
    let deactivate_ops = build_deactivate_operations(plan);
    // Phase 3: Schema changes
    // TODO: Add solution_name parameter when config is available
    let schema_ops = build_schema_operations(plan, None);
    // Phase 4: Update (regular entities, records in both)
    let update_ops = build_update_operations(plan);
    // Phase 5: Insert/Create (regular entities, origin-only records)
    let insert_ops = build_insert_operations(plan);
    // Phase 6: Junction associations (N:N relationships)
    let junction_ops = build_junction_operations(plan);

    // Chunk into batches
    let delete_batches = chunk_operations(delete_ops, DEFAULT_BATCH_SIZE);
    let deactivate_batches = chunk_operations(deactivate_ops, DEFAULT_BATCH_SIZE);
    let schema_batches = chunk_operations(schema_ops, DEFAULT_BATCH_SIZE);
    let update_batches = chunk_operations(update_ops, DEFAULT_BATCH_SIZE);
    let insert_batches = chunk_operations(insert_ops, DEFAULT_BATCH_SIZE);
    let junction_batches = chunk_operations(junction_ops, DEFAULT_BATCH_SIZE);

    // Build queue items
    let delete_items = build_queue_items_for_phase(
        delete_batches,
        "delete",
        priority::DELETE,
        target_env,
    );
    let deactivate_items = build_queue_items_for_phase(
        deactivate_batches,
        "deactivate",
        priority::DEACTIVATE,
        target_env,
    );
    let schema_items = build_queue_items_for_phase(
        schema_batches,
        "schema",
        priority::SCHEMA,
        target_env,
    );
    let update_items = build_queue_items_for_phase(
        update_batches,
        "update",
        priority::UPDATE,
        target_env,
    );
    let insert_items = build_queue_items_for_phase(
        insert_batches,
        "create",
        priority::INSERT,
        target_env,
    );
    let junction_items = build_queue_items_for_phase(
        junction_batches,
        "junction",
        priority::JUNCTION,
        target_env,
    );

    SyncQueueItems {
        delete_items,
        deactivate_items,
        schema_items,
        update_items,
        insert_items,
        junction_items,
    }
}

/// Build queue items for a specific phase
fn build_queue_items_for_phase(
    batches: Vec<Vec<Operation>>,
    phase_name: &str,
    priority: u8,
    environment_name: &str,
) -> Vec<QueueItem> {
    let total_batches = batches.len();

    batches
        .into_iter()
        .enumerate()
        .map(|(idx, ops)| {
            let entity_type = build_entity_type(phase_name, &ops);
            let description = format!(
                "{} (batch {}/{})",
                phase_name,
                idx + 1,
                total_batches
            );

            let metadata = QueueMetadata {
                source: "Entity Sync".to_string(),
                entity_type,
                description,
                row_number: None,
                environment_name: environment_name.to_string(),
            };

            QueueItem::new(Operations::from(ops), metadata, priority)
        })
        .collect()
}

/// Build entity_type string from operations (e.g., "delete: contact, account")
fn build_entity_type(phase_name: &str, operations: &[Operation]) -> String {
    let unique_entities: HashSet<&str> = operations
        .iter()
        .map(|op| op.entity())
        .collect();

    let mut entities: Vec<&str> = unique_entities.into_iter().collect();
    entities.sort();

    if entities.is_empty() {
        phase_name.to_string()
    } else {
        format!("{}: {}", phase_name, entities.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_build_entity_type_single() {
        let ops = vec![
            Operation::Delete {
                entity: "contacts".to_string(),
                id: "123".to_string(),
            },
            Operation::Delete {
                entity: "contacts".to_string(),
                id: "456".to_string(),
            },
        ];

        let result = build_entity_type("delete", &ops);
        assert_eq!(result, "delete: contacts");
    }

    #[test]
    fn test_build_entity_type_multiple() {
        let ops = vec![
            Operation::Delete {
                entity: "contacts".to_string(),
                id: "123".to_string(),
            },
            Operation::Delete {
                entity: "accounts".to_string(),
                id: "456".to_string(),
            },
            Operation::Delete {
                entity: "contacts".to_string(),
                id: "789".to_string(),
            },
        ];

        let result = build_entity_type("delete", &ops);
        assert_eq!(result, "delete: accounts, contacts");
    }

    #[test]
    fn test_build_entity_type_empty() {
        let ops: Vec<Operation> = vec![];
        let result = build_entity_type("schema", &ops);
        assert_eq!(result, "schema");
    }

    #[test]
    fn test_priority_ordering() {
        assert!(priority::DELETE < priority::DEACTIVATE);
        assert!(priority::DEACTIVATE < priority::SCHEMA);
        assert!(priority::SCHEMA < priority::UPDATE);
        assert!(priority::UPDATE < priority::INSERT);
        assert!(priority::INSERT < priority::JUNCTION);
    }

    #[test]
    fn test_sync_queue_items_total_operations() {
        let items = SyncQueueItems {
            delete_items: vec![
                QueueItem::new(
                    Operations::from(vec![
                        Operation::delete("contacts", "1"),
                        Operation::delete("contacts", "2"),
                    ]),
                    QueueMetadata {
                        source: "test".to_string(),
                        entity_type: "delete".to_string(),
                        description: "test".to_string(),
                        row_number: None,
                        environment_name: "test".to_string(),
                    },
                    priority::DELETE,
                ),
            ],
            deactivate_items: vec![],
            schema_items: vec![],
            update_items: vec![],
            insert_items: vec![
                QueueItem::new(
                    Operations::from(vec![
                        Operation::create("contacts", json!({"name": "test"})),
                    ]),
                    QueueMetadata {
                        source: "test".to_string(),
                        entity_type: "insert".to_string(),
                        description: "test".to_string(),
                        row_number: None,
                        environment_name: "test".to_string(),
                    },
                    priority::INSERT,
                ),
            ],
            junction_items: vec![],
        };

        assert_eq!(items.total_operations(), 3);
    }
}
