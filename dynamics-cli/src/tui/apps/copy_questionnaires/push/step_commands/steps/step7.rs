/// Step 7: Create template lines
use super::super::super::super::copy::domain::Questionnaire;
use super::super::super::models::{CopyError, CopyPhase};
use super::super::entity_sets;
use super::super::execution::{EntityInfo, execute_creation_step, process_creation_results};
use super::super::field_specs;
use super::super::helpers::{build_payload, get_shared_entities};
use crate::api::operations::Operations;
use std::collections::HashMap;
use std::sync::Arc;

pub async fn step7_create_template_lines(
    questionnaire: Arc<Questionnaire>,
    id_map: HashMap<String, String>,
    mut created_ids: Vec<(String, String)>,
) -> Result<(HashMap<String, String>, Vec<(String, String)>), CopyError> {
    if questionnaire.template_lines.is_empty() {
        return Ok((id_map, created_ids));
    }

    let expected_count = questionnaire.template_lines.len();
    let mut new_id_map = id_map.clone();

    let (results, entity_info) = execute_creation_step(
        Arc::clone(&questionnaire),
        id_map,
        &mut created_ids,
        CopyPhase::CreatingTemplateLines,
        7,
        expected_count,
        |q, id_map| {
            let shared_entities = get_shared_entities();
            let mut operations = Operations::new();
            let mut entity_info = Vec::new();

            for template_line in &q.template_lines {
                let data = build_payload(
                    &template_line.raw,
                    field_specs::TEMPLATE_LINE_FIELDS,
                    &id_map,
                    &shared_entities,
                )
                .map_err(|e| format!("Failed to build template line payload: {}", e))?;

                operations = operations.create(entity_sets::TEMPLATE_LINES, data);
                entity_info.push(EntityInfo {
                    old_id: None, // No ID mapping needed for template lines
                    entity_set: entity_sets::TEMPLATE_LINES.to_string(),
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
        CopyPhase::CreatingTemplateLines,
        7,
    )?;

    Ok((new_id_map, created_ids))
}
