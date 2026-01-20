/// Helper functions for data transformation and field manipulation
use super::entity_sets;
use super::field_specs::{FieldSpec, FieldType};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// Get list of shared entity field names (not copied, referenced from existing entities)
pub fn get_shared_entities() -> HashSet<&'static str> {
    let mut set = HashSet::new();
    set.insert("questiontemplateid");
    set.insert("questiontagid");
    set.insert("contactrole");
    set.insert("categoryid");
    set.insert("domainid");
    set.insert("fundid");
    set.insert("supportid");
    set.insert("typeid");
    set.insert("subcategoryid");
    set.insert("flemishshareid");
    set
}

/// Build a clean payload from raw entity data using field specifications
///
/// This replaces the old remove_system_fields() + remap_lookup_fields() approach
/// with an allowlist-based system that:
/// 1. Only includes fields defined in the spec (avoiding OData annotations)
/// 2. Transforms lookup fields to @odata.bind format
/// 3. Applies ID remapping for copied entities vs shared entities
pub fn build_payload(
    raw: &Value,
    fields: &[FieldSpec],
    id_map: &HashMap<String, String>,
    shared_entities: &HashSet<&str>,
) -> Result<Value, String> {
    let mut payload = json!({});

    for spec in fields {
        if let Some(value) = raw.get(spec.source_name) {
            // Skip null values
            if value.is_null() {
                continue;
            }

            match &spec.field_type {
                FieldType::Value => {
                    // FIXME: Validate option set values against metadata to handle cross-environment copies
                    // For now, only validate known-problematic fields that may have orphaned option set values
                    let final_value = if spec.field_name == "nrq_requeststatus" {
                        if let Some(num) = value.as_i64() {
                            // Valid values for nrq_requeststatus: 1, 875810001-875810016
                            // Orphaned values like 170590008 should default to 1 (Draft)
                            let is_valid = num == 1 || (num >= 875810001 && num <= 875810016);
                            if !is_valid {
                                log::warn!(
                                    "Field {} has invalid option set value {}, defaulting to 1 (Draft)",
                                    spec.field_name,
                                    num
                                );
                                json!(1)
                            } else {
                                value.clone()
                            }
                        } else {
                            value.clone()
                        }
                    } else {
                        value.clone()
                    };

                    payload[spec.field_name] = final_value;
                }
                FieldType::Lookup { target_entity } => {
                    let guid = value.as_str().ok_or_else(|| {
                        format!("Lookup field {} is not a string", spec.source_name)
                    })?;

                    // Check if this is a shared entity (don't remap)
                    let is_shared = shared_entities
                        .iter()
                        .any(|&entity| spec.field_name.contains(entity));

                    // Remap if not shared and mapping exists
                    let final_guid = if is_shared {
                        guid
                    } else {
                        id_map.get(guid).map(|s| s.as_str()).unwrap_or(guid)
                    };

                    // Transform to @odata.bind format
                    let bind_key = format!("{}@odata.bind", spec.field_name);
                    payload[bind_key] = json!(format!("/{}({})", target_entity, final_guid));
                }
            }
        }
    }

    Ok(payload)
}

/// DEPRECATED: Old approach - kept for reference during migration
/// Remove system-managed fields from entity data before creation
/// These fields are read-only and will be populated by Dynamics
#[allow(dead_code)]
pub fn remove_system_fields(data: &mut Value, id_field: &str) {
    if let Value::Object(map) = data {
        // Remove primary key
        map.remove(id_field);

        // Remove system-managed fields
        map.remove("createdon");
        map.remove("modifiedon");
        map.remove("_createdby_value");
        map.remove("_modifiedby_value");
        map.remove("versionnumber");
    }
}

/// Convert entity set name to friendly display name for UI
pub fn entity_set_to_friendly_name(entity_set: &str) -> &str {
    match entity_set {
        entity_sets::QUESTIONNAIRES => "questionnaire",
        entity_sets::PAGES => "pages",
        entity_sets::PAGE_LINES => "page_lines",
        entity_sets::GROUPS => "groups",
        entity_sets::GROUP_LINES => "group_lines",
        entity_sets::QUESTIONS => "questions",
        entity_sets::TEMPLATE_LINES => "template_lines",
        entity_sets::CONDITIONS => "conditions",
        entity_sets::CONDITION_ACTIONS => "condition_actions",
        _ => entity_set, // Fallback for classifications and unknown types
    }
}

/// DEPRECATED: Old approach - replaced by build_payload()
/// Remap lookup fields from old entity IDs to new entity IDs
/// Handles both copied entities (remapped) and shared entities (preserved)
#[allow(dead_code)]
pub fn remap_lookup_fields(
    raw_data: &Value,
    id_map: &HashMap<String, String>,
    shared_entities: &HashSet<&str>,
) -> Result<Value, String> {
    let mut data = raw_data.clone();

    if let Value::Object(map) = &mut data {
        let mut remapped_fields = Vec::new();

        for (key, value) in map.iter() {
            if key.starts_with('_') && key.ends_with("_value") {
                if let Some(guid) = value.as_str() {
                    let field_name = key.trim_start_matches('_').trim_end_matches("_value");
                    let is_shared = shared_entities
                        .iter()
                        .any(|&entity_field| field_name.contains(entity_field));

                    let final_guid = if is_shared {
                        guid.to_string()
                    } else {
                        // No fallback - error if mapping not found
                        id_map.get(guid).cloned().ok_or_else(|| {
                            format!(
                                "ID mapping not found for GUID {} in field {}",
                                guid, field_name
                            )
                        })?
                    };

                    let entity_set = infer_entity_set_from_field(field_name)?;

                    remapped_fields.push((
                        key.clone(),
                        format!("{}@odata.bind", field_name),
                        format!("/{}({})", entity_set, final_guid),
                    ));
                }
            }
        }

        for (old_key, new_key, new_value) in remapped_fields {
            map.remove(&old_key);
            map.insert(new_key, json!(new_value));
        }
    }

    Ok(data)
}

