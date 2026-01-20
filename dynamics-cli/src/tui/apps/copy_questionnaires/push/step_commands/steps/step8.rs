/// Step 8: Create conditions
use super::super::super::super::copy::domain::Questionnaire;
use super::super::super::models::{CopyError, CopyPhase};
use super::super::entity_sets;
use super::super::execution::{EntityInfo, execute_creation_step, process_creation_results};
use super::super::field_specs;
use super::super::helpers::{build_payload, get_shared_entities, remap_condition_json};
use crate::api::operations::Operations;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

pub async fn step8_create_conditions(
    questionnaire: Arc<Questionnaire>,
    id_map: HashMap<String, String>,
    mut created_ids: Vec<(String, String)>,
) -> Result<(HashMap<String, String>, Vec<(String, String)>), CopyError> {
    if questionnaire.conditions.is_empty() {
        return Ok((id_map, created_ids));
    }

    let expected_count = questionnaire.conditions.len();
    let mut new_id_map = id_map.clone();

    let (results, entity_info) = execute_creation_step(
        Arc::clone(&questionnaire),
        id_map,
        &mut created_ids,
        CopyPhase::CreatingConditions,
        8,
        expected_count,
        |q, id_map| {
            let shared_entities = get_shared_entities();
            let mut operations = Operations::new();
            let mut entity_info = Vec::new();

            for condition in &q.conditions {
                let mut data = build_payload(
                    &condition.raw,
                    field_specs::CONDITION_FIELDS,
                    &id_map,
                    &shared_entities,
                )
                .map_err(|e| format!("Failed to build condition payload: {}", e))?;

                // CRITICAL: Remap condition JSON with embedded question IDs
                if let Some(condition_json_str) = condition
                    .raw
                    .get("nrq_conditionjson")
                    .and_then(|v| v.as_str())
                {
                    let remapped_json = remap_condition_json(condition_json_str, &id_map)
                        .map_err(|e| format!("Failed to remap condition JSON: {}", e))?;
                    data["nrq_conditionjson"] = json!(remapped_json);
                }

                // CRITICAL: Create condition in draft status (1) not published (170590001)
                // Published status requires actions to exist first, which are created in Step 9
                // Step 11 will restore the original statuscode after actions are created
                data["statuscode"] = json!(1);

                operations = operations.create(entity_sets::CONDITIONS, data);
                entity_info.push(EntityInfo {
                    old_id: Some(condition.id.clone()),
                    entity_set: entity_sets::CONDITIONS.to_string(),
                });
            }

            Ok((operations, entity_info))
        },
    )
    .await?;

    process_creation_results(
        &results,
        entity_info,
        &mut new_id_map,
        &mut created_ids,
        CopyPhase::CreatingConditions,
        8,
    )?;

    Ok((new_id_map, created_ids))
}
