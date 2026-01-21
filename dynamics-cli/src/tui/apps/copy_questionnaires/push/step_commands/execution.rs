use super::super::super::copy::domain::Questionnaire;
/// Generic execution logic for creation steps with automatic batching
use super::super::models::{CopyError, CopyPhase};
use super::error::build_error;
use super::helpers::extract_entity_id;
use crate::api::ResilienceConfig;
use crate::api::operations::Operations;
use std::collections::HashMap;
use std::sync::Arc;

/// Metadata about an entity being created
pub struct EntityInfo {
    pub old_id: Option<String>, // Original ID (for ID mapping), None if no mapping needed
    pub entity_set: String,     // Entity set name for tracking
}

/// Batch size for chunking large operations (Dynamics 365 limit is 1000, we use 75 for safety)
pub const BATCH_CHUNK_SIZE: usize = 75;

/// Generic helper for executing creation steps with common scaffolding
/// This eliminates ~700 lines of duplication across steps 2-10
/// Automatically chunks large batches to avoid Dynamics 365 limits
pub async fn execute_creation_step<F>(
    questionnaire: Arc<Questionnaire>,
    id_map: HashMap<String, String>,
    created_ids: &mut Vec<(String, String)>,
    phase: CopyPhase,
    step: usize,
    expected_count: usize,
    build_operations: F,
) -> Result<
    (
        Vec<crate::api::operations::OperationResult>,
        Vec<EntityInfo>,
    ),
    CopyError,
>
where
    F: FnOnce(
        Arc<Questionnaire>,
        HashMap<String, String>,
    ) -> Result<(Operations, Vec<EntityInfo>), String>,
{
    log::info!(
        "Step {}/10: Starting {} (expecting {} entities)",
        step,
        phase.name(),
        expected_count
    );

    // 1. Get client (common scaffolding)
    let client_manager = crate::client_manager();
    let env_name = client_manager
        .get_current_environment_name()
        .await
        .map_err(|e| build_error(e.to_string(), phase.clone(), step, created_ids))?
        .ok_or_else(|| {
            build_error(
                "No environment selected".to_string(),
                phase.clone(),
                step,
                created_ids,
            )
        })?;

    let client = client_manager
        .get_client(&env_name)
        .await
        .map_err(|e| build_error(e.to_string(), phase.clone(), step, created_ids))?;

    let resilience = ResilienceConfig::migration();

    // 2. Build operations (unique per step)
    log::debug!("Building operations for {} entities", expected_count);
    let (operations, entity_info) = build_operations(questionnaire, id_map)
        .map_err(|e| build_error(e, phase.clone(), step, created_ids))?;

    // 3. Execute operations with automatic chunking
    let all_operations = operations.operations();
    let total_ops = all_operations.len();

    if total_ops == 0 {
        log::info!(
            "Step {}/10: No entities to create for {}",
            step,
            phase.name()
        );
        return Ok((vec![], entity_info));
    }

    let mut all_results = Vec::with_capacity(total_ops);

    // Chunk operations if exceeds batch size
    if total_ops > BATCH_CHUNK_SIZE {
        log::info!(
            "Chunking {} operations into batches of {} for {}",
            total_ops,
            BATCH_CHUNK_SIZE,
            phase.name()
        );

        // Process in chunks
        for (chunk_idx, chunk) in all_operations.chunks(BATCH_CHUNK_SIZE).enumerate() {
            let chunk_ops = Operations::from_operations(chunk.to_vec());

            log::debug!(
                "Executing chunk {}/{} ({} operations) for {}",
                chunk_idx + 1,
                (total_ops + BATCH_CHUNK_SIZE - 1) / BATCH_CHUNK_SIZE,
                chunk.len(),
                phase.name()
            );

            let chunk_results = chunk_ops.execute(&client, &resilience).await.map_err(|e| {
                build_error(
                    format!("Failed to execute chunk {}: {}", chunk_idx + 1, e),
                    phase.clone(),
                    step,
                    created_ids,
                )
            })?;

            log::info!(
                "Completed chunk {}/{}: created {} entities",
                chunk_idx + 1,
                (total_ops + BATCH_CHUNK_SIZE - 1) / BATCH_CHUNK_SIZE,
                chunk_results.len()
            );

            all_results.extend(chunk_results);
        }
    } else {
        // Single batch execution
        log::debug!("Executing single batch with {} operations", total_ops);
        all_results = operations
            .execute(&client, &resilience)
            .await
            .map_err(|e| build_error(e.to_string(), phase.clone(), step, created_ids))?;
        log::info!("Created {} entities in single batch", all_results.len());
    }

    // 4. Validate result count (common scaffolding)
    if all_results.len() != expected_count {
        log::error!(
            "Result count mismatch for {}: expected {}, got {}",
            phase.name(),
            expected_count,
            all_results.len()
        );
        return Err(build_error(
            format!(
                "Result count mismatch: expected {} entities, got {} results",
                expected_count,
                all_results.len()
            ),
            phase,
            step,
            created_ids,
        ));
    }

    log::info!(
        "Step {}/10: Completed {} successfully ({} entities created)",
        step,
        phase.name(),
        expected_count
    );
    Ok((all_results, entity_info))
}

/// Process results from creation operations - extracts IDs and handles errors
pub fn process_creation_results(
    results: &[crate::api::operations::OperationResult],
    entity_info: Vec<EntityInfo>,
    id_map: &mut HashMap<String, String>,
    created_ids: &mut Vec<(String, String)>,
    phase: CopyPhase,
    step: usize,
) -> Result<(), CopyError> {
    log::debug!(
        "Processing {} creation results for {}",
        results.len(),
        phase.name()
    );

    let mut first_error = None;
    let mut success_count = 0;
    let mut mapping_count = 0;

    // Process ALL results, tracking successes even if some fail
    for (idx, (info, result)) in entity_info.iter().zip(results.iter()).enumerate() {
        if result.success {
            match extract_entity_id(result) {
                Ok(new_id) => {
                    // Update ID mapping if this entity has an old ID
                    if let Some(ref old_id) = info.old_id {
                        log::debug!(
                            "ID remapping [{}/{}]: {} → {} ({})",
                            idx + 1,
                            results.len(),
                            old_id,
                            new_id,
                            info.entity_set
                        );
                        id_map.insert(old_id.clone(), new_id.clone());
                        mapping_count += 1;
                    } else {
                        log::debug!(
                            "Created [{}/{}]: {} ({})",
                            idx + 1,
                            results.len(),
                            new_id,
                            info.entity_set
                        );
                    }
                    created_ids.push((info.entity_set.clone(), new_id));
                    success_count += 1;
                }
                Err(e) => {
                    log::error!(
                        "Failed to extract entity ID for {} [{}/{}]: {}",
                        info.entity_set,
                        idx + 1,
                        results.len(),
                        e
                    );
                    if first_error.is_none() {
                        first_error = Some(format!("Failed to extract entity ID: {}", e));
                    }
                }
            }
        } else {
            let error_msg = result
                .error
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string());
            log::error!(
                "Creation failed for {} [{}/{}]: {}",
                info.entity_set,
                idx + 1,
                results.len(),
                error_msg
            );
            if first_error.is_none() {
                first_error = Some(error_msg);
            }
        }
    }

    log::info!(
        "Processed {} results: {} successful, {} ID mappings created",
        results.len(),
        success_count,
        mapping_count
    );

    // If any errors occurred, return error with ALL successful IDs tracked
    if let Some(error_msg) = first_error {
        log::error!("Step {} failed with error: {}", step, error_msg);
        return Err(build_error(error_msg, phase, step, created_ids));
    }

    Ok(())
}
