use super::domain::{
    Classifications, Condition, ConditionAction, Group, Page, Question, Questionnaire, Reference,
    TemplateLine,
};
use super::models::QuestionnaireSnapshot;
use crate::api::query::{Filter, FilterValue, Query};
use serde_json::Value;
use std::collections::HashMap;

/// Load complete questionnaire snapshot with all related entities
/// This performs multiple sequential queries to fetch all data
pub async fn load_full_snapshot(questionnaire_id: &str) -> Result<QuestionnaireSnapshot, String> {
    log::info!(
        "Loading full snapshot for questionnaire: {}",
        questionnaire_id
    );

    // Get the client
    let manager = crate::client_manager();
    let env_name = manager
        .get_current_environment_name()
        .await
        .ok()
        .flatten()
        .ok_or_else(|| "No environment selected".to_string())?;

    let client = manager
        .get_client(&env_name)
        .await
        .map_err(|e| e.to_string())?;

    // Phase 1 - Snapshot (Read Everything) - Load in dependency order

    // 1. Load questionnaire entity by ID
    log::debug!("Loading questionnaire: {}", questionnaire_id);
    let questionnaire = load_questionnaire(&client, questionnaire_id).await?;

    // 2. Load page lines (junction records for page ordering)
    log::debug!("Loading page lines");
    let page_lines = load_page_lines(&client, questionnaire_id).await?;
    let page_ids = extract_ids(&page_lines, "_nrq_questionnairepageid_value");
    log::debug!("Extracted page IDs: {:?}", page_ids);

    // 3. Load pages
    log::debug!("Loading pages ({})", page_ids.len());
    let pages = if !page_ids.is_empty() {
        load_pages(&client, &page_ids).await?
    } else {
        vec![]
    };

    // 4. Load group lines (junction records for group ordering)
    log::debug!("Loading group lines");
    let group_lines = if !page_ids.is_empty() {
        load_group_lines(&client, &page_ids).await?
    } else {
        vec![]
    };
    let group_ids = extract_ids(&group_lines, "_nrq_questiongroupid_value");

    // 5. Load groups
    log::debug!("Loading groups ({})", group_ids.len());
    let groups = if !group_ids.is_empty() {
        load_groups(&client, &group_ids).await?
    } else {
        vec![]
    };

    // 6. Load questions
    log::debug!("Loading questions");
    let questions = load_questions(&client, questionnaire_id).await?;
    let question_ids = extract_ids(&questions, "nrq_questionid");

    // 7. Load template lines
    log::debug!("Loading template lines");
    let template_lines = if !group_ids.is_empty() {
        load_template_lines(&client, &group_ids).await?
    } else {
        vec![]
    };

    // 8. Load conditions
    log::debug!("Loading conditions");
    let conditions = if !question_ids.is_empty() {
        load_conditions(&client, &question_ids).await?
    } else {
        vec![]
    };
    let condition_ids = extract_ids(&conditions, "nrq_questionconditionid");

    // 9. Load condition actions
    log::debug!("Loading condition actions");
    let condition_actions = if !condition_ids.is_empty() {
        load_condition_actions(&client, &condition_ids).await?
    } else {
        vec![]
    };

    // 10. Load N:N relationships (7 junction tables) and fetch the actual entities
    log::debug!("Loading N:N relationships");
    let category_ids = load_n_n_relationship(
        &client,
        "nrq_questionnaire_nrq_category",
        questionnaire_id,
        "nrq_categoryid",
    )
    .await?;
    let domain_ids = load_n_n_relationship(
        &client,
        "nrq_questionnaire_nrq_domain",
        questionnaire_id,
        "nrq_domainid",
    )
    .await?;
    let fund_ids = load_n_n_relationship(
        &client,
        "nrq_questionnaire_nrq_fund",
        questionnaire_id,
        "nrq_fundid",
    )
    .await?;
    let support_ids = load_n_n_relationship(
        &client,
        "nrq_questionnaire_nrq_support",
        questionnaire_id,
        "nrq_supportid",
    )
    .await?;
    let type_ids = load_n_n_relationship(
        &client,
        "nrq_questionnaire_nrq_type",
        questionnaire_id,
        "nrq_typeid",
    )
    .await?;
    let subcategory_ids = load_n_n_relationship(
        &client,
        "nrq_questionnaire_nrq_subcategory",
        questionnaire_id,
        "nrq_subcategoryid",
    )
    .await?;
    let flemish_share_ids = load_n_n_relationship(
        &client,
        "nrq_questionnaire_nrq_flemishshare",
        questionnaire_id,
        "nrq_flemishshareid",
    )
    .await?;

    // 11. Fetch the actual classification entity records
    log::debug!("Loading classification entities:");
    log::debug!("  Category IDs: {:?}", category_ids);
    log::debug!("  Domain IDs: {:?}", domain_ids);
    log::debug!("  Fund IDs: {:?}", fund_ids);
    log::debug!("  Support IDs: {:?}", support_ids);
    log::debug!("  Type IDs: {:?}", type_ids);
    log::debug!("  Subcategory IDs: {:?}", subcategory_ids);
    log::debug!("  Flemish Share IDs: {:?}", flemish_share_ids);

    let categories = if !category_ids.is_empty() {
        load_entities_by_ids(&client, "nrq_categories", "nrq_categoryid", &category_ids).await?
    } else {
        vec![]
    };
    let domains = if !domain_ids.is_empty() {
        load_entities_by_ids(&client, "nrq_domains", "nrq_domainid", &domain_ids).await?
    } else {
        vec![]
    };
    let funds = if !fund_ids.is_empty() {
        load_entities_by_ids(&client, "nrq_funds", "nrq_fundid", &fund_ids).await?
    } else {
        vec![]
    };
    let supports = if !support_ids.is_empty() {
        load_entities_by_ids(&client, "nrq_supports", "nrq_supportid", &support_ids).await?
    } else {
        vec![]
    };
    let types = if !type_ids.is_empty() {
        load_entities_by_ids(&client, "nrq_types", "nrq_typeid", &type_ids).await?
    } else {
        vec![]
    };
    let subcategories = if !subcategory_ids.is_empty() {
        load_entities_by_ids(
            &client,
            "nrq_subcategories",
            "nrq_subcategoryid",
            &subcategory_ids,
        )
        .await?
    } else {
        vec![]
    };
    let flemish_shares = if !flemish_share_ids.is_empty() {
        load_entities_by_ids(
            &client,
            "nrq_flemishshares",
            "nrq_flemishshareid",
            &flemish_share_ids,
        )
        .await?
    } else {
        vec![]
    };

    log::debug!("Loaded classification records:");
    log::debug!("  Categories: {}", categories.len());
    log::debug!("  Domains: {}", domains.len());
    log::debug!("  Funds: {}", funds.len());
    log::debug!("  Supports: {}", supports.len());
    log::debug!("  Types: {}", types.len());
    log::debug!("  Subcategories: {}", subcategories.len());
    log::debug!("  Flemish Shares: {}", flemish_shares.len());

    log::info!(
        "Successfully loaded complete snapshot with {} total entities",
        1 + pages.len()
            + page_lines.len()
            + groups.len()
            + group_lines.len()
            + questions.len()
            + template_lines.len()
            + conditions.len()
            + condition_actions.len()
            + categories.len()
            + domains.len()
            + funds.len()
            + supports.len()
            + types.len()
            + subcategories.len()
            + flemish_shares.len()
    );

    Ok(QuestionnaireSnapshot {
        questionnaire,
        pages,
        page_lines,
        groups,
        group_lines,
        questions,
        template_lines,
        conditions,
        condition_actions,
        categories,
        domains,
        funds,
        supports,
        types,
        subcategories,
        flemish_shares,
    })
}

