use super::super::entity_sets;
use super::super::field_specs;
/// Step 3: Create page lines

use super::super::super::super::copy::domain::Questionnaire;
use super::super::super::models::{CopyError, CopyPhase};
use super::super::execution::{execute_creation_step, process_creation_results, EntityInfo};
use super::super::helpers::{get_shared_entities, build_payload};
use crate::api::operations::Operations;
use std::collections::HashMap;
use std::sync::Arc;

pub async fn step3_create_page_lines(
    questionnaire: Arc<Questionnaire>,
    id_map: HashMap<String, String>,
    mut created_ids: Vec<(String, String)>,
) -> Result<(HashMap<String, String>, Vec<(String, String)>), CopyError> {
    if questionnaire.page_lines.is_empty() {
        return Ok((id_map, created_ids));
    }

    let expected_count = questionnaire.page_lines.len();
    let mut new_id_map = id_map.clone();

    let (results, entity_info) = execute_creation_step(
        Arc::clone(&questionnaire),
        id_map,
        &mut created_ids,
        CopyPhase::CreatingPageLines,
        3,
        expected_count,
        |q, id_map| {
            let shared_entities = get_shared_entities();
            let mut operations = Operations::new();
            let mut entity_info = Vec::new();

            for page_line in &q.page_lines {
                let data = build_payload(page_line, field_specs::PAGE_LINE_FIELDS, &id_map, &shared_entities)
                    .map_err(|e| format!("Failed to build page line payload: {}", e))?;

                operations = operations.create(entity_sets::PAGE_LINES, data);
                entity_info.push(EntityInfo {
                    old_id: None,  // No ID mapping needed for page lines
                    entity_set: entity_sets::PAGE_LINES.to_string(),
                });
            }

            Ok((operations, entity_info))
        }
    ).await?;

    process_creation_results(&results, entity_info, &mut new_id_map, &mut created_ids, CopyPhase::CreatingPageLines, 3)?;

    Ok((new_id_map, created_ids))
}
