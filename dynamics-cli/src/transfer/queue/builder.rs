//! Build QueueItems from ResolvedTransfer

use crate::api::operations::{Operation, Operations};
use crate::transfer::{RecordAction, ResolvedEntity, ResolvedTransfer};
use crate::tui::apps::queue::models::{QueueItem, QueueMetadata};

/// Base priority for transfer operations (start low to maximize priority space)
const BASE_PRIORITY: u8 = 1;

/// Default batch size (operations per queue item)
const DEFAULT_BATCH_SIZE: usize = 50;

/// Options for building queue items
#[derive(Debug, Clone)]
pub struct QueueBuildOptions {
    /// Maximum operations per queue item
    pub batch_size: usize,
}

impl Default for QueueBuildOptions {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }
}

/// Build queue items from a resolved transfer
///
/// Queue items are ordered by:
/// 1. Entity priority (lower = first)
/// 2. Phase: creates before updates
/// 3. Batch number within phase
///
/// Each batch contains up to `batch_size` operations (default 50).
pub fn build_queue_items(
    transfer: &ResolvedTransfer,
    options: &QueueBuildOptions,
) -> Vec<QueueItem> {
    let mut items = Vec::new();

    // Sort entities by priority (ascending - lower priority executes first)
    let mut sorted_entities: Vec<_> = transfer.entities.iter().collect();
    sorted_entities.sort_by_key(|e| e.priority);

    for entity in sorted_entities {
        let entity_items = build_entity_queue_items(entity, transfer, options);
        items.extend(entity_items);
    }

    items
}

/// Phase of operation within an entity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Create,
    Update,
}

impl Phase {
    fn label(&self) -> &'static str {
        match self {
            Phase::Create => "create",
            Phase::Update => "update",
        }
    }

    fn priority_offset(&self) -> u8 {
        match self {
            Phase::Create => 0,
            Phase::Update => 1,
        }
    }
}

fn build_entity_queue_items(
    entity: &ResolvedEntity,
    transfer: &ResolvedTransfer,
    options: &QueueBuildOptions,
) -> Vec<QueueItem> {
    let mut items = Vec::new();

    // Separate creates and updates
    let creates: Vec<_> = entity
        .records
        .iter()
        .filter(|r| r.action == RecordAction::Create)
        .collect();

    let updates: Vec<_> = entity
        .records
        .iter()
        .filter(|r| r.action == RecordAction::Update)
        .collect();

    // Build queue items for creates (phase 1)
    items.extend(build_phase_queue_items(
        entity,
        transfer,
        &creates,
        Phase::Create,
        options,
    ));

    // Build queue items for updates (phase 2)
    items.extend(build_phase_queue_items(
        entity,
        transfer,
        &updates,
        Phase::Update,
        options,
    ));

    items
}

