//! Execution logic for the Entity Sync App
//!
//! This module builds QueueItems from a SyncPlan for execution via the queue system.

use std::collections::HashSet;

use crate::api::operations::{Operation, Operations};
use crate::tui::apps::queue::models::{QueueItem, QueueMetadata};

use super::operation_builder::{
    build_delete_operations, build_insert_operations, build_junction_operations,
    build_schema_operations, chunk_operations, DEFAULT_BATCH_SIZE,
};
use super::super::types::SyncPlan;

/// Priority levels for sync operations (lower = higher priority)
pub mod priority {
    pub const DELETE: u8 = 32;
    pub const SCHEMA: u8 = 64;
    pub const INSERT: u8 = 96;
    pub const JUNCTION: u8 = 128;
}

/// Result of building sync queue items
#[derive(Debug, Clone)]
pub struct SyncQueueItems {
    /// Delete operation batches
    pub delete_items: Vec<QueueItem>,
    /// Schema operation batches
    pub schema_items: Vec<QueueItem>,
    /// Insert operation batches (non-junction entities)
    pub insert_items: Vec<QueueItem>,
    /// Junction operation batches (N:N associations)
    pub junction_items: Vec<QueueItem>,
}

impl SyncQueueItems {
    /// Get all queue items in priority order (for sending to queue)
    pub fn all_items(&self) -> Vec<QueueItem> {
        let mut items = Vec::new();
        items.extend(self.delete_items.clone());
        items.extend(self.schema_items.clone());
        items.extend(self.insert_items.clone());
        items.extend(self.junction_items.clone());
        items
    }

    /// Get all queue item IDs grouped by phase
    pub fn item_ids(&self) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
        (
            self.delete_items.iter().map(|i| i.id.clone()).collect(),
            self.schema_items.iter().map(|i| i.id.clone()).collect(),
            self.insert_items.iter().map(|i| i.id.clone()).collect(),
            self.junction_items.iter().map(|i| i.id.clone()).collect(),
        )
    }

    /// Total number of operations across all batches
    pub fn total_operations(&self) -> usize {
        self.delete_items.iter().map(|i| i.operations.len()).sum::<usize>()
            + self.schema_items.iter().map(|i| i.operations.len()).sum::<usize>()
            + self.insert_items.iter().map(|i| i.operations.len()).sum::<usize>()
            + self.junction_items.iter().map(|i| i.operations.len()).sum::<usize>()
    }
}

/// Build all queue items for executing a sync plan
pub fn build_sync_queue_items(
    plan: &SyncPlan,
    target_env: &str,
) -> SyncQueueItems {
    // Build operations for each phase
    let delete_ops = build_delete_operations(plan);
    // TODO: Add solution_name parameter when config is available
    let schema_ops = build_schema_operations(plan, None);
    let insert_ops = build_insert_operations(plan);
    let junction_ops = build_junction_operations(plan);

    // Chunk into batches
    let delete_batches = chunk_operations(delete_ops, DEFAULT_BATCH_SIZE);
    let schema_batches = chunk_operations(schema_ops, DEFAULT_BATCH_SIZE);
    let insert_batches = chunk_operations(insert_ops, DEFAULT_BATCH_SIZE);
    let junction_batches = chunk_operations(junction_ops, DEFAULT_BATCH_SIZE);

    // Build queue items
    let delete_items = build_queue_items_for_phase(
        delete_batches,
        "delete",
        priority::DELETE,
        target_env,
    );
    let schema_items = build_queue_items_for_phase(
        schema_batches,
        "schema",
        priority::SCHEMA,
        target_env,
    );
    let insert_items = build_queue_items_for_phase(
        insert_batches,
        "insert",
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
        schema_items,
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
        assert!(priority::DELETE < priority::SCHEMA);
        assert!(priority::SCHEMA < priority::INSERT);
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
            schema_items: vec![],
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
