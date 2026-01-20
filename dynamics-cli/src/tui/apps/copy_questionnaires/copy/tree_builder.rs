use super::domain::*;
use super::field_filter::{ConditionLogic, RelevantFields};
use super::tree_items::{CopyBadge, SnapshotTreeItem};
use serde_json::Value;
use std::collections::HashMap;

/// Helper to build filtered attribute list from JSON object
fn build_filtered_fields(
    value: &Value,
    filter: &RelevantFields,
    parent_id: &str,
) -> Vec<SnapshotTreeItem> {
    filter
        .filter_fields(value)
        .into_iter()
        .map(|(key, val)| SnapshotTreeItem::Attribute {
            parent_id: parent_id.to_string(),
            label: key,
            value: val,
        })
        .collect()
}

/// Create a Fields category if there are any fields
fn fields_category(
    value: &Value,
    filter: &RelevantFields,
    unique_id: &str,
) -> Option<SnapshotTreeItem> {
    let fields = build_filtered_fields(value, filter, unique_id);
    if fields.is_empty() {
        None
    } else {
        Some(SnapshotTreeItem::Category {
            id: format!("fields:{}", unique_id),
            name: "Fields".to_string(),
            count: fields.len(),
            children: fields,
        })
    }
}

pub fn build_snapshot_tree(questionnaire: &Questionnaire) -> Vec<SnapshotTreeItem> {
    let mut questionnaire_children = vec![];

    // Build lookup maps
    let mut page_lines_by_page: HashMap<&str, Vec<&Value>> = HashMap::new();
    for line in &questionnaire.page_lines {
        if let Some(page_id) = line
            .get("_nrq_questionnairepageid_value")
            .and_then(|v| v.as_str())
        {
            page_lines_by_page
                .entry(page_id)
                .or_insert_with(Vec::new)
                .push(line);
        }
    }

    let mut group_lines_by_page: HashMap<&str, Vec<&Value>> = HashMap::new();
    for line in &questionnaire.group_lines {
        if let Some(page_id) = line
            .get("_nrq_questionnairepageid_value")
            .and_then(|v| v.as_str())
        {
            group_lines_by_page
                .entry(page_id)
                .or_insert_with(Vec::new)
                .push(line);
        }
    }

    let mut template_lines_by_group: HashMap<&str, Vec<&TemplateLine>> = HashMap::new();
    for template_line in &questionnaire.template_lines {
        template_lines_by_group
            .entry(template_line.group_id.as_str())
            .or_insert_with(Vec::new)
            .push(template_line);
    }

    let mut conditions_by_question: HashMap<&str, Vec<&Condition>> = HashMap::new();
    for condition in &questionnaire.conditions {
        if let Some(question_id) = condition
            .raw
            .get("_nrq_questionid_value")
            .and_then(|v| v.as_str())
        {
            conditions_by_question
                .entry(question_id)
                .or_insert_with(Vec::new)
                .push(condition);
        }
    }

    // 1. Questionnaire Fields section
    if let Some(fields) = fields_category(
        &questionnaire.raw,
        &RelevantFields::for_questionnaire(),
        &questionnaire.id,
    ) {
        questionnaire_children.push(fields);
    }

    // 2. Pages section (with integrated page_lines and group_lines)
    if !questionnaire.pages.is_empty() {
        let page_children: Vec<SnapshotTreeItem> = questionnaire.pages.iter()
            .map(|page| {
                let mut page_children_vec = vec![];

                // Add page line (junction) if exists
                if let Some(page_lines) = page_lines_by_page.get(page.id.as_str()) {
                    for line in page_lines {
                        let id = line.get("nrq_questionnairepagelineid")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let order = line.get("nrq_order")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);

                        let mut junction_children = vec![];
                        if let Some(fields) = fields_category(line, &RelevantFields::for_page_line(), &id) {
                            junction_children.push(fields);
                        }

                        page_children_vec.push(SnapshotTreeItem::JunctionRecord {
                            name: "Page Line".to_string(),
                            id,
                            description: Some(format!("Order: {}", order)),
                            children: junction_children,
                        });
                    }
                }

                // Add page fields
                if let Some(fields) = fields_category(&page.raw, &RelevantFields::for_page(), &page.id) {
                    page_children_vec.push(fields);
                }

                // Add groups under this page
                if !page.groups.is_empty() {
                    let group_children: Vec<SnapshotTreeItem> = page.groups.iter()
                        .map(|group| {
                            let mut group_children_vec = vec![];

                            // Add group line (junction) if exists
                            if let Some(group_lines) = group_lines_by_page.get(page.id.as_str()) {
                                for line in group_lines {
                                    if let Some(group_id_in_line) = line.get("_nrq_questiongroupid_value").and_then(|v| v.as_str()) {
                                        if group_id_in_line == group.id {
                                            let id = line.get("nrq_questiongrouplineid")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("unknown")
                                                .to_string();
                                            let order = line.get("nrq_order")
                                                .and_then(|v| v.as_i64())
                                                .unwrap_or(0);

                                            let mut junction_children = vec![];
                                            if let Some(fields) = fields_category(line, &RelevantFields::for_group_line(), &id) {
                                                junction_children.push(fields);
                                            }

                                            group_children_vec.push(SnapshotTreeItem::JunctionRecord {
                                                name: "Group Line".to_string(),
                                                id,
                                                description: Some(format!("Order: {}", order)),
                                                children: junction_children,
                                            });
                                        }
                                    }
                                }
                            }

                            // Add group fields
                            if let Some(fields) = fields_category(&group.raw, &RelevantFields::for_group(), &group.id) {
                                group_children_vec.push(fields);
                            }

                            // Add questions under this group
                            if !group.questions.is_empty() {
                                let question_children: Vec<SnapshotTreeItem> = group.questions.iter()
                                    .map(|question| {
                                        let mut question_children_vec = vec![];

                                        // Add question fields
                                        if let Some(fields) = fields_category(&question.raw, &RelevantFields::for_question(), &question.id) {
                                            question_children_vec.push(fields);
                                        }

                                        // Add template reference if exists
                                        if let Some(template) = &question.template {
                                            question_children_vec.push(SnapshotTreeItem::ReferencedEntity {
                                                name: format!("Template → {}", template.name.clone().unwrap_or_else(|| template.id.clone())),
                                                id: template.id.clone(),
                                                entity_type: "nrq_questiontemplate".to_string(),
                                            });
                                        }

                                        // Add tag reference if exists
                                        if let Some(tag) = &question.tag {
                                            question_children_vec.push(SnapshotTreeItem::ReferencedEntity {
                                                name: format!("Tag → {}", tag.name.clone().unwrap_or_else(|| tag.id.clone())),
                                                id: tag.id.clone(),
                                                entity_type: "nrq_questiontag".to_string(),
                                            });
                                        }

                                        // Add conditions for this question if any exist
                                        if let Some(conditions) = conditions_by_question.get(question.id.as_str()) {
                                            let condition_items: Vec<SnapshotTreeItem> = conditions.iter()
                                                .map(|condition| {
                                                    let mut condition_children_vec = vec![];

                                                    // Parse and add condition JSON logic
                                                    if let Some(json_str) = condition.raw.get("nrq_conditionjson").and_then(|v| v.as_str()) {
                                                        match ConditionLogic::parse(json_str) {
                                                            Ok(logic) => {
                                                                let details: Vec<String> = logic.affected_questions.iter()
                                                                    .map(|q| format!("Question {} → visible: {}, required: {}",
                                                                        &q.question_id[..8.min(q.question_id.len())],
                                                                        q.visible,
                                                                        q.required
                                                                    ))
                                                                    .collect();

                                                                condition_children_vec.push(SnapshotTreeItem::ConditionLogicInfo {
                                                                    trigger_question_id: logic.trigger_question_id.clone(),
                                                                    condition_operator: logic.format_operator().to_string(),
                                                                    value: logic.value.clone(),
                                                                    affected_count: logic.affected_questions.len(),
                                                                    details,
                                                                });
                                                            }
                                                            Err(e) => {
                                                                log::warn!("Failed to parse condition JSON for {}: {}", condition.id, e);
                                                            }
                                                        }
                                                    }

                                                    // Add condition fields
                                                    if let Some(fields) = fields_category(&condition.raw, &RelevantFields::for_condition(), &condition.id) {
                                                        condition_children_vec.push(fields);
                                                    }

                                                    // Add actions
                                                    if !condition.actions.is_empty() {
                                                        let action_children: Vec<SnapshotTreeItem> = condition.actions.iter()
                                                            .map(|action| {
                                                                let mut action_children_vec = vec![];
                                                                if let Some(fields) = fields_category(&action.raw, &RelevantFields::for_condition_action(), &action.id) {
                                                                    action_children_vec.push(fields);
                                                                }

                                                                SnapshotTreeItem::Entity {
                                                                    name: action.name.clone(),
                                                                    id: action.id.clone(),
                                                                    badge: Some(CopyBadge::Remap),
                                                                    children: action_children_vec,
                                                                }
                                                            })
                                                            .collect();

                                                        condition_children_vec.push(SnapshotTreeItem::Category {
                                                            id: format!("actions:{}", condition.id),
                                                            name: "Actions".to_string(),
                                                            count: action_children.len(),
                                                            children: action_children,
                                                        });
                                                    }

                                                    SnapshotTreeItem::Entity {
                                                        name: condition.name.clone(),
                                                        id: condition.id.clone(),
                                                        badge: Some(CopyBadge::Remap),
                                                        children: condition_children_vec,
                                                    }
                                                })
                                                .collect();

                                            question_children_vec.push(SnapshotTreeItem::Category {
                                                id: format!("conditions:{}", question.id),
                                                name: "Conditions".to_string(),
                                                count: condition_items.len(),
                                                children: condition_items,
                                            });
                                        }

                                        SnapshotTreeItem::Entity {
                                            name: question.name.clone(),
                                            id: question.id.clone(),
                                            badge: Some(CopyBadge::Copy),
                                            children: question_children_vec,
                                        }
                                    })
                                    .collect();

                                group_children_vec.push(SnapshotTreeItem::Category {
                                    id: format!("questions:{}", group.id),
                                    name: "Questions".to_string(),
                                    count: question_children.len(),
                                    children: question_children,
                                });
                            }

                            // Add template lines under this group
                            if let Some(template_lines) = template_lines_by_group.get(group.id.as_str()) {
                                let template_line_items: Vec<SnapshotTreeItem> = template_lines.iter()
                                    .map(|line| {
                                        let mut line_children_vec = vec![];

                                        // Add template reference
                                        line_children_vec.push(SnapshotTreeItem::ReferencedEntity {
                                            name: format!("Template → {}", line.template.name.clone().unwrap_or_else(|| line.template.id.clone())),
                                            id: line.template.id.clone(),
                                            entity_type: "nrq_questiontemplate".to_string(),
                                        });

                                        // Add template line fields
                                        if let Some(fields) = fields_category(&line.raw, &RelevantFields::for_template_line(), &line.id) {
                                            line_children_vec.push(fields);
                                        }

                                        SnapshotTreeItem::JunctionRecord {
                                            name: format!("Template Line: {}", line.template.name.clone().unwrap_or_else(|| "Unknown".to_string())),
                                            id: line.id.clone(),
                                            description: None,
                                            children: line_children_vec,
                                        }
                                    })
                                    .collect();

                                group_children_vec.push(SnapshotTreeItem::Category {
                                    id: format!("templatelines:{}", group.id),
                                    name: "Template Lines".to_string(),
                                    count: template_line_items.len(),
                                    children: template_line_items,
                                });
                            }

                            SnapshotTreeItem::Entity {
                                name: group.name.clone(),
                                id: group.id.clone(),
                                badge: Some(CopyBadge::Copy),
                                children: group_children_vec,
                            }
                        })
                        .collect();

                    page_children_vec.push(SnapshotTreeItem::Category {
                        id: format!("groups:{}", page.id),
                        name: "Groups".to_string(),
                        count: group_children.len(),
                        children: group_children,
                    });
                }

                SnapshotTreeItem::Entity {
                    name: page.name.clone(),
                    id: page.id.clone(),
                    badge: Some(CopyBadge::Copy),
                    children: page_children_vec,
                }
            })
            .collect();

        questionnaire_children.push(SnapshotTreeItem::Category {
            id: format!("pages:{}", questionnaire.id),
            name: "Pages".to_string(),
            count: page_children.len(),
            children: page_children,
        });
    }

    // 3. Classifications section (N:N relationships with junction notes)
    let mut classification_children = vec![];

    if !questionnaire.classifications.categories.is_empty() {
        let category_items: Vec<SnapshotTreeItem> = questionnaire
            .classifications
            .categories
            .iter()
            .map(|ref_item| SnapshotTreeItem::ReferencedEntity {
                name: ref_item.name.clone().unwrap_or_else(|| ref_item.id.clone()),
                id: ref_item.id.clone(),
                entity_type: "nrq_category".to_string(),
            })
            .collect();

        classification_children.push(SnapshotTreeItem::Category {
            id: format!("class_categories:{}", questionnaire.id),
            name: "Categories (via new junctions)".to_string(),
            count: category_items.len(),
            children: category_items,
        });
    }

    if !questionnaire.classifications.domains.is_empty() {
        let domain_items: Vec<SnapshotTreeItem> = questionnaire
            .classifications
            .domains
            .iter()
            .map(|ref_item| SnapshotTreeItem::ReferencedEntity {
                name: ref_item.name.clone().unwrap_or_else(|| ref_item.id.clone()),
                id: ref_item.id.clone(),
                entity_type: "nrq_domain".to_string(),
            })
            .collect();

        classification_children.push(SnapshotTreeItem::Category {
            id: format!("class_domains:{}", questionnaire.id),
            name: "Domains (via new junctions)".to_string(),
            count: domain_items.len(),
            children: domain_items,
        });
    }

    if !questionnaire.classifications.funds.is_empty() {
        let fund_items: Vec<SnapshotTreeItem> = questionnaire
            .classifications
            .funds
            .iter()
            .map(|ref_item| SnapshotTreeItem::ReferencedEntity {
                name: ref_item.name.clone().unwrap_or_else(|| ref_item.id.clone()),
                id: ref_item.id.clone(),
                entity_type: "nrq_fund".to_string(),
            })
            .collect();

        classification_children.push(SnapshotTreeItem::Category {
            id: format!("class_funds:{}", questionnaire.id),
            name: "Funds (via new junctions)".to_string(),
            count: fund_items.len(),
            children: fund_items,
        });
    }

    if !questionnaire.classifications.supports.is_empty() {
        let support_items: Vec<SnapshotTreeItem> = questionnaire
            .classifications
            .supports
            .iter()
            .map(|ref_item| SnapshotTreeItem::ReferencedEntity {
                name: ref_item.name.clone().unwrap_or_else(|| ref_item.id.clone()),
                id: ref_item.id.clone(),
                entity_type: "nrq_support".to_string(),
            })
            .collect();

        classification_children.push(SnapshotTreeItem::Category {
            id: format!("class_supports:{}", questionnaire.id),
            name: "Supports (via new junctions)".to_string(),
            count: support_items.len(),
            children: support_items,
        });
    }

    if !questionnaire.classifications.types.is_empty() {
        let type_items: Vec<SnapshotTreeItem> = questionnaire
            .classifications
            .types
            .iter()
            .map(|ref_item| SnapshotTreeItem::ReferencedEntity {
                name: ref_item.name.clone().unwrap_or_else(|| ref_item.id.clone()),
                id: ref_item.id.clone(),
                entity_type: "nrq_type".to_string(),
            })
            .collect();

        classification_children.push(SnapshotTreeItem::Category {
            id: format!("class_types:{}", questionnaire.id),
            name: "Types (via new junctions)".to_string(),
            count: type_items.len(),
            children: type_items,
        });
    }

    if !questionnaire.classifications.subcategories.is_empty() {
        let subcategory_items: Vec<SnapshotTreeItem> = questionnaire
            .classifications
            .subcategories
            .iter()
            .map(|ref_item| SnapshotTreeItem::ReferencedEntity {
                name: ref_item.name.clone().unwrap_or_else(|| ref_item.id.clone()),
                id: ref_item.id.clone(),
                entity_type: "nrq_subcategory".to_string(),
            })
            .collect();

        classification_children.push(SnapshotTreeItem::Category {
            id: format!("class_subcategories:{}", questionnaire.id),
            name: "Subcategories (via new junctions)".to_string(),
            count: subcategory_items.len(),
            children: subcategory_items,
        });
    }

    if !questionnaire.classifications.flemish_shares.is_empty() {
        let flemish_share_items: Vec<SnapshotTreeItem> = questionnaire
            .classifications
            .flemish_shares
            .iter()
            .map(|ref_item| SnapshotTreeItem::ReferencedEntity {
                name: ref_item.name.clone().unwrap_or_else(|| ref_item.id.clone()),
                id: ref_item.id.clone(),
                entity_type: "nrq_flemishshare".to_string(),
            })
            .collect();

        classification_children.push(SnapshotTreeItem::Category {
            id: format!("class_flemish_shares:{}", questionnaire.id),
            name: "Flemish Shares (via new junctions)".to_string(),
            count: flemish_share_items.len(),
            children: flemish_share_items,
        });
    }

    if !classification_children.is_empty() {
        let total_count: usize = classification_children
            .iter()
            .map(|c| {
                if let SnapshotTreeItem::Category { count, .. } = c {
                    *count
                } else {
                    0
                }
            })
            .sum();

        questionnaire_children.push(SnapshotTreeItem::Category {
            id: format!("classifications:{}", questionnaire.id),
            name: "Classifications".to_string(),
            count: total_count,
            children: classification_children,
        });
    }

    // Return single questionnaire root node
    vec![SnapshotTreeItem::QuestionnaireRoot {
        name: questionnaire.name.clone(),
        id: questionnaire.id.clone(),
        badge: Some(CopyBadge::Copy),
        children: questionnaire_children,
    }]
}