/// Load a single questionnaire entity by ID
async fn load_questionnaire(
    client: &crate::api::DynamicsClient,
    id: &str,
) -> Result<Value, String> {
    client
        .fetch_record_by_id("nrq_questionnaire", id)
        .await
        .map_err(|e| format!("Failed to load questionnaire: {}", e))
}

/// Load page lines (ordering junctions) for a questionnaire
async fn load_page_lines(
    client: &crate::api::DynamicsClient,
    questionnaire_id: &str,
) -> Result<Vec<Value>, String> {
    let mut query = Query::new("nrq_questionnairepagelines");
    query.filter = Some(Filter::eq(
        "_nrq_questionnaireid_value",
        FilterValue::Guid(questionnaire_id.to_string()),
    ));

    log::debug!(
        "Loading page_lines with questionnaire_id: {}",
        questionnaire_id
    );
    if let Some(ref filter) = query.filter {
        log::debug!("Filter OData string: {}", filter.to_odata_string());
    }

    let result = client
        .execute_query(&query)
        .await
        .map_err(|e| format!("Failed to load page lines: {}", e))?;

    log::debug!(
        "Page lines result count: {}",
        result.data.as_ref().map(|d| d.value.len()).unwrap_or(0)
    );

    Ok(result.data.map(|d| d.value).unwrap_or_default())
}

