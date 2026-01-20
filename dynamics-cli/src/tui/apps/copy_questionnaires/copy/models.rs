use super::domain::Questionnaire;
use crate::tui::{
    Resource,
    widgets::{TreeState, fields::TextInputField},
};
use serde_json::Value;

#[derive(Clone)]
pub struct State {
    pub questionnaire_id: String,
    pub questionnaire_name: String,
    pub questionnaire: Resource<Questionnaire>,
    pub tree_state: TreeState,
    pub copy_name_input: TextInputField,
    pub copy_code_input: TextInputField,
    pub validation_error: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            questionnaire_id: String::new(),
            questionnaire_name: String::new(),
            questionnaire: Resource::NotAsked,
            tree_state: TreeState::with_selection(),
            copy_name_input: TextInputField::new(),
            copy_code_input: TextInputField::new(),
            validation_error: None,
        }
    }
}

/// Complete snapshot of a questionnaire and all related entities
#[derive(Clone, Debug)]
pub struct QuestionnaireSnapshot {
    // Core entities
    pub questionnaire: Value,
    pub pages: Vec<Value>,
    pub page_lines: Vec<Value>,
    pub groups: Vec<Value>,
    pub group_lines: Vec<Value>,
    pub questions: Vec<Value>,
    pub template_lines: Vec<Value>,

    // Logic entities
    pub conditions: Vec<Value>,
    pub condition_actions: Vec<Value>,

    // N:N relationship entities (full entity records with names)
    pub categories: Vec<Value>,
    pub domains: Vec<Value>,
    pub funds: Vec<Value>,
    pub supports: Vec<Value>,
    pub types: Vec<Value>,
    pub subcategories: Vec<Value>,
    pub flemish_shares: Vec<Value>,
}

impl QuestionnaireSnapshot {
    /// Count total entities in the snapshot
    pub fn total_entities(&self) -> usize {
        1 + // questionnaire
        self.pages.len() +
        self.page_lines.len() +
        self.groups.len() +
        self.group_lines.len() +
        self.questions.len() +
        self.template_lines.len() +
        self.conditions.len() +
        self.condition_actions.len() +
        self.categories.len() +
        self.domains.len() +
        self.funds.len() +
        self.supports.len() +
        self.types.len() +
        self.subcategories.len() +
        self.flemish_shares.len()
    }
}

#[derive(Clone)]
pub enum Msg {
    QuestionnaireLoaded(Result<Questionnaire, String>),
    TreeEvent(crate::tui::widgets::TreeEvent),
    TreeNodeClicked(String), // Node clicked in tree
    ViewportHeight(usize),   // Called by renderer with actual area.height
    CopyNameInputEvent(crate::tui::widgets::TextInputEvent),
    CopyCodeInputEvent(crate::tui::widgets::TextInputEvent),
    Continue,
    Back,
}

pub struct CopyQuestionnaireParams {
    pub questionnaire_id: String,
    pub questionnaire_name: String,
}

impl Default for CopyQuestionnaireParams {
    fn default() -> Self {
        Self {
            questionnaire_id: String::new(),
            questionnaire_name: String::new(),
        }
    }
}
