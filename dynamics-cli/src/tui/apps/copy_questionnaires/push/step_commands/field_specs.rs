/// Static field specifications for questionnaire entity copying
///
/// This module defines which fields to copy for each entity type and how to transform them.
/// Uses an allowlist approach to avoid copying system fields and OData annotations.
use serde_json::Value;

/// Field type determines how the value is processed during copy
#[derive(Debug, Clone)]
pub enum FieldType {
    /// Direct value field - copy as-is
    Value,
    /// Lookup field - transform to @odata.bind format
    Lookup { target_entity: &'static str },
}

/// Field specification for entity copying
#[derive(Debug, Clone)]
pub struct FieldSpec {
    /// Name in raw JSON from API (e.g., "_nrq_questionnaireid_value" or "nrq_name")
    pub source_name: &'static str,
    /// Base field name for Dynamics (e.g., "nrq_questionnaireid" or "nrq_name")
    pub field_name: &'static str,
    /// How to process this field
    pub field_type: FieldType,
}

// Helper macros to reduce boilerplate
macro_rules! value_field {
    ($name:expr) => {
        FieldSpec {
            source_name: $name,
            field_name: $name,
            field_type: FieldType::Value,
        }
    };
}

macro_rules! lookup_field {
    ($source:expr, $field:expr, $target:expr) => {
        FieldSpec {
            source_name: $source,
            field_name: $field,
            field_type: FieldType::Lookup {
                target_entity: $target,
            },
        }
    };
}

/// Step 1: Questionnaire fields
pub const QUESTIONNAIRE_FIELDS: &[FieldSpec] = &[
    value_field!("nrq_name"),
    value_field!("nrq_code"),
    value_field!("nrq_type"),
    value_field!("nrq_publishdate"),
    value_field!("nrq_copypostfix"),
    value_field!("nrq_pullquestionstrigger"),
    lookup_field!("_nrq_deadline_value", "nrq_Deadline", "nrq_deadlines"),
    lookup_field!("_nrq_domain_value", "nrq_Domain", "nrq_domains"),
];

/// Step 2: Page fields
pub const PAGE_FIELDS: &[FieldSpec] = &[
    value_field!("nrq_name"),
    value_field!("nrq_description"),
    value_field!("nrq_pagecode"),
    value_field!("nrq_isdeliverable"),
    value_field!("nrq_schijf"),
    lookup_field!(
        "_nrq_relatedquestionnaire_value",
        "nrq_RelatedQuestionnaire",
        "nrq_questionnaires"
    ),
];

/// Step 3: Page Line fields
pub const PAGE_LINE_FIELDS: &[FieldSpec] = &[
    value_field!("nrq_name"),
    value_field!("nrq_code"),
    value_field!("nrq_order"),
    value_field!("nrq_createquestions"),
    value_field!("nrq_requeststatus"),
    value_field!("nrq_submittedrequeststatus"),
    value_field!("nrq_editablerequeststatusses"),
    value_field!("nrq_visibleinstatusses"),
    lookup_field!(
        "_nrq_questionnaireid_value",
        "nrq_QuestionnaireId",
        "nrq_questionnaires"
    ),
    lookup_field!(
        "_nrq_questionnairepageid_value",
        "nrq_QuestionnairepageId",
        "nrq_questionnairepages"
    ),
];

/// Step 4: Group fields
pub const GROUP_FIELDS: &[FieldSpec] = &[
    value_field!("nrq_name"),
    value_field!("nrq_code"),
    value_field!("nrq_description"),
    value_field!("nrq_enablemultipleentries"),
];

/// Step 5: Group Line fields
pub const GROUP_LINE_FIELDS: &[FieldSpec] = &[
    value_field!("nrq_name"),
    value_field!("nrq_code"),
    value_field!("nrq_order"),
    lookup_field!(
        "_nrq_questiongroupid_value",
        "nrq_QuestionGroupId",
        "nrq_questiongroups"
    ),
    lookup_field!(
        "_nrq_questionnairepageid_value",
        "nrq_QuestionnairePageId",
        "nrq_questionnairepages"
    ),
];

/// Step 6: Question fields (largest entity)
pub const QUESTION_FIELDS: &[FieldSpec] = &[
    // Value fields
    value_field!("nrq_name"),
    value_field!("nrq_questiontext"),
    value_field!("nrq_questiontype"),
    value_field!("nrq_required"),
    value_field!("nrq_ismultiselect"),
    value_field!("nrq_lookuptype"),
    value_field!("nrq_options"),
    value_field!("nrq_publicorprivatefile"),
    value_field!("nrq_regex"),
    value_field!("nrq_regexerrormessage"),
    value_field!("nrq_showasradio"),
    value_field!("nrq_targetentity"),
    value_field!("nrq_targetentityfield"),
    value_field!("nrq_targetfield"),
    value_field!("nrq_tooltip"),
    value_field!("nrq_uploadfolder"),
    value_field!("nrq_versionnumber"),
    // Lookup fields
    lookup_field!(
        "_nrq_questiongroupid_value",
        "nrq_QuestionGroupid",
        "nrq_questiongroups"
    ),
    lookup_field!(
        "_nrq_questionnaireid_value",
        "nrq_QuestionnaireId",
        "nrq_questionnaires"
    ),
    lookup_field!(
        "_nrq_questiontagid_value",
        "nrq_QuestionTagId",
        "nrq_questiontags"
    ),
    lookup_field!(
        "_nrq_questiontemplateid_value",
        "nrq_QuestionTemplateId",
        "nrq_questiontemplates"
    ),
    lookup_field!("_nrq_contactrole_value", "nrq_ContactRole", "nrq_roles"),
];

/// Step 7: Template Line fields
pub const TEMPLATE_LINE_FIELDS: &[FieldSpec] = &[
    value_field!("nrq_name"),
    value_field!("nrq_order"),
    lookup_field!(
        "_nrq_questiontemplateid_value",
        "nrq_QuestionTemplateId",
        "nrq_questiontemplates"
    ),
    lookup_field!(
        "_nrq_questiongroupid_value",
        "nrq_QuestionGroupid",
        "nrq_questiongroups"
    ),
];

/// Step 8: Condition fields
pub const CONDITION_FIELDS: &[FieldSpec] = &[
    value_field!("nrq_name"),
    value_field!("nrq_conditionjson"),
    value_field!("nrq_logicaloperator"),
    value_field!("nrq_value"),
    lookup_field!("_nrq_questionid_value", "nrq_questionid", "nrq_questions"),
    lookup_field!(
        "_nrq_questionnaireid_value",
        "nrq_questionnaireid",
        "nrq_questionnaires"
    ),
];

/// Step 9: Condition Action fields
pub const CONDITION_ACTION_FIELDS: &[FieldSpec] = &[
    value_field!("nrq_name"),
    value_field!("nrq_required"),
    value_field!("nrq_visible"),
    lookup_field!(
        "_nrq_questionconditionid_value",
        "nrq_questionconditionid",
        "nrq_questionconditions"
    ),
    lookup_field!("_nrq_questionid_value", "nrq_questionid", "nrq_questions"),
];