/// Load pages by IDs
async fn load_pages(
    client: &crate::api::DynamicsClient,
    page_ids: &[String],
) -> Result<Vec<Value>, String> {
    if page_ids.is_empty() {
        return Ok(vec![]);
    }

    let mut query = Query::new("nrq_questionnairepages");
    query.filter = Some(Filter::any_of("nrq_questionnairepageid", page_ids));

    log::debug!("Loading pages with IDs: {:?}", page_ids);
    if let Some(ref filter) = query.filter {
        log::debug!("Pages filter: {}", filter.to_odata_string());
    }

    let result = client
        .execute_query(&query)
        .await
        .map_err(|e| format!("Failed to load pages: {}", e))?;

    log::debug!(
        "Pages result count: {}",
        result.data.as_ref().map(|d| d.value.len()).unwrap_or(0)
    );

    Ok(result.data.map(|d| d.value).unwrap_or_default())
}

/// Load group lines (ordering junctions) for pages
async fn load_group_lines(
    client: &crate::api::DynamicsClient,
    page_ids: &[String],
) -> Result<Vec<Value>, String> {
    if page_ids.is_empty() {
        return Ok(vec![]);
    }

    let mut query = Query::new("nrq_questiongrouplines");
    query.filter = Some(Filter::any_of("_nrq_questionnairepageid_value", page_ids));

    let result = client
        .execute_query(&query)
        .await
        .map_err(|e| format!("Failed to load group lines: {}", e))?;

    Ok(result.data.map(|d| d.value).unwrap_or_default())
}

/// Load groups by IDs
async fn load_groups(
    client: &crate::api::DynamicsClient,
    group_ids: &[String],
) -> Result<Vec<Value>, String> {
    if group_ids.is_empty() {
        return Ok(vec![]);
    }

    let mut query = Query::new("nrq_questiongroups");
    query.filter = Some(Filter::any_of("nrq_questiongroupid", group_ids));

    let result = client
        .execute_query(&query)
        .await
        .map_err(|e| format!("Failed to load groups: {}", e))?;

    Ok(result.data.map(|d| d.value).unwrap_or_default())
}

/// Load questions related to a questionnaire
async fn load_questions(
    client: &crate::api::DynamicsClient,
    questionnaire_id: &str,
) -> Result<Vec<Value>, String> {
    let mut query = Query::new("nrq_questions");
    query.filter = Some(Filter::eq(
        "_nrq_questionnaireid_value",
        FilterValue::Guid(questionnaire_id.to_string()),
    ));

    log::debug!(
        "Loading questions with questionnaire_id: {}",
        questionnaire_id
    );
    if let Some(ref filter) = query.filter {
        log::debug!("Filter OData string: {}", filter.to_odata_string());
    }

    let result = client
        .execute_query(&query)
        .await
        .map_err(|e| format!("Failed to load questions: {}", e))?;

    log::debug!(
        "Questions result count: {}",
        result.data.as_ref().map(|d| d.value.len()).unwrap_or(0)
    );

    Ok(result.data.map(|d| d.value).unwrap_or_default())
}

