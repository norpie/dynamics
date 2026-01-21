/// Step 1: Create the questionnaire entity
use super::super::super::super::copy::domain::Questionnaire;
use super::super::super::models::{CopyError, CopyPhase};
use super::super::entity_sets;
use super::super::error::build_error;
use super::super::field_specs;
use super::super::helpers::{build_payload, extract_entity_id, get_shared_entities};
use crate::api::ResilienceConfig;
use crate::api::operations::Operations;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

pub async fn step1_create_questionnaire(
    questionnaire: Arc<Questionnaire>,
    copy_name: String,
    copy_code: String,
) -> Result<(String, Vec<(String, String)>), CopyError> {
    log::info!("Step 1/11: Starting Creating Questionnaire (expecting 1 entity)");
    log::debug!(
        "Copy name: '{}', copy code: '{}', source ID: {}",
        copy_name,
        copy_code,
        questionnaire.id
    );

    let client_manager = crate::client_manager();

    let env_name = client_manager
        .get_current_environment_name()
        .await
        .map_err(|e| build_error(e.to_string(), CopyPhase::CreatingQuestionnaire, 1, &[]))?
        .ok_or_else(|| {
            build_error(
                "No environment selected".to_string(),
                CopyPhase::CreatingQuestionnaire,
                1,
                &[],
            )
        })?;

    let client = client_manager
        .get_client(&env_name)
        .await
        .map_err(|e| build_error(e.to_string(), CopyPhase::CreatingQuestionnaire, 1, &[]))?;

    let resilience = match ResilienceConfig::load_from_options().await {
        Ok(config) => config,
        Err(e) => {
            log::warn!(
                "Failed to load resilience config from options: {}, using defaults",
                e
            );
            ResilienceConfig::default()
        }
    };

    log::debug!("Preparing questionnaire data");
    let shared_entities = get_shared_entities();
    let id_map = HashMap::new(); // Step 1 has no remapping yet

    let mut data = build_payload(
        &questionnaire.raw,
        field_specs::QUESTIONNAIRE_FIELDS,
        &id_map,
        &shared_entities,
    )
    .map_err(|e| build_error(e, CopyPhase::CreatingQuestionnaire, 1, &[]))?;

    // Override name and code with user-provided values
    data["nrq_name"] = json!(copy_name);
    data["nrq_copypostfix"] = json!(copy_code);

    log::debug!("Executing questionnaire creation");
    let operations = Operations::new().create(entity_sets::QUESTIONNAIRES, data);
    let results = operations
        .execute(&client, &resilience)
        .await
        .map_err(|e| build_error(e.to_string(), CopyPhase::CreatingQuestionnaire, 1, &[]))?;

    if !results[0].success {
        let error_msg = results[0]
            .error
            .clone()
            .unwrap_or_else(|| "Unknown error".to_string());
        log::error!("Questionnaire creation failed: {}", error_msg);
        return Err(build_error(
            error_msg,
            CopyPhase::CreatingQuestionnaire,
            1,
            &[],
        ));
    }

    let new_id = extract_entity_id(&results[0]).map_err(|e| {
        build_error(
            format!("Failed to extract questionnaire ID: {}", e),
            CopyPhase::CreatingQuestionnaire,
            1,
            &[],
        )
    })?;

    log::info!("Created questionnaire: {} → {}", questionnaire.id, new_id);
    log::debug!(
        "ID mapping: {} → {} ({})",
        questionnaire.id,
        new_id,
        entity_sets::QUESTIONNAIRES
    );

    let created_ids = vec![(entity_sets::QUESTIONNAIRES.to_string(), new_id.clone())];

    log::info!("Step 1/11: Completed Creating Questionnaire successfully (1 entity created)");
    Ok((new_id, created_ids))
}
