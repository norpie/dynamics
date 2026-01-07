//! Build QueueItems from ResolvedTransfer

use std::collections::HashMap;

use crate::api::operations::{Operation, Operations};
use crate::transfer::{LookupBindingContext, OrphanAction, RecordAction, ResolvedEntity, ResolvedRecord, ResolvedTransfer, Value};
use crate::tui::apps::queue::models::{QueueItem, QueueMetadata};

/// Base priority for transfer operations (start low to maximize priority space)
const BASE_PRIORITY: u8 = 1;

/// Default batch size (operations per queue item)
const DEFAULT_BATCH_SIZE: usize = 50;

/// Prepare a record's fields for API submission
///
/// Converts lookup fields to @odata.bind format when lookup context is available.
/// Non-lookup fields pass through unchanged.
///
/// For Update operations with `changed_fields` set, only the changed fields are
/// included in the payload (partial update). This reduces payload size and avoids
/// unnecessary writes to unchanged fields.
///
/// When `skip_state_fields` is true, statecode and statuscode fields are excluded
/// from the payload. This is used for CREATE operations on inactive records, which
/// must be created as active first, then deactivated in a separate operation.
fn prepare_payload(
    record: &ResolvedRecord,
    lookup_ctx: Option<&LookupBindingContext>,
    skip_state_fields: bool,
) -> serde_json::Value {
    let mut obj = serde_json::Map::new();

    for (field_name, value) in &record.fields {
        // Skip statecode/statuscode for inactive records being created
        // (they must be created as active first, then deactivated separately)
        if skip_state_fields && (field_name == "statecode" || field_name == "statuscode") {
            continue;
        }

        // For partial updates, skip fields that haven't changed
        if let Some(ref changed) = record.changed_fields {
            if !changed.contains(field_name) {
                continue;
            }
        }

        // Check if this is a lookup field that needs @odata.bind
        if let Some(ctx) = lookup_ctx {
            if let Some(binding_info) = ctx.get(field_name) {
                // Handle null lookup values - skip the field entirely
                if matches!(value, Value::Null) {
                    continue;
                }

                // Try to extract GUID from the value
                let guid_str = match value {
                    Value::Guid(guid) => Some(guid.to_string()),
                    Value::String(s) if uuid::Uuid::parse_str(s).is_ok() => Some(s.clone()),
                    _ => None,
                };

                if let Some(guid) = guid_str {
                    let bind_key = format!("{}@odata.bind", binding_info.schema_name);
                    let bind_value = format!("/{}({})", binding_info.target_entity_set, guid);
                    obj.insert(bind_key, serde_json::Value::String(bind_value));
                    continue;
                }
                // If value isn't a GUID, fall through to normal handling
            }
        }

        // Not a lookup or no context - insert normally
        obj.insert(field_name.clone(), value.to_json());
    }

    serde_json::Value::Object(obj)
}

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
    /// Handle target-only records first (delete/deactivate)
    TargetOnly,
    /// Then create new records
    Create,
    /// Deactivate newly created records that were inactive in source
    PostCreateDeactivate,
    /// Finally update existing records
    Update,
}

impl Phase {
    fn label(&self) -> &'static str {
        match self {
            Phase::TargetOnly => "target-only",
            Phase::Create => "create",
            Phase::PostCreateDeactivate => "post-create-deactivate",
            Phase::Update => "update",
        }
    }

    fn priority_offset(&self) -> u8 {
        match self {
            Phase::TargetOnly => 0,
            Phase::Create => 1,
            Phase::PostCreateDeactivate => 2,
            Phase::Update => 3,
        }
    }
}

