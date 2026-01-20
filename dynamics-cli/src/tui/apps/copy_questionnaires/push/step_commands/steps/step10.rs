/// Step 10: Create classification associations
use super::super::super::super::copy::domain::Questionnaire;
use super::super::super::models::{CopyError, CopyPhase};
use super::super::entity_sets;
use super::super::error::build_error;
use super::super::execution::BATCH_CHUNK_SIZE;
use crate::api::operations::{Operation, Operations};
use crate::api::{ResilienceConfig, constants};
use std::collections::HashMap;
use std::sync::Arc;

pub async fn step10_create_classifications(
    questionnaire: Arc<Questionnaire>,
    id_map: HashMap<String, String>,
    mut created_ids: Vec<(String, String)>,
) -> Result<(HashMap<String, String>, Vec<(String, String)>, usize), CopyError> {
    let new_questionnaire_id = id_map.get(&questionnaire.id).ok_or_else(|| {
        build_error(
            "Questionnaire ID not found in map".to_string(),
            CopyPhase::CreatingClassifications,
            10,
            &created_ids,
        )
    })?;

    let mut operations = Operations::new();
    let mut classifications_count = 0;

    // Category associations
    for category_ref in &questionnaire.classifications.categories {
        operations = operations.add(Operation::AssociateRef {
            entity: entity_sets::QUESTIONNAIRES.to_string(),
            entity_ref: new_questionnaire_id.clone(),
            navigation_property: "nrq_questionnaire_nrq_Category_nrq_Category".to_string(),
            target_ref: format!(
                "{}/nrq_categories({})",
                constants::api_path(),
                category_ref.id
            ),
        });
        classifications_count += 1;
    }

    // Domain associations
    for domain_ref in &questionnaire.classifications.domains {
        operations = operations.add(Operation::AssociateRef {
            entity: entity_sets::QUESTIONNAIRES.to_string(),
            entity_ref: new_questionnaire_id.clone(),
            navigation_property: "nrq_questionnaire_nrq_Domain_nrq_Domain".to_string(),
            target_ref: format!("{}/nrq_domains({})", constants::api_path(), domain_ref.id),
        });
        classifications_count += 1;
    }

    // Fund associations
    for fund_ref in &questionnaire.classifications.funds {
        operations = operations.add(Operation::AssociateRef {
            entity: entity_sets::QUESTIONNAIRES.to_string(),
            entity_ref: new_questionnaire_id.clone(),
            navigation_property: "nrq_questionnaire_nrq_Fund_nrq_Fund".to_string(),
            target_ref: format!("{}/nrq_funds({})", constants::api_path(), fund_ref.id),
        });
        classifications_count += 1;
    }

    // Support associations
    for support_ref in &questionnaire.classifications.supports {
        operations = operations.add(Operation::AssociateRef {
            entity: entity_sets::QUESTIONNAIRES.to_string(),
            entity_ref: new_questionnaire_id.clone(),
            navigation_property: "nrq_questionnaire_nrq_Support_nrq_Support".to_string(),
            target_ref: format!("{}/nrq_supports({})", constants::api_path(), support_ref.id),
        });
        classifications_count += 1;
    }

    // Type associations
    for type_ref in &questionnaire.classifications.types {
        operations = operations.add(Operation::AssociateRef {
            entity: entity_sets::QUESTIONNAIRES.to_string(),
            entity_ref: new_questionnaire_id.clone(),
            navigation_property: "nrq_questionnaire_nrq_Type_nrq_Type".to_string(),
            target_ref: format!("{}/nrq_types({})", constants::api_path(), type_ref.id),
        });
        classifications_count += 1;
    }

    // Subcategory associations
    for subcategory_ref in &questionnaire.classifications.subcategories {
        operations = operations.add(Operation::AssociateRef {
            entity: entity_sets::QUESTIONNAIRES.to_string(),
            entity_ref: new_questionnaire_id.clone(),
            navigation_property: "nrq_questionnaire_nrq_Subcategory_nrq_Subcategory".to_string(),
            target_ref: format!(
                "{}/nrq_subcategories({})",
                constants::api_path(),
                subcategory_ref.id
            ),
        });
        classifications_count += 1;
    }

    // Flemish share associations
    for flemish_share_ref in &questionnaire.classifications.flemish_shares {
        operations = operations.add(Operation::AssociateRef {
            entity: entity_sets::QUESTIONNAIRES.to_string(),
            entity_ref: new_questionnaire_id.clone(),
            navigation_property: "nrq_questionnaire_nrq_FlemishShare_nrq_FlemishShare".to_string(),
            target_ref: format!(
                "{}/nrq_flemishshares({})",
                constants::api_path(),
                flemish_share_ref.id
            ),
        });
        classifications_count += 1;
    }

    if classifications_count == 0 {
        return Ok((id_map, created_ids, 0));
    }

    let client_manager = crate::client_manager();
    let env_name = client_manager
        .get_current_environment_name()
        .await
        .map_err(|e| {
            build_error(
                e.to_string(),
                CopyPhase::CreatingClassifications,
                10,
                &created_ids,
            )
        })?
        .ok_or_else(|| {
            build_error(
                "No environment selected".to_string(),
                CopyPhase::CreatingClassifications,
                10,
                &created_ids,
            )
        })?;

    let client = client_manager.get_client(&env_name).await.map_err(|e| {
        build_error(
            e.to_string(),
            CopyPhase::CreatingClassifications,
            10,
            &created_ids,
        )
    })?;

    let resilience = ResilienceConfig::default();

    // Execute with automatic chunking (same as other steps)
    let all_operations = operations.operations();
    let mut results = Vec::with_capacity(classifications_count);

    if classifications_count > BATCH_CHUNK_SIZE {
        log::info!(
            "Chunking {} classification associations into batches of {}",
            classifications_count,
            BATCH_CHUNK_SIZE
        );

        for (chunk_idx, chunk) in all_operations.chunks(BATCH_CHUNK_SIZE).enumerate() {
            let chunk_ops = Operations::from_operations(chunk.to_vec());

            log::debug!(
                "Executing classification chunk {}/{} ({} operations)",
                chunk_idx + 1,
                (classifications_count + BATCH_CHUNK_SIZE - 1) / BATCH_CHUNK_SIZE,
                chunk.len()
            );

            let chunk_results = chunk_ops.execute(&client, &resilience).await.map_err(|e| {
                build_error(
                    format!(
                        "Failed to execute classification chunk {}: {}",
                        chunk_idx + 1,
                        e
                    ),
                    CopyPhase::CreatingClassifications,
                    10,
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
                    CopyPhase::CreatingClassifications,
                    10,
                    &created_ids,
                )
            })?;
    }

    // Validate result count matches expected count
    if results.len() != classifications_count {
        return Err(build_error(
            format!(
                "Result count mismatch: expected {} classification associations, got {} results",
                classifications_count,
                results.len()
            ),
            CopyPhase::CreatingClassifications,
            10,
            &created_ids,
        ));
    }

    // Track errors but don't stop - we want to know if ANY associations failed
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

    // Note: AssociateRef operations don't create new entities, they just link existing ones
    // So we don't add anything to created_ids here

    // If any errors occurred, return error (rollback will delete questionnaire which removes associations)
    if let Some(error_msg) = first_error {
        return Err(build_error(
            error_msg,
            CopyPhase::CreatingClassifications,
            10,
            &created_ids,
        ));
    }

    Ok((id_map, created_ids, classifications_count))
}
