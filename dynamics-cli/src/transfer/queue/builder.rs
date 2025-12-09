//! Build QueueItems from ResolvedTransfer

use crate::api::operations::{Operation, Operations};
use crate::transfer::{RecordAction, ResolvedEntity, ResolvedTransfer};
use crate::tui::apps::queue::models::{QueueItem, QueueMetadata};

/// Base priority for transfer upserts
const BASE_PRIORITY: u8 = 64;

/// Options for building queue items
#[derive(Debug, Clone, Default)]
pub struct QueueBuildOptions {
    /// Maximum operations per queue item (0 = unlimited)
    pub batch_size: usize,
}

/// Build queue items from a resolved transfer
///
/// Returns one QueueItem per entity (or multiple if batch_size is set).
/// Only records with `RecordAction::Upsert` are included.
pub fn build_queue_items(
    transfer: &ResolvedTransfer,
    options: &QueueBuildOptions,
) -> Vec<QueueItem> {
    let mut items = Vec::new();

    for entity in &transfer.entities {
        let entity_items = build_entity_queue_items(entity, transfer, options);
        items.extend(entity_items);
    }

    items
}

fn build_entity_queue_items(
    entity: &ResolvedEntity,
    transfer: &ResolvedTransfer,
    options: &QueueBuildOptions,
) -> Vec<QueueItem> {
    let upsert_records: Vec<_> = entity
        .records
        .iter()
        .filter(|r| r.action == RecordAction::Upsert)
        .collect();

    if upsert_records.is_empty() {
        return Vec::new();
    }

    // Calculate priority: base + entity priority (capped at 127)
    let priority = BASE_PRIORITY.saturating_add(entity.priority as u8).min(127);

    // Build operations
    let operations: Vec<Operation> = upsert_records
        .iter()
        .map(|record| {
            Operation::upsert(
                &entity.entity_name,
                &entity.primary_key_field,
                record.source_id.to_string(),
                record.to_json(),
            )
        })
        .collect();

    // Split into batches if batch_size is set
    if options.batch_size > 0 && operations.len() > options.batch_size {
        let chunks: Vec<_> = operations.chunks(options.batch_size).collect();
        let total_batches = chunks.len();

        chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| {
                let ops = Operations::from_operations(chunk.to_vec());
                let metadata = QueueMetadata {
                    source: "Transfer".to_string(),
                    entity_type: format!("upsert: {}", entity.entity_name),
                    description: format!(
                        "{} batch {}/{} ({} records)",
                        transfer.config_name,
                        i + 1,
                        total_batches,
                        chunk.len()
                    ),
                    row_number: None,
                    environment_name: transfer.target_env.clone(),
                };
                QueueItem::new(ops, metadata, priority)
            })
            .collect()
    } else {
        // Single queue item for all operations
        let ops = Operations::from_operations(operations);
        let metadata = QueueMetadata {
            source: "Transfer".to_string(),
            entity_type: format!("upsert: {}", entity.entity_name),
            description: format!(
                "{}: {} ({} records)",
                transfer.config_name,
                entity.entity_name,
                upsert_records.len()
            ),
            row_number: None,
            environment_name: transfer.target_env.clone(),
        };
        vec![QueueItem::new(ops, metadata, priority)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::{ResolvedRecord, Value};
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_test_transfer() -> ResolvedTransfer {
        let mut transfer = ResolvedTransfer::new("test-config", "dev", "prod");

        // Entity 1: accounts (priority 1)
        let mut accounts = ResolvedEntity::new("accounts", 1, "accountid");
        accounts.add_record(ResolvedRecord::upsert(
            Uuid::new_v4(),
            HashMap::from([("name".to_string(), Value::String("Contoso".to_string()))]),
        ));
        accounts.add_record(ResolvedRecord::upsert(
            Uuid::new_v4(),
            HashMap::from([("name".to_string(), Value::String("Fabrikam".to_string()))]),
        ));
        accounts.add_record(ResolvedRecord::skip(Uuid::new_v4(), HashMap::new()));

        // Entity 2: contacts (priority 2)
        let mut contacts = ResolvedEntity::new("contacts", 2, "contactid");
        contacts.add_record(ResolvedRecord::upsert(
            Uuid::new_v4(),
            HashMap::from([("fullname".to_string(), Value::String("John Doe".to_string()))]),
        ));
        contacts.add_record(ResolvedRecord::error(Uuid::new_v4(), "transform failed"));

        transfer.add_entity(accounts);
        transfer.add_entity(contacts);
        transfer
    }

    #[test]
    fn test_build_queue_items_creates_one_per_entity() {
        let transfer = make_test_transfer();
        let options = QueueBuildOptions::default();

        let items = build_queue_items(&transfer, &options);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].operations.len(), 2); // 2 upserts, 1 skip excluded
        assert_eq!(items[1].operations.len(), 1); // 1 upsert, 1 error excluded
    }

    #[test]
    fn test_build_queue_items_priority_ordering() {
        let transfer = make_test_transfer();
        let options = QueueBuildOptions::default();

        let items = build_queue_items(&transfer, &options);

        // accounts (priority 1) → 64 + 1 = 65
        // contacts (priority 2) → 64 + 2 = 66
        assert_eq!(items[0].priority, 65);
        assert_eq!(items[1].priority, 66);
    }

    #[test]
    fn test_build_queue_items_metadata() {
        let transfer = make_test_transfer();
        let options = QueueBuildOptions::default();

        let items = build_queue_items(&transfer, &options);

        assert_eq!(items[0].metadata.source, "Transfer");
        assert_eq!(items[0].metadata.entity_type, "upsert: accounts");
        assert_eq!(items[0].metadata.environment_name, "prod");
        assert!(items[0].metadata.description.contains("test-config"));
    }

    #[test]
    fn test_build_queue_items_with_batch_size() {
        let mut transfer = ResolvedTransfer::new("test", "dev", "prod");
        let mut entity = ResolvedEntity::new("accounts", 1, "accountid");

        // Add 5 records
        for i in 0..5 {
            entity.add_record(ResolvedRecord::upsert(
                Uuid::new_v4(),
                HashMap::from([("name".to_string(), Value::String(format!("Account {}", i)))]),
            ));
        }
        transfer.add_entity(entity);

        let options = QueueBuildOptions { batch_size: 2 };
        let items = build_queue_items(&transfer, &options);

        // 5 records / batch_size 2 = 3 batches
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].operations.len(), 2);
        assert_eq!(items[1].operations.len(), 2);
        assert_eq!(items[2].operations.len(), 1);
    }

    #[test]
    fn test_build_queue_items_empty_entity_skipped() {
        let mut transfer = ResolvedTransfer::new("test", "dev", "prod");
        let mut entity = ResolvedEntity::new("accounts", 1, "accountid");

        // Only skip/error records
        entity.add_record(ResolvedRecord::skip(Uuid::new_v4(), HashMap::new()));
        entity.add_record(ResolvedRecord::error(Uuid::new_v4(), "error"));
        transfer.add_entity(entity);

        let options = QueueBuildOptions::default();
        let items = build_queue_items(&transfer, &options);

        assert!(items.is_empty());
    }
}