fn build_phase_queue_items(
    entity: &ResolvedEntity,
    transfer: &ResolvedTransfer,
    records: &[&crate::transfer::ResolvedRecord],
    phase: Phase,
    options: &QueueBuildOptions,
) -> Vec<QueueItem> {
    if records.is_empty() {
        return Vec::new();
    }

    // Calculate priority:
    // - Base priority (64)
    // - + entity.priority * 2 (so each entity has room for create/update phases)
    // - + phase offset (0 for create, 1 for update)
    // This ensures: entity1.creates < entity1.updates < entity2.creates < entity2.updates
    let priority = BASE_PRIORITY
        .saturating_add((entity.priority as u8).saturating_mul(2))
        .saturating_add(phase.priority_offset())
        .min(127);

    // Build operations
    let operations: Vec<Operation> = records
        .iter()
        .map(|record| match phase {
            Phase::Create => Operation::create(&entity.entity_name, record.to_json()),
            Phase::Update => Operation::update(
                &entity.entity_name,
                record.source_id.to_string(),
                record.to_json(),
            ),
        })
        .collect();

    // Determine batch size (0 means use all in one batch)
    let batch_size = if options.batch_size == 0 {
        operations.len()
    } else {
        options.batch_size
    };

    // Split into batches
    let chunks: Vec<_> = operations.chunks(batch_size).collect();
    let total_batches = chunks.len();

    chunks
        .into_iter()
        .enumerate()
        .map(|(i, chunk)| {
            let ops = Operations::from_operations(chunk.to_vec());

            let description = if total_batches == 1 {
                format!(
                    "{}: {} {} ({} records)",
                    transfer.config_name,
                    entity.entity_name,
                    phase.label(),
                    chunk.len()
                )
            } else {
                format!(
                    "{}: {} {} {}/{} ({} records)",
                    transfer.config_name,
                    entity.entity_name,
                    phase.label(),
                    i + 1,
                    total_batches,
                    chunk.len()
                )
            };

            let metadata = QueueMetadata {
                source: "Transfer".to_string(),
                entity_type: format!("transfer: {}", entity.entity_name),
                description,
                row_number: None,
                environment_name: transfer.target_env.clone(),
            };

            QueueItem::new(ops, metadata, priority)
        })
        .collect()
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
        accounts.add_record(ResolvedRecord::create(
            Uuid::new_v4(),
            HashMap::from([("name".to_string(), Value::String("Contoso".to_string()))]),
        ));
        accounts.add_record(ResolvedRecord::update(
            Uuid::new_v4(),
            HashMap::from([("name".to_string(), Value::String("Fabrikam".to_string()))]),
        ));
        accounts.add_record(ResolvedRecord::skip(Uuid::new_v4(), HashMap::new()));

        // Entity 2: contacts (priority 2)
        let mut contacts = ResolvedEntity::new("contacts", 2, "contactid");
        contacts.add_record(ResolvedRecord::create(
            Uuid::new_v4(),
            HashMap::from([("fullname".to_string(), Value::String("John Doe".to_string()))]),
        ));
        contacts.add_record(ResolvedRecord::error(Uuid::new_v4(), "transform failed"));

        transfer.add_entity(accounts);
        transfer.add_entity(contacts);
        transfer
    }

    #[test]
    fn test_build_queue_items_phases_separated() {
        let transfer = make_test_transfer();
        let options = QueueBuildOptions { batch_size: 0 }; // No chunking for this test

        let items = build_queue_items(&transfer, &options);

        // accounts: 1 create batch + 1 update batch
        // contacts: 1 create batch (no updates)
        assert_eq!(items.len(), 3);

        // Verify order: accounts creates, accounts updates, contacts creates
        assert!(items[0].metadata.description.contains("accounts"));
        assert!(items[0].metadata.description.contains("create"));
        assert!(items[1].metadata.description.contains("accounts"));
        assert!(items[1].metadata.description.contains("update"));
        assert!(items[2].metadata.description.contains("contacts"));
        assert!(items[2].metadata.description.contains("create"));
    }

    #[test]
    fn test_build_queue_items_priority_ordering() {
        let transfer = make_test_transfer();
        let options = QueueBuildOptions { batch_size: 0 };

        let items = build_queue_items(&transfer, &options);

        // accounts (priority 1): creates=3, updates=4
        // contacts (priority 2): creates=5
        // Formula: BASE(1) + entity.priority * 2 + phase_offset
        assert_eq!(items[0].priority, 3); // accounts create
        assert_eq!(items[1].priority, 4); // accounts update
        assert_eq!(items[2].priority, 5); // contacts create
    }

    #[test]
    fn test_build_queue_items_default_batch_size() {
        let mut transfer = ResolvedTransfer::new("test", "dev", "prod");
        let mut entity = ResolvedEntity::new("accounts", 1, "accountid");

        // Add 120 create records (should split into 3 batches with default size 50)
        for i in 0..120 {
            entity.add_record(ResolvedRecord::create(
                Uuid::new_v4(),
                HashMap::from([("name".to_string(), Value::String(format!("Account {}", i)))]),
            ));
        }
        transfer.add_entity(entity);

        let options = QueueBuildOptions::default(); // batch_size = 50
        let items = build_queue_items(&transfer, &options);

        // 120 creates / 50 = 3 batches
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].operations.len(), 50);
        assert_eq!(items[1].operations.len(), 50);
        assert_eq!(items[2].operations.len(), 20);

        // All should have same priority (same entity, same phase)
        assert_eq!(items[0].priority, items[1].priority);
        assert_eq!(items[1].priority, items[2].priority);
    }

    #[test]
    fn test_build_queue_items_with_custom_batch_size() {
        let mut transfer = ResolvedTransfer::new("test", "dev", "prod");
        let mut entity = ResolvedEntity::new("accounts", 1, "accountid");

        // Add 5 records
        for i in 0..5 {
            entity.add_record(ResolvedRecord::create(
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

        // Verify batch numbering in description
        assert!(items[0].metadata.description.contains("1/3"));
        assert!(items[1].metadata.description.contains("2/3"));
        assert!(items[2].metadata.description.contains("3/3"));
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

    #[test]
    fn test_build_queue_items_metadata() {
        let transfer = make_test_transfer();
        let options = QueueBuildOptions { batch_size: 0 };

        let items = build_queue_items(&transfer, &options);

        assert_eq!(items[0].metadata.source, "Transfer");
        assert_eq!(items[0].metadata.entity_type, "transfer: accounts");
        assert_eq!(items[0].metadata.environment_name, "prod");
        assert!(items[0].metadata.description.contains("test-config"));
    }

    #[test]
    fn test_priority_respects_entity_order() {
        let mut transfer = ResolvedTransfer::new("test", "dev", "prod");

        // Add entities in reverse priority order to ensure sorting works
        let mut contacts = ResolvedEntity::new("contacts", 3, "contactid");
        contacts.add_record(ResolvedRecord::create(Uuid::new_v4(), HashMap::new()));
        transfer.add_entity(contacts);

        let mut accounts = ResolvedEntity::new("accounts", 1, "accountid");
        accounts.add_record(ResolvedRecord::create(Uuid::new_v4(), HashMap::new()));
        transfer.add_entity(accounts);

        let mut leads = ResolvedEntity::new("leads", 2, "leadid");
        leads.add_record(ResolvedRecord::create(Uuid::new_v4(), HashMap::new()));
        transfer.add_entity(leads);

        let options = QueueBuildOptions { batch_size: 0 };
        let items = build_queue_items(&transfer, &options);

        // Should be sorted by priority: accounts(1), leads(2), contacts(3)
        assert!(items[0].metadata.description.contains("accounts"));
        assert!(items[1].metadata.description.contains("leads"));
        assert!(items[2].metadata.description.contains("contacts"));

        // Priorities should be ascending
        assert!(items[0].priority < items[1].priority);
        assert!(items[1].priority < items[2].priority);
    }
}