fn build_entity_queue_items(
    entity: &ResolvedEntity,
    transfer: &ResolvedTransfer,
    options: &QueueBuildOptions,
) -> Vec<QueueItem> {
    let mut items = Vec::new();

    // Handle delete records first (phase 0)
    let deletes: Vec<_> = entity
        .records
        .iter()
        .filter(|r| r.action == RecordAction::Delete)
        .collect();

    if !deletes.is_empty() {
        items.extend(build_delete_queue_items(
            entity,
            transfer,
            &deletes,
            options,
        ));
    }

    // Handle deactivate records (also phase 0)
    let deactivates: Vec<_> = entity
        .records
        .iter()
        .filter(|r| r.action == RecordAction::Deactivate)
        .collect();

    if !deactivates.is_empty() {
        items.extend(build_deactivate_queue_items(
            entity,
            transfer,
            &deactivates,
            options,
        ));
    }

    // Build queue items for creates (phase 1) - only if creates are enabled
    if entity.operation_filter.creates {
        let creates: Vec<_> = entity
            .records
            .iter()
            .filter(|r| r.action == RecordAction::Create)
            .collect();

        items.extend(build_phase_queue_items(
            entity,
            transfer,
            &creates,
            Phase::Create,
            options,
        ));

        // Build queue items for post-create deactivation (phase 2)
        // This handles records that were inactive in source - they must be created
        // as active first, then deactivated in a separate operation
        items.extend(build_post_create_deactivate_queue_items(
            entity,
            transfer,
            &creates,
            options,
        ));
    }

    // Build queue items for updates (phase 3) - only if updates are enabled
    if entity.operation_filter.updates {
        let updates: Vec<_> = entity
            .records
            .iter()
            .filter(|r| r.action == RecordAction::Update)
            .collect();

        items.extend(build_phase_queue_items(
            entity,
            transfer,
            &updates,
            Phase::Update,
            options,
        ));
    }

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
    // - Base priority (1)
    // - + entity.priority * 4 (so each entity has room for target-only/create/post-create-deactivate/update phases)
    // - + phase offset (0 for target-only, 1 for create, 2 for post-create-deactivate, 3 for update)
    // This ensures: entity1.target-only < entity1.creates < entity1.post-create-deactivate < entity1.updates < entity2.target-only ...
    let priority = BASE_PRIORITY
        .saturating_add((entity.priority as u8).saturating_mul(4))
        .saturating_add(phase.priority_offset())
        .min(127);

    // Build operations with @odata.bind for lookup fields
    let lookup_ctx = entity.lookup_context.as_ref();
    // Use entity_set_name for API calls (required by OData), fallback to entity_name
    let entity_set = entity
        .entity_set_name
        .as_ref()
        .unwrap_or(&entity.entity_name);
    let operations: Vec<Operation> = records
        .iter()
        .map(|record| {
            // For CREATE operations on inactive records, skip statecode/statuscode
            // (they must be created as active first, then deactivated separately)
            let is_inactive = record
                .fields
                .get("statecode")
                .and_then(|v| v.as_int())
                .map(|s| s != 0)
                .unwrap_or(false);
            let skip_state_fields = phase == Phase::Create && is_inactive;

            let payload = prepare_payload(record, lookup_ctx, skip_state_fields);
            match phase {
                Phase::TargetOnly | Phase::PostCreateDeactivate => {
                    // TargetOnly and PostCreateDeactivate are handled separately
                    // This branch shouldn't be reached, but we handle it anyway
                    Operation::delete(entity_set, record.source_id.to_string())
                }
                Phase::Create => Operation::create(entity_set, payload),
                Phase::Update => Operation::update(
                    entity_set,
                    record.source_id.to_string(),
                    payload,
                ),
            }
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

/// Build queue items for deactivating newly created inactive records
///
/// When a source record is inactive (statecode != 0), we cannot create it directly
/// in inactive state. Instead, we create it as active, then deactivate it in a
/// separate operation. This function builds those deactivation operations.
fn build_post_create_deactivate_queue_items(
    entity: &ResolvedEntity,
    transfer: &ResolvedTransfer,
    records: &[&ResolvedRecord],
    options: &QueueBuildOptions,
) -> Vec<QueueItem> {
    // Filter to only inactive CREATE records
    let inactive_creates: Vec<_> = records
        .iter()
        .filter(|r| r.action == RecordAction::Create)
        .filter(|r| {
            r.fields
                .get("statecode")
                .and_then(|v| v.as_int())
                .map(|s| s != 0)
                .unwrap_or(false)
        })
        .collect();

    if inactive_creates.is_empty() {
        return Vec::new();
    }

    // Calculate priority (PostCreateDeactivate phase - after creates, before updates)
    let priority = BASE_PRIORITY
        .saturating_add((entity.priority as u8).saturating_mul(4))
        .saturating_add(Phase::PostCreateDeactivate.priority_offset())
        .min(127);

    // Use entity_set_name for API calls
    let entity_set = entity
        .entity_set_name
        .as_ref()
        .unwrap_or(&entity.entity_name);

    // Build PATCH operations with statecode + statuscode
    let operations: Vec<Operation> = inactive_creates
        .iter()
        .map(|record| {
            let mut payload = serde_json::Map::new();
            if let Some(statecode) = record.fields.get("statecode") {
                payload.insert("statecode".to_string(), statecode.to_json());
            }
            if let Some(statuscode) = record.fields.get("statuscode") {
                payload.insert("statuscode".to_string(), statuscode.to_json());
            }
            Operation::update(
                entity_set,
                record.source_id.to_string(),
                serde_json::Value::Object(payload),
            )
        })
        .collect();

    // Determine batch size
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
                    Phase::PostCreateDeactivate.label(),
                    chunk.len()
                )
            } else {
                format!(
                    "{}: {} {} {}/{} ({} records)",
                    transfer.config_name,
                    entity.entity_name,
                    Phase::PostCreateDeactivate.label(),
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

/// Build queue items for delete records
fn build_delete_queue_items(
    entity: &ResolvedEntity,
    transfer: &ResolvedTransfer,
    records: &[&ResolvedRecord],
    options: &QueueBuildOptions,
) -> Vec<QueueItem> {
    if records.is_empty() {
        return Vec::new();
    }

    // Calculate priority (phase 0 - deletes first)
    let priority = BASE_PRIORITY
        .saturating_add((entity.priority as u8).saturating_mul(4))
        .saturating_add(Phase::TargetOnly.priority_offset())
        .min(127);

    // Use entity_set_name for API calls
    let entity_set = entity
        .entity_set_name
        .as_ref()
        .unwrap_or(&entity.entity_name);

    // Build delete operations
    let operations: Vec<Operation> = records
        .iter()
        .map(|record| Operation::delete(entity_set, record.source_id.to_string()))
        .collect();

    // Determine batch size
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
                    "{}: {} delete ({} records)",
                    transfer.config_name,
                    entity.entity_name,
                    chunk.len()
                )
            } else {
                format!(
                    "{}: {} delete batch {}/{} ({} records)",
                    transfer.config_name,
                    entity.entity_name,
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

/// Build queue items for deactivate records
fn build_deactivate_queue_items(
    entity: &ResolvedEntity,
    transfer: &ResolvedTransfer,
    records: &[&ResolvedRecord],
    options: &QueueBuildOptions,
) -> Vec<QueueItem> {
    if records.is_empty() {
        return Vec::new();
    }

    // Calculate priority (phase 0 - deactivates first)
    let priority = BASE_PRIORITY
        .saturating_add((entity.priority as u8).saturating_mul(4))
        .saturating_add(Phase::TargetOnly.priority_offset())
        .min(127);

    // Use entity_set_name for API calls
    let entity_set = entity
        .entity_set_name
        .as_ref()
        .unwrap_or(&entity.entity_name);

    // Build deactivate operations (PATCH with statecode = 1)
    let operations: Vec<Operation> = records
        .iter()
        .map(|record| {
            let mut payload = serde_json::Map::new();
            payload.insert("statecode".to_string(), serde_json::Value::Number(1.into()));
            Operation::update(
                entity_set,
                record.source_id.to_string(),
                serde_json::Value::Object(payload),
            )
        })
        .collect();

    // Determine batch size
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
                    "{}: {} deactivate ({} records)",
                    transfer.config_name,
                    entity.entity_name,
                    chunk.len()
                )
            } else {
                format!(
                    "{}: {} deactivate batch {}/{} ({} records)",
                    transfer.config_name,
                    entity.entity_name,
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

/// Build queue items for target-only records based on operation_filter config
fn build_target_only_queue_items(
    entity: &ResolvedEntity,
    transfer: &ResolvedTransfer,
    records: &[&ResolvedRecord],
    options: &QueueBuildOptions,
) -> Vec<QueueItem> {
    if records.is_empty() {
        return Vec::new();
    }

    // Get the orphan action (deletes takes precedence over deactivates)
    let orphan_action = match entity.operation_filter.orphan_action() {
        Some(action) => action,
        None => return Vec::new(), // No orphan action configured
    };

    // Calculate priority (same as build_phase_queue_items but for phase 0)
    let priority = BASE_PRIORITY
        .saturating_add((entity.priority as u8).saturating_mul(4))
        .saturating_add(Phase::TargetOnly.priority_offset())
        .min(127);

    // Use entity_set_name for API calls
    let entity_set = entity
        .entity_set_name
        .as_ref()
        .unwrap_or(&entity.entity_name);

    // Build operations based on orphan action
    let operations: Vec<Operation> = records
        .iter()
        .map(|record| {
            match orphan_action {
                OrphanAction::Delete => {
                    Operation::delete(entity_set, record.source_id.to_string())
                }
                OrphanAction::Deactivate => {
                    // PATCH with statecode = 1 (inactive)
                    let mut payload = serde_json::Map::new();
                    payload.insert("statecode".to_string(), serde_json::Value::Number(1.into()));
                    Operation::update(
                        entity_set,
                        record.source_id.to_string(),
                        serde_json::Value::Object(payload),
                    )
                }
            }
        })
        .collect();

    // Determine batch size
    let batch_size = if options.batch_size == 0 {
        operations.len()
    } else {
        options.batch_size
    };

    // Split into batches
    let chunks: Vec<_> = operations.chunks(batch_size).collect();
    let total_batches = chunks.len();

    let action_label = match orphan_action {
        OrphanAction::Delete => "delete",
        OrphanAction::Deactivate => "deactivate",
    };

    chunks
        .into_iter()
        .enumerate()
        .map(|(i, chunk)| {
            let ops = Operations::from_operations(chunk.to_vec());

            let description = if total_batches == 1 {
                format!(
                    "{}: {} {} ({} target-only records)",
                    transfer.config_name,
                    entity.entity_name,
                    action_label,
                    chunk.len()
                )
            } else {
                format!(
                    "{}: {} {} {}/{} ({} target-only records)",
                    transfer.config_name,
                    entity.entity_name,
                    action_label,
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
    use std::collections::{HashMap, HashSet};
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

        // accounts (priority 1): creates, updates
        // contacts (priority 2): creates
        // Formula: BASE(1) + entity.priority * 4 + phase_offset (0=target-only, 1=create, 2=post-create-deactivate, 3=update)
        assert_eq!(items[0].priority, 6); // accounts create: 1 + 1*4 + 1 = 6
        assert_eq!(items[1].priority, 8); // accounts update: 1 + 1*4 + 3 = 8
        assert_eq!(items[2].priority, 10); // contacts create: 1 + 2*4 + 1 = 10
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

    #[test]
    fn test_partial_update_only_includes_changed_fields() {
        // Test that update_partial records only include changed fields in payload
        let id = Uuid::new_v4();
        let fields = HashMap::from([
            ("name".to_string(), Value::String("Updated Name".to_string())),
            ("revenue".to_string(), Value::Int(1000000)),
            ("description".to_string(), Value::String("Same description".to_string())),
        ]);
        // Only name and revenue changed, not description
        let changed = HashSet::from(["name".to_string(), "revenue".to_string()]);

        let record = ResolvedRecord::update_partial(id, fields, changed);

        // Build payload
        let payload = prepare_payload(&record, None, false);
        let obj = payload.as_object().unwrap();

        // Should only contain changed fields
        assert_eq!(obj.len(), 2);
        assert!(obj.contains_key("name"));
        assert!(obj.contains_key("revenue"));
        assert!(!obj.contains_key("description")); // Not changed, excluded
    }

    #[test]
    fn test_full_update_includes_all_fields() {
        // Test that regular update (no changed_fields) includes all fields
        let id = Uuid::new_v4();
        let fields = HashMap::from([
            ("name".to_string(), Value::String("Updated Name".to_string())),
            ("revenue".to_string(), Value::Int(1000000)),
            ("description".to_string(), Value::String("Description".to_string())),
        ]);

        let record = ResolvedRecord::update(id, fields);

        // Build payload
        let payload = prepare_payload(&record, None, false);
        let obj = payload.as_object().unwrap();

        // Should contain all fields
        assert_eq!(obj.len(), 3);
        assert!(obj.contains_key("name"));
        assert!(obj.contains_key("revenue"));
        assert!(obj.contains_key("description"));
    }

    #[test]
    fn test_create_includes_all_fields() {
        // Test that create records include all fields (changed_fields is None)
        let id = Uuid::new_v4();
        let fields = HashMap::from([
            ("name".to_string(), Value::String("New Record".to_string())),
            ("revenue".to_string(), Value::Int(500000)),
        ]);

        let record = ResolvedRecord::create(id, fields);

        let payload = prepare_payload(&record, None, false);
        let obj = payload.as_object().unwrap();

        assert_eq!(obj.len(), 2);
        assert!(obj.contains_key("name"));
        assert!(obj.contains_key("revenue"));
    }

    #[test]
    fn test_create_skips_state_fields_when_inactive() {
        // Test that create records skip statecode/statuscode when skip_state_fields is true
        let id = Uuid::new_v4();
        let fields = HashMap::from([
            ("name".to_string(), Value::String("Inactive Record".to_string())),
            ("statecode".to_string(), Value::Int(1)),
            ("statuscode".to_string(), Value::Int(2)),
        ]);

        let record = ResolvedRecord::create(id, fields);

        // With skip_state_fields = true, statecode/statuscode should be excluded
        let payload = prepare_payload(&record, None, true);
        let obj = payload.as_object().unwrap();

        assert_eq!(obj.len(), 1);
        assert!(obj.contains_key("name"));
        assert!(!obj.contains_key("statecode"));
        assert!(!obj.contains_key("statuscode"));
    }

    #[test]
    fn test_create_includes_state_fields_when_active() {
        // Test that create records include statuscode when statecode is 0 (active)
        let id = Uuid::new_v4();
        let fields = HashMap::from([
            ("name".to_string(), Value::String("Active Record".to_string())),
            ("statecode".to_string(), Value::Int(0)),
            ("statuscode".to_string(), Value::Int(170590005)), // Non-default active status
        ]);

        let record = ResolvedRecord::create(id, fields);

        // With skip_state_fields = false, all fields should be included
        let payload = prepare_payload(&record, None, false);
        let obj = payload.as_object().unwrap();

        assert_eq!(obj.len(), 3);
        assert!(obj.contains_key("name"));
        assert!(obj.contains_key("statecode"));
        assert!(obj.contains_key("statuscode"));
    }

    #[test]
    fn test_post_create_deactivate_queue_items_generated_for_inactive_records() {
        // Test that inactive records generate both a create and a post-create-deactivate queue item
        let mut transfer = ResolvedTransfer::new("test", "dev", "prod");
        let mut entity = ResolvedEntity::new("accounts", 1, "accountid");

        // Add an inactive create record
        entity.add_record(ResolvedRecord::create(
            Uuid::new_v4(),
            HashMap::from([
                ("name".to_string(), Value::String("Inactive Account".to_string())),
                ("statecode".to_string(), Value::Int(1)),
                ("statuscode".to_string(), Value::Int(2)),
            ]),
        ));

        // Add an active create record
        entity.add_record(ResolvedRecord::create(
            Uuid::new_v4(),
            HashMap::from([
                ("name".to_string(), Value::String("Active Account".to_string())),
            ]),
        ));

        transfer.add_entity(entity);

        let options = QueueBuildOptions { batch_size: 0 };
        let items = build_queue_items(&transfer, &options);

        // Should have 2 items: create batch (2 records) and post-create-deactivate batch (1 record)
        assert_eq!(items.len(), 2);

        // First item: creates (2 records)
        assert!(items[0].metadata.description.contains("create"));
        assert!(items[0].metadata.description.contains("2 records"));
        assert_eq!(items[0].priority, 6); // 1 + 1*4 + 1 = 6

        // Second item: post-create-deactivate (1 inactive record)
        assert!(items[1].metadata.description.contains("post-create-deactivate"));
        assert!(items[1].metadata.description.contains("1 records"));
        assert_eq!(items[1].priority, 7); // 1 + 1*4 + 2 = 7

        // Verify the post-create-deactivate operation has the right data
        let deactivate_op = &items[1].operations.operations()[0];
        if let Operation::Update { data, .. } = deactivate_op {
            let obj = data.as_object().unwrap();
            assert!(obj.contains_key("statecode"));
            assert!(obj.contains_key("statuscode"));
            assert_eq!(obj.get("statecode").unwrap(), 1);
            assert_eq!(obj.get("statuscode").unwrap(), 2);
        } else {
            panic!("Expected Update operation");
        }
    }
}
