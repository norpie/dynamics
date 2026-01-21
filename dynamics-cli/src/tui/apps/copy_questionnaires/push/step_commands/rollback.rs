use super::execution::BATCH_CHUNK_SIZE;
/// Rollback operations for cleaning up partially created entities
use crate::api::ResilienceConfig;
use crate::api::operations::{Operation, Operations};
use std::fs::File;
use std::io::Write;

/// Export orphaned entities to CSV for manual cleanup
fn export_orphaned_entities_csv(entities: &[(String, String)]) -> Result<String, String> {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("orphaned_entities_{}.csv", timestamp);

    // Save to user's Downloads folder - visible and accessible when rollback fails
    let downloads_dir = dirs::download_dir()
        .ok_or_else(|| "Could not determine downloads directory".to_string())?;

    let path = downloads_dir.join(&filename);

    let mut file = File::create(&path).map_err(|e| format!("Failed to create CSV file: {}", e))?;

    // Write CSV header
    writeln!(file, "entity_set,entity_id")
        .map_err(|e| format!("Failed to write CSV header: {}", e))?;

    // Write entities in REVERSE order (deletion order: children before parents)
    for (entity_set, entity_id) in entities.iter().rev() {
        writeln!(file, "{},{}", entity_set, entity_id)
            .map_err(|e| format!("Failed to write entity to CSV: {}", e))?;
    }

    log::info!(
        "Exported {} orphaned entities to: {}",
        entities.len(),
        path.display()
    );

    Ok(path.to_string_lossy().to_string())
}

/// Rollback all created entities in reverse order
/// Returns Ok(()) if rollback succeeded, Err(csv_path) if it failed
pub async fn rollback_created_entities(created_ids: Vec<(String, String)>) -> Result<(), String> {
    if created_ids.is_empty() {
        log::info!("Rollback: No entities to delete");
        return Ok(()); // Nothing to rollback
    }

    log::info!(
        "Starting rollback: deleting {} entities in reverse order",
        created_ids.len()
    );
    log::debug!(
        "Entities to delete: {:?}",
        created_ids
            .iter()
            .map(|(set, id)| format!("{}({})", set, id))
            .collect::<Vec<_>>()
    );

    let client_manager = crate::client_manager();

    // Get client
    let env_name = match client_manager.get_current_environment_name().await {
        Ok(Some(name)) => name,
        _ => {
            log::error!("Rollback failed: Could not get environment name");
            let csv_path = export_orphaned_entities_csv(&created_ids)
                .unwrap_or_else(|e| format!("(CSV export also failed: {})", e));
            return Err(csv_path);
        }
    };

    let client = match client_manager.get_client(&env_name).await {
        Ok(c) => c,
        Err(e) => {
            log::error!("Rollback failed: Could not get client: {}", e);
            let csv_path = export_orphaned_entities_csv(&created_ids)
                .unwrap_or_else(|e| format!("(CSV export also failed: {})", e));
            return Err(csv_path);
        }
    };

    let resilience = ResilienceConfig::migration();
    let mut operations = Operations::new();

    // Delete in REVERSE order (bottom-up to respect dependencies)
    log::debug!("Building delete operations in reverse order");
    for (idx, (entity_set, entity_id)) in created_ids.iter().rev().enumerate() {
        log::debug!(
            "Queuing delete [{}/{}]: {} ({})",
            idx + 1,
            created_ids.len(),
            entity_id,
            entity_set
        );
        operations = operations.add(Operation::Delete {
            entity: entity_set.clone(),
            id: entity_id.clone(),
        });
    }

    // Execute batch delete with automatic chunking
    let entity_count = created_ids.len();
    let all_operations = operations.operations();
    let mut results = Vec::with_capacity(entity_count);

    if entity_count > BATCH_CHUNK_SIZE {
        log::info!(
            "Chunking {} delete operations into batches of {}",
            entity_count,
            BATCH_CHUNK_SIZE
        );

        for (chunk_idx, chunk) in all_operations.chunks(BATCH_CHUNK_SIZE).enumerate() {
            let chunk_ops = Operations::from_operations(chunk.to_vec());

            log::debug!(
                "Executing rollback chunk {}/{} ({} operations)",
                chunk_idx + 1,
                (entity_count + BATCH_CHUNK_SIZE - 1) / BATCH_CHUNK_SIZE,
                chunk.len()
            );

            match chunk_ops.execute(&client, &resilience).await {
                Ok(chunk_results) => {
                    results.extend(chunk_results);
                }
                Err(e) => {
                    log::error!("Rollback chunk {} failed: {}", chunk_idx + 1, e);
                    let csv_path = export_orphaned_entities_csv(&created_ids)
                        .unwrap_or_else(|e| format!("(CSV export also failed: {})", e));
                    return Err(csv_path);
                }
            }
        }
    } else {
        log::info!("Executing batch delete for {} entities", entity_count);
        match operations.execute(&client, &resilience).await {
            Ok(batch_results) => {
                results = batch_results;
            }
            Err(e) => {
                log::error!("Rollback batch operation failed: {}", e);
                let csv_path = export_orphaned_entities_csv(&created_ids)
                    .unwrap_or_else(|e| format!("(CSV export also failed: {})", e));
                return Err(csv_path);
            }
        }
    }

    // Process results
    log::debug!("Received {} deletion results", results.len());

    let mut all_success = true;
    let mut success_count = 0;
    let mut failure_count = 0;

    for (idx, result) in results.iter().enumerate() {
        let (entity_set, entity_id) = &created_ids[created_ids.len() - 1 - idx];

        if result.success {
            log::debug!(
                "Deleted [{}/{}]: {} ({})",
                idx + 1,
                results.len(),
                entity_id,
                entity_set
            );
            success_count += 1;
        } else {
            log::error!(
                "Failed to delete [{}/{}]: {} ({}) - {:?}",
                idx + 1,
                results.len(),
                entity_id,
                entity_set,
                result.error
            );
            all_success = false;
            failure_count += 1;
        }
    }

    if all_success {
        log::info!(
            "Rollback completed successfully - deleted {} entities",
            created_ids.len()
        );
        Ok(())
    } else {
        log::warn!(
            "Rollback partially failed: {} succeeded, {} failed",
            success_count,
            failure_count
        );
        let csv_path = export_orphaned_entities_csv(&created_ids)
            .unwrap_or_else(|e| format!("(CSV export also failed: {})", e));
        log::error!("Orphaned entities exported to: {}", csv_path);
        Err(csv_path)
    }
}