/// Load template lines related to groups
async fn load_template_lines(
    client: &crate::api::DynamicsClient,
    group_ids: &[String],
) -> Result<Vec<Value>, String> {
    if group_ids.is_empty() {
        return Ok(vec![]);
    }

    let mut query = Query::new("nrq_questiontemplatelines");
    query.filter = Some(Filter::any_of("_nrq_questiongroupid_value", group_ids));

    let result = client
        .execute_query(&query)
        .await
        .map_err(|e| format!("Failed to load template lines: {}", e))?;

    Ok(result.data.map(|d| d.value).unwrap_or_default())
}

/// Load conditions related to questions
async fn load_conditions(
    client: &crate::api::DynamicsClient,
    question_ids: &[String],
) -> Result<Vec<Value>, String> {
    if question_ids.is_empty() {
        return Ok(vec![]);
    }

    let mut query = Query::new("nrq_questionconditions");
    query.filter = Some(Filter::any_of("_nrq_questionid_value", question_ids));

    let result = client
        .execute_query(&query)
        .await
        .map_err(|e| format!("Failed to load conditions: {}", e))?;

    Ok(result.data.map(|d| d.value).unwrap_or_default())
}

/// Load condition actions related to conditions
async fn load_condition_actions(
    client: &crate::api::DynamicsClient,
    condition_ids: &[String],
) -> Result<Vec<Value>, String> {
    if condition_ids.is_empty() {
        return Ok(vec![]);
    }

    let mut query = Query::new("nrq_questionconditionactions");
    query.filter = Some(Filter::any_of(
        "_nrq_questionconditionid_value",
        condition_ids,
    ));

    let result = client
        .execute_query(&query)
        .await
        .map_err(|e| format!("Failed to load condition actions: {}", e))?;

    Ok(result.data.map(|d| d.value).unwrap_or_default())
}

