/// Step 6: Create questions
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

pub async fn step6_create_questions(
    questionnaire: Arc<Questionnaire>,
    id_map: HashMap<String, String>,
    mut created_ids: Vec<(String, String)>,
) -> Result<(HashMap<String, String>, Vec<(String, String)>), CopyError> {
    // Count total questions across all pages and groups
    let questions_count: usize = questionnaire
        .pages
        .iter()
        .flat_map(|p| &p.groups)
        .map(|g| g.questions.len())
        .sum();
    if questions_count == 0 {
        return Ok((id_map, created_ids));
    }

    let mut new_id_map = id_map.clone();

    let (results, entity_info) = execute_creation_step(
        Arc::clone(&questionnaire),
        id_map,
        &mut created_ids,
        CopyPhase::CreatingQuestions,
        6,
        questions_count,
        |q, id_map| {
            let shared_entities = get_shared_entities();
            let mut operations = Operations::new();
            let mut entity_info = Vec::new();

            for page in &q.pages {
                // Get the new page ID for setting nrq_QuestionPage on questions
                let new_page_id = id_map.get(&page.id).ok_or_else(|| {
                    format!("Page ID {} not found in id_map", page.id)
                })?;

                for group in &page.groups {
                    for question in &group.questions {
                        let mut data = build_payload(
                            &question.raw,
                            field_specs::QUESTION_FIELDS,
                            &id_map,
                            &shared_entities,
                        )
                        .map_err(|e| format!("Failed to build question payload: {}", e))?;

                        // Always set the question's page from iteration context
                        // This fills in missing values or corrects existing ones
                        // (a group can appear on multiple pages, so we use the current page context)
                        data["nrq_QuestionPage@odata.bind"] =
                            json!(format!("/nrq_questionnairepages({})", new_page_id));

                        operations = operations.create(entity_sets::QUESTIONS, data);
                        entity_info.push(EntityInfo {
                            old_id: Some(question.id.clone()),
                            entity_set: entity_sets::QUESTIONS.to_string(),
                        });
                    }
                }
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
        CopyPhase::CreatingQuestions,
        6,
    )?;

    Ok((new_id_map, created_ids))
}
