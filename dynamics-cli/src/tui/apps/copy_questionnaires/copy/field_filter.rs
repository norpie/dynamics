use serde_json::Value;
use std::collections::HashSet;

/// System fields that should always be excluded
const SYSTEM_FIELDS: &[&str] = &[
    "createdon",
    "modifiedon",
    "_createdby_value",
    "_modifiedby_value",
    "_createdonbehalfby_value",
    "_modifiedonbehalfby_value",
    "_ownerid_value",
    "_owningbusinessunit_value",
    "_owningteam_value",
    "_owninguser_value",
    "importsequencenumber",
    "overriddencreatedon",
    "timezoneruleversionnumber",
    "utcconversiontimezonecode",
    "versionnumber",
];

/// Relevant fields by entity type
pub struct RelevantFields {
    fields: HashSet<String>,
}

impl RelevantFields {
    pub fn for_questionnaire() -> Self {
        Self {
            fields: [
                "nrq_name",
                "nrq_code",
                "nrq_copypostfix",
                "nrq_publishdate",
                "nrq_type",
                "nrq_description",
                "statecode",
                "statuscode",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }

    pub fn for_page() -> Self {
        Self {
            fields: [
                "nrq_name",
                "nrq_pagecode",
                "nrq_description",
                "nrq_settings",
                "statecode",
                "statuscode",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }

    pub fn for_group() -> Self {
        Self {
            fields: [
                "nrq_name",
                "nrq_code",
                "nrq_description",
                "nrq_enablemultipleentries",
                "statecode",
                "statuscode",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }

    pub fn for_question() -> Self {
        Self {
            fields: [
                "nrq_name",
                "nrq_questiontext",
                "nrq_questiontype",
                "nrq_required",
                "nrq_ismultiselect",
                "nrq_versionnumber",
                "nrq_targetfield",
                "nrq_targetentity",
                "nrq_targetentityfield",
                "nrq_options",
                "nrq_regex",
                "nrq_regexerrormessage",
                "nrq_tooltip",
                "nrq_publicorprivatefile",
                "nrq_uploadfolder",
                "nrq_showasradio",
                "nrq_lookuptype",
                "statecode",
                "statuscode",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }

    pub fn for_page_line() -> Self {
        Self {
            fields: ["nrq_order", "nrq_createquestions"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }

    pub fn for_group_line() -> Self {
        Self {
            fields: ["nrq_order"].iter().map(|s| s.to_string()).collect(),
        }
    }

    pub fn for_template_line() -> Self {
        Self {
            fields: [
                "nrq_name",
                "nrq_order",
                "nrq_size",
                "nrq_code",
                "nrq_settings",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }

    pub fn for_condition() -> Self {
        Self {
            fields: [
                "nrq_name",
                "nrq_logicaloperator",
                "nrq_value",
                "statuscode",
                // nrq_conditionjson handled separately
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }

    pub fn for_condition_action() -> Self {
        Self {
            fields: ["nrq_name", "nrq_visible", "nrq_required", "nrq_settings"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }

    pub fn for_classification() -> Self {
        Self {
            fields: ["nrq_name", "nrq_code"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }

    /// Check if a field should be included
    pub fn should_include(&self, field_name: &str) -> bool {
        // Exclude system fields
        if SYSTEM_FIELDS.contains(&field_name) {
            return false;
        }

        // Exclude @OData annotations
        if field_name.contains("@OData") {
            return false;
        }

        // Exclude lookup fields (they're shown as references)
        if field_name.starts_with('_') && field_name.ends_with("_value") {
            return false;
        }

        // Exclude ID fields (already in node)
        if field_name.ends_with("id") && !field_name.starts_with("nrq_") {
            return false;
        }

        // Include if in whitelist
        self.fields.contains(field_name)
    }

    /// Filter fields from a JSON object
    pub fn filter_fields(&self, value: &Value) -> Vec<(String, String)> {
        let mut result = vec![];

        if let Some(obj) = value.as_object() {
            for (key, val) in obj {
                if !self.should_include(key) {
                    continue;
                }

                // Check if there's a formatted value annotation
                let formatted_key = format!("{}@OData.Community.Display.V1.FormattedValue", key);
                let display_value =
                    if let Some(formatted) = obj.get(&formatted_key).and_then(|v| v.as_str()) {
                        formatted.to_string()
                    } else {
                        match val {
                            Value::Null => "null".to_string(),
                            Value::Bool(b) => b.to_string(),
                            Value::Number(n) => n.to_string(),
                            Value::String(s) => s.clone(),
                            Value::Array(_) => "[array]".to_string(),
                            Value::Object(_) => "{object}".to_string(),
                        }
                    };

                result.push((key.clone(), display_value));
            }
        }

        // Sort by key
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }
}

/// Parsed condition JSON structure
#[derive(Debug, Clone)]
pub struct ConditionLogic {
    pub trigger_question_id: String,
    pub condition_operator: String,
    pub value: String,
    pub affected_questions: Vec<AffectedQuestion>,
}

#[derive(Debug, Clone)]
pub struct AffectedQuestion {
    pub question_id: String,
    pub visible: bool,
    pub required: bool,
}

impl ConditionLogic {
    /// Parse condition JSON string
    pub fn parse(json_str: &str) -> Result<Self, String> {
        let json: Value =
            serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;

        let trigger_question_id = json
            .get("questionId")
            .and_then(|v| v.as_str())
            .ok_or("Missing questionId")?
            .to_string();

        let condition_operator = json
            .get("condition")
            .and_then(|v| v.as_str())
            .unwrap_or("eq")
            .to_string();

        let value = json
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mut affected_questions = vec![];
        if let Some(questions) = json.get("questions").and_then(|v| v.as_array()) {
            for q in questions {
                if let Some(qid) = q.get("questionId").and_then(|v| v.as_str()) {
                    let visible = q.get("visible").and_then(|v| v.as_bool()).unwrap_or(true);
                    let required = q.get("required").and_then(|v| v.as_bool()).unwrap_or(false);

                    affected_questions.push(AffectedQuestion {
                        question_id: qid.to_string(),
                        visible,
                        required,
                    });
                }
            }
        }

        Ok(ConditionLogic {
            trigger_question_id,
            condition_operator,
            value,
            affected_questions,
        })
    }

    /// Total number of question IDs that need remapping
    pub fn reference_count(&self) -> usize {
        1 + self.affected_questions.len() // trigger + targets
    }

    /// Format operator for display
    pub fn format_operator(&self) -> &str {
        match self.condition_operator.as_str() {
            "eq" => "equals",
            "ne" => "not equals",
            "gt" => "greater than",
            "lt" => "less than",
            "contains" => "contains",
            _ => self.condition_operator.as_str(),
        }
    }
}
