/// Step 5: Create group lines
use super::super::super::super::copy::domain::Questionnaire;
use super::super::super::models::{CopyError, CopyPhase};
use super::super::entity_sets;
use super::super::execution::{EntityInfo, execute_creation_step, process_creation_results};
use super::super::field_specs;
use super::super::helpers::{build_payload, get_shared_entities};
use crate::api::operations::Operations;
use std::collections::HashMap;
use std::sync::Arc;

pub async fn step5_create_group_lines(
    questionnaire: Arc<Questionnaire>,
    id_map: HashMap<String, String>,
    mut created_ids: Vec<(String, String)>,
) -> Result<(HashMap<String, String>, Vec<(String, String)>), CopyError> {
    if questionnaire.group_lines.is_empty() {
        return Ok((id_map, created_ids));
    }

    let expected_count = questionnaire.group_lines.len();
    let mut new_id_map = id_map.clone();

    let (results, entity_info) = execute_creation_step(
        Arc::clone(&questionnaire),
        id_map,
        &mut created_ids,
        CopyPhase::CreatingGroupLines,
        5,
        expected_count,
        |q, id_map| {
            let shared_entities = get_shared_entities();
            let mut operations = Operations::new();
            let mut entity_info = Vec::new();

            for group_line in &q.group_lines {
                let data = build_payload(
                    group_line,
                    field_specs::GROUP_LINE_FIELDS,
                    &id_map,
                    &shared_entities,
                )
                .map_err(|e| format!("Failed to build group line payload: {}", e))?;

                operations = operations.create(entity_sets::GROUP_LINES, data);
                entity_info.push(EntityInfo {
                    old_id: None, // No ID mapping needed for group lines
                    entity_set: entity_sets::GROUP_LINES.to_string(),
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
        CopyPhase::CreatingGroupLines,
        5,
    )?;

    Ok((new_id_map, created_ids))
}