/// DEPRECATED: Infer entity set name from field name pattern
#[allow(dead_code)]
fn infer_entity_set_from_field(field_name: &str) -> Result<String, String> {
    if field_name.contains("questionnaireid") {
        Ok(entity_sets::QUESTIONNAIRES.to_string())
    } else if field_name.contains("questionnairepageid") {
        Ok(entity_sets::PAGES.to_string())
    } else if field_name.contains("questiongroupid") {
        Ok(entity_sets::GROUPS.to_string())
    } else if field_name.contains("questiontemplateid") {
        Ok(entity_sets::TEMPLATES.to_string())
    } else if field_name.contains("questiontagid") {
        Ok(entity_sets::TAGS.to_string())
    } else if field_name.contains("questionconditionid") {
        Ok(entity_sets::CONDITIONS.to_string())
    } else if field_name.contains("questionid") {
        Ok(entity_sets::QUESTIONS.to_string())
    } else if field_name.contains("categoryid") {
        Ok(entity_sets::CATEGORIES.to_string())
    } else if field_name.contains("domainid") {
        Ok(entity_sets::DOMAINS.to_string())
    } else if field_name.contains("fundid") {
        Ok(entity_sets::FUNDS.to_string())
    } else if field_name.contains("supportid") {
        Ok(entity_sets::SUPPORTS.to_string())
    } else if field_name.contains("typeid") {
        Ok(entity_sets::TYPES.to_string())
    } else if field_name.contains("subcategoryid") {
        Ok(entity_sets::SUBCATEGORIES.to_string())
    } else if field_name.contains("flemishshareid") {
        Ok(entity_sets::FLEMISH_SHARES.to_string())
    } else {
        Err(format!(
            "Unknown entity field: {} - please add explicit mapping",
            field_name
        ))
    }
}

/// Remap question IDs embedded in condition JSON
pub fn remap_condition_json(
    condition_json_str: &str,
    id_map: &HashMap<String, String>,
) -> Result<String, String> {
    let mut json: Value = serde_json::from_str(condition_json_str)
        .map_err(|e| format!("Failed to parse condition JSON: {}", e))?;

    // Remap root questionId - REQUIRED
    if let Some(question_id) = json.get("questionId").and_then(|v| v.as_str()) {
        let new_id = id_map.get(question_id).ok_or_else(|| {
            format!(
                "Question ID {} not found in mapping (root questionId)",
                question_id
            )
        })?;
        json["questionId"] = json!(new_id);
    }

    // Remap questions array - each is REQUIRED
    if let Some(questions) = json.get_mut("questions").and_then(|v| v.as_array_mut()) {
        for (idx, q) in questions.iter_mut().enumerate() {
            if let Some(question_id) = q.get("questionId").and_then(|v| v.as_str()) {
                let new_id = id_map.get(question_id).ok_or_else(|| {
                    format!(
                        "Question ID {} not found in mapping (questions[{}])",
                        question_id, idx
                    )
                })?;
                q["questionId"] = json!(new_id);
            }
        }
    }

    serde_json::to_string(&json).map_err(|e| format!("Failed to serialize condition JSON: {}", e))
}

/// Extract entity ID from OData-EntityId header or response body
pub fn extract_entity_id(
    result: &crate::api::operations::OperationResult,
) -> Result<String, String> {
    // Primary method: Extract from OData-EntityId or Location header
    for (key, value) in &result.headers {
        if key.eq_ignore_ascii_case("odata-entityid") || key.eq_ignore_ascii_case("location") {
            // Format: /entityset(guid) or https://host/api/data/v9.2/entityset(guid)
            if let Some(start) = value.rfind('(') {
                if let Some(end) = value.rfind(')') {
                    if end > start {
                        return Ok(value[start + 1..end].to_string());
                    }
                }
            }
            return Err(format!("Failed to parse {} header: {}", key, value));
        }
    }

    // Fallback: Try response body (when Prefer: return=representation is used)
    if let Some(ref data) = result.data {
        // Try common questionnaire entity ID fields
        if let Some(id_value) = data
            .get("nrq_questionnaireid")
            .or_else(|| data.get("nrq_questionnairepageid"))
            .or_else(|| data.get("nrq_questionnairepagelineid"))
            .or_else(|| data.get("nrq_questiongroupid"))
            .or_else(|| data.get("nrq_questiongrouplineid"))
            .or_else(|| data.get("nrq_questionid"))
            .or_else(|| data.get("nrq_questiontemplatelineid"))
            .or_else(|| data.get("nrq_questionconditionid"))
            .or_else(|| data.get("nrq_questionconditionactionid"))
        {
            if let Some(guid_str) = id_value.as_str() {
                return Ok(guid_str.to_string());
            }
        }
    }

    Err("No OData-EntityId header or ID field found in response body".to_string())
}
