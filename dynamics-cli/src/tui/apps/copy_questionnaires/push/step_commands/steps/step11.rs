/// Step 11: Restore condition status (set statuscode to original value)
use super::super::super::super::copy::domain::Questionnaire;
use super::super::super::models::{CopyError, CopyPhase};
use super::super::entity_sets;
use super::super::error::build_error;
use super::super::execution::BATCH_CHUNK_SIZE;
use crate::api::ResilienceConfig;
use crate::api::operations::Operations;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

pub async fn step11_publish_conditions(
    questionnaire: Arc<Questionnaire>,
    id_map: HashMap<String, String>,
    created_ids: Vec<(String, String)>,
) -> Result<(HashMap<String, String>, Vec<(String, String)>), CopyError> {
    if questionnaire.conditions.is_empty() {
        return Ok((id_map, created_ids));
    }

    let conditions_count = questionnaire.conditions.len();
    log::info!(
        "Step 11/11: Starting Restoring Condition Status (updating {} conditions)",
        conditions_count
    );

    let mut operations = Operations::new();

    // Build update operations for each condition
    for condition in &questionnaire.conditions {
        // Get the new condition ID from the id_map
        let new_condition_id = id_map.get(&condition.id).ok_or_else(|| {
            build_error(
                format!("Condition ID {} not found in map", condition.id),
                CopyPhase::PublishingConditions,
                11,
                &created_ids,
            )
        })?;

        // Get the original statuscode from the source condition
        let original_statuscode = condition
            .raw
            .get("statuscode")
            .and_then(|v| v.as_i64())
            .unwrap_or(170590001); // Default to Published if not found

        // Update the condition to restore original statuscode
        let update_data = json!({
            "statuscode": original_statuscode
        });

        operations = operations.update(
            entity_sets::CONDITIONS,
            new_condition_id.clone(),
            update_data,
        );
    }

    let client_manager = crate::client_manager();
    let env_name = client_manager
        .get_current_environment_name()
        .await
        .map_err(|e| {
            build_error(
                e.to_string(),
                CopyPhase::PublishingConditions,
                11,
                &created_ids,
            )
        })?
        .ok_or_else(|| {
            build_error(
                "No environment selected".to_string(),
                CopyPhase::PublishingConditions,
                11,
                &created_ids,
            )
        })?;

    let client = client_manager.get_client(&env_name).await.map_err(|e| {
        build_error(
            e.to_string(),
            CopyPhase::PublishingConditions,
            11,
            &created_ids,
        )
    })?;

    let resilience = ResilienceConfig::migration();

    // Execute with automatic chunking (same pattern as classifications)
    let all_operations = operations.operations();
    let mut results = Vec::with_capacity(conditions_count);

    if conditions_count > BATCH_CHUNK_SIZE {
        log::info!(
            "Chunking {} condition updates into batches of {}",
            conditions_count,
            BATCH_CHUNK_SIZE
        );

        for (chunk_idx, chunk) in all_operations.chunks(BATCH_CHUNK_SIZE).enumerate() {
            let chunk_ops = Operations::from_operations(chunk.to_vec());

            log::debug!(
                "Executing condition publish chunk {}/{} ({} operations)",
                chunk_idx + 1,
                (conditions_count + BATCH_CHUNK_SIZE - 1) / BATCH_CHUNK_SIZE,
                chunk.len()
            );

            let chunk_results = chunk_ops.execute(&client, &resilience).await.map_err(|e| {
                build_error(
                    format!(
                        "Failed to execute condition publish chunk {}: {}",
                        chunk_idx + 1,
                        e
                    ),
                    CopyPhase::PublishingConditions,
                    11,
                    &created_ids,
                )
            })?;

            results.extend(chunk_results);
        }
    } else {
        results = operations
            .execute(&client, &resilience)
            .await
            .map_err(|e| {
                build_error(
                    e.to_string(),
                    CopyPhase::PublishingConditions,
                    11,
                    &created_ids,
                )
            })?;
    }

    // Validate result count matches expected count
    if results.len() != conditions_count {
        return Err(build_error(
            format!(
                "Result count mismatch: expected {} condition updates, got {} results",
                conditions_count,
                results.len()
            ),
            CopyPhase::PublishingConditions,
            11,
            &created_ids,
        ));
    }

    // Track errors
    let mut first_error = None;
    for result in &results {
        if !result.success && first_error.is_none() {
            first_error = Some(
                result
                    .error
                    .clone()
                    .unwrap_or_else(|| "Unknown error".to_string()),
            );
        }
    }

    // Note: Update operations don't create new entities, so we don't add to created_ids

    // If any errors occurred, return error
    if let Some(error_msg) = first_error {
        return Err(build_error(
            error_msg,
            CopyPhase::PublishingConditions,
            11,
            &created_ids,
        ));
    }

    log::info!(
        "Step 11/11: Completed Restoring Condition Status successfully ({} conditions updated)",
        conditions_count
    );

    Ok((id_map, created_ids))
}