/// Load N:N relationship IDs using navigation properties
/// Returns the IDs of the related entities (e.g., category IDs)
///
/// For N:N relationships in Dynamics 365, we use the navigation property pattern:
/// nrq_questionnaires(<id>)/nrq_questionnaire_nrq_Category_nrq_Category
///
/// The navigation property format is: {relationship_name}_nrq_{PascalCaseEntity}_nrq_{PascalCaseEntity}
async fn load_n_n_relationship(
    client: &crate::api::DynamicsClient,
    relationship_name: &str,
    questionnaire_id: &str,
    related_id_field: &str,
) -> Result<Vec<String>, String> {
    // Convert relationship name to navigation property
    // Example: "nrq_questionnaire_nrq_category" -> "nrq_questionnaire_nrq_Category_nrq_Category"
    let entity_name = relationship_name
        .strip_prefix("nrq_questionnaire_nrq_")
        .ok_or_else(|| format!("Invalid relationship name: {}", relationship_name))?;

    // Map lowercase entity names to PascalCase (handles compound words like FlemishShare)
    let pascal_case = match entity_name {
        "category" => "Category",
        "domain" => "Domain",
        "fund" => "Fund",
        "support" => "Support",
        "type" => "Type",
        "subcategory" => "Subcategory",
        "flemishshare" => "FlemishShare",
        _ => {
            // Fallback: just capitalize first letter
            return Err(format!(
                "Unknown entity name in relationship: {}",
                entity_name
            ));
        }
    };

    let navigation_property = format!("nrq_questionnaire_nrq_{}_nrq_{}", pascal_case, pascal_case);

    log::debug!(
        "Loading N:N relationship via navigation property: {}",
        navigation_property
    );

    // Use the new execute_navigation_property method
    let result = client
        .execute_navigation_property(
            "nrq_questionnaires",
            questionnaire_id,
            &navigation_property,
            Some(vec![related_id_field.to_string()]),
        )
        .await
        .map_err(|e| format!("Failed to load N:N relationship: {}", e))?;

    let records = result
        .get("value")
        .and_then(|v| v.as_array())
        .map(|arr| arr.clone())
        .unwrap_or_default();

    log::debug!("{} returned {} records", navigation_property, records.len());

    // Extract the related entity IDs
    let ids: Vec<String> = records
        .iter()
        .filter_map(|record| {
            record
                .get(related_id_field)
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .collect();

    log::debug!(
        "{} extracted {} IDs from field '{}'",
        navigation_property,
        ids.len(),
        related_id_field
    );

    Ok(ids)
}

/// Extract IDs from a Vec<Value> given a field name
fn extract_ids(records: &[Value], id_field: &str) -> Vec<String> {
    records
        .iter()
        .filter_map(|record| {
            record
                .get(id_field)
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .collect()
}

/// Load entities by their IDs (for classification entities)
async fn load_entities_by_ids(
    client: &crate::api::DynamicsClient,
    entity_name: &str,
    id_field: &str,
    ids: &[String],
) -> Result<Vec<Value>, String> {
    if ids.is_empty() {
        return Ok(vec![]);
    }

    let mut query = Query::new(entity_name);
    query.filter = Some(Filter::any_of(id_field, ids));

    let result = client
        .execute_query(&query)
        .await
        .map_err(|e| format!("Failed to load {} entities: {}", entity_name, e))?;

    Ok(result.data.map(|d| d.value).unwrap_or_default())
}

/// Convert raw snapshot into structured domain model
pub fn build_domain_model(snapshot: QuestionnaireSnapshot) -> Result<Questionnaire, String> {
    // Build lookup maps
    let mut page_groups: HashMap<String, Vec<&Value>> = HashMap::new();
    for group_line in &snapshot.group_lines {
        if let Some(page_id) = group_line
            .get("_nrq_questionnairepageid_value")
            .and_then(|v| v.as_str())
        {
            if let Some(group_id) = group_line
                .get("_nrq_questiongroupid_value")
                .and_then(|v| v.as_str())
            {
                if let Some(group) = snapshot.groups.iter().find(|g| {
                    g.get("nrq_questiongroupid").and_then(|v| v.as_str()) == Some(group_id)
                }) {
                    page_groups
                        .entry(page_id.to_string())
                        .or_insert_with(Vec::new)
                        .push(group);
                }
            }
        }
    }

    let mut group_questions: HashMap<String, Vec<&Value>> = HashMap::new();
    for question in &snapshot.questions {
        if let Some(group_id) = question
            .get("_nrq_questiongroupid_value")
            .and_then(|v| v.as_str())
        {
            group_questions
                .entry(group_id.to_string())
                .or_insert_with(Vec::new)
                .push(question);
        }
    }

    let mut condition_actions_map: HashMap<String, Vec<&Value>> = HashMap::new();
    for action in &snapshot.condition_actions {
        if let Some(condition_id) = action
            .get("_nrq_questionconditionid_value")
            .and_then(|v| v.as_str())
        {
            condition_actions_map
                .entry(condition_id.to_string())
                .or_insert_with(Vec::new)
                .push(action);
        }
    }

    // Build pages with groups and questions
    let pages: Vec<Page> = snapshot
        .pages
        .iter()
        .map(|page_val| {
            let page_id = page_val
                .get("nrq_questionnairepageid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let page_name = page_val
                .get("nrq_name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unnamed Page")
                .to_string();

            let groups = page_groups
                .get(&page_id)
                .map(|gs| gs.as_slice())
                .unwrap_or(&[])
                .iter()
                .map(|group_val| {
                    let group_id = group_val
                        .get("nrq_questiongroupid")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let group_name = group_val
                        .get("nrq_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unnamed Group")
                        .to_string();

                    let questions = group_questions
                        .get(&group_id)
                        .map(|qs| qs.as_slice())
                        .unwrap_or(&[])
                        .iter()
                        .map(|question_val| {
                            let question_id = question_val
                                .get("nrq_questionid")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let question_name = question_val
                                .get("nrq_name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unnamed Question")
                                .to_string();

                            let tag = question_val
                                .get("_nrq_questiontagid_value")
                                .and_then(|v| v.as_str())
                                .map(|id| Reference {
                                    id: id.to_string(),
                                    name: None,
                                });

                            let template = question_val
                                .get("_nrq_questiontemplateid_value")
                                .and_then(|v| v.as_str())
                                .map(|id| Reference {
                                    id: id.to_string(),
                                    name: None,
                                });

                            Question {
                                id: question_id,
                                name: question_name,
                                raw: (*question_val).clone(),
                                tag,
                                template,
                            }
                        })
                        .collect();

                    Group {
                        id: group_id,
                        name: group_name,
                        order: None,
                        raw: (*group_val).clone(),
                        questions,
                    }
                })
                .collect();

            Page {
                id: page_id,
                name: page_name,
                order: None,
                raw: page_val.clone(),
                groups,
            }
        })
        .collect();

    // Build template lines
    let template_lines: Vec<TemplateLine> = snapshot
        .template_lines
        .iter()
        .map(|line_val| {
            let id = line_val
                .get("nrq_questiontemplatelineid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let template_id = line_val
                .get("_nrq_questiontemplateid_value")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let group_id = line_val
                .get("_nrq_questiongroupid_value")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            TemplateLine {
                id,
                raw: line_val.clone(),
                template: Reference {
                    id: template_id,
                    name: None,
                },
                group_id,
            }
        })
        .collect();

    // Build conditions with actions
    let conditions: Vec<Condition> = snapshot
        .conditions
        .iter()
        .map(|condition_val| {
            let condition_id = condition_val
                .get("nrq_questionconditionid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let condition_name = condition_val
                .get("nrq_name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unnamed Condition")
                .to_string();

            let actions = condition_actions_map
                .get(&condition_id)
                .map(|as_ref| as_ref.as_slice())
                .unwrap_or(&[])
                .iter()
                .map(|action_val| {
                    let action_id = action_val
                        .get("nrq_questionconditionactionid")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let action_name = action_val
                        .get("nrq_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unnamed Action")
                        .to_string();

                    ConditionAction {
                        id: action_id,
                        name: action_name,
                        raw: (*action_val).clone(),
                    }
                })
                .collect();

            Condition {
                id: condition_id,
                name: condition_name,
                raw: condition_val.clone(),
                actions,
            }
        })
        .collect();

    // Build classifications - extract ID and name from entity records
    let classifications = Classifications {
        categories: snapshot
            .categories
            .iter()
            .map(|record| Reference {
                id: record
                    .get("nrq_categoryid")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: record
                    .get("nrq_name")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
            .collect(),
        domains: snapshot
            .domains
            .iter()
            .map(|record| Reference {
                id: record
                    .get("nrq_domainid")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: record
                    .get("nrq_name")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
            .collect(),
        funds: snapshot
            .funds
            .iter()
            .map(|record| Reference {
                id: record
                    .get("nrq_fundid")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: record
                    .get("nrq_name")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
            .collect(),
        supports: snapshot
            .supports
            .iter()
            .map(|record| Reference {
                id: record
                    .get("nrq_supportid")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: record
                    .get("nrq_name")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
            .collect(),
        types: snapshot
            .types
            .iter()
            .map(|record| Reference {
                id: record
                    .get("nrq_typeid")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: record
                    .get("nrq_name")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
            .collect(),
        subcategories: snapshot
            .subcategories
            .iter()
            .map(|record| Reference {
                id: record
                    .get("nrq_subcategoryid")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: record
                    .get("nrq_name")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
            .collect(),
        flemish_shares: snapshot
            .flemish_shares
            .iter()
            .map(|record| Reference {
                id: record
                    .get("nrq_flemishshareid")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: record
                    .get("nrq_name")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
            .collect(),
    };

    let questionnaire_id = snapshot
        .questionnaire
        .get("nrq_questionnaireid")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let questionnaire_name = snapshot
        .questionnaire
        .get("nrq_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unnamed Questionnaire")
        .to_string();

    Ok(Questionnaire {
        id: questionnaire_id,
        name: questionnaire_name,
        raw: snapshot.questionnaire,
        pages,
        page_lines: snapshot.page_lines,
        group_lines: snapshot.group_lines,
        template_lines,
        conditions,
        classifications,
    })
}
