/// Step 2: Create questionnaire pages
use super::super::super::super::copy::domain::Questionnaire;
use super::super::super::models::{CopyError, CopyPhase};
use super::super::entity_sets;
use super::super::execution::{EntityInfo, execute_creation_step, process_creation_results};
use super::super::field_specs;
use super::super::helpers::{build_payload, get_shared_entities};
use crate::api::operations::Operations;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

pub async fn step2_create_pages(
    questionnaire: Arc<Questionnaire>,
    id_map: HashMap<String, String>,
    mut created_ids: Vec<(String, String)>,
) -> Result<(HashMap<String, String>, Vec<(String, String)>), CopyError> {
    if questionnaire.pages.is_empty() {
        return Ok((id_map, created_ids));
    }

    let expected_count = questionnaire.pages.len();
    let mut new_id_map = id_map.clone();

    // Execute creation using generic helper
    let (results, entity_info) = execute_creation_step(
        Arc::clone(&questionnaire),
        id_map,
        &mut created_ids,
        CopyPhase::CreatingPages,
        2,
        expected_count,
        |q, id_map| {
            let new_questionnaire_id = id_map
                .get(&q.id)
                .ok_or_else(|| "Questionnaire ID not found in map".to_string())?;

            let shared_entities = get_shared_entities();
            let mut operations = Operations::new();
            let mut entity_info = Vec::new();

            for page in &q.pages {
                let data = build_payload(
                    &page.raw,
                    field_specs::PAGE_FIELDS,
                    &id_map,
                    &shared_entities,
                )
                .map_err(|e| format!("Failed to build page payload: {}", e))?;

                // Note: Pages are standalone entities - relationship to questionnaire
                // is established via page_lines junction table (step 3)

                operations = operations.create(entity_sets::PAGES, data);
                entity_info.push(EntityInfo {
                    old_id: Some(page.id.clone()),
                    entity_set: entity_sets::PAGES.to_string(),
                });
            }

            Ok((operations, entity_info))
        },
    )
    .await?;

    // Process results using generic helper
    process_creation_results(
        &results,
        entity_info,
        &mut new_id_map,
        &mut created_ids,
        CopyPhase::CreatingPages,
        2,
    )?;

    Ok((new_id_map, created_ids))
}
