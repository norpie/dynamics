use super::super::copy::domain::Questionnaire;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct State {
    pub questionnaire_id: String,
    pub copy_name: String,
    pub copy_code: String,
    pub questionnaire: Arc<Questionnaire>, // Shared, not cloned
    pub push_state: PushState,

    // Copy engine state (persists across steps)
    pub id_map: HashMap<String, String>,    // old_id -> new_id
    pub created_ids: Vec<(String, String)>, // (entity_set, id) for rollback
    pub classifications_associated: usize, // Count of classification associations (not in created_ids since they're not entities)
    pub start_time: Option<std::time::Instant>,

    // Cancellation flag
    pub cancel_requested: bool,

    // Undo confirmation flag
    pub show_undo_confirmation: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            questionnaire_id: String::new(),
            copy_name: String::new(),
            copy_code: String::new(),
            questionnaire: Arc::new(Questionnaire {
                id: String::new(),
                name: String::new(),
                raw: serde_json::Value::Null,
                pages: vec![],
                page_lines: vec![],
                group_lines: vec![],
                template_lines: vec![],
                conditions: vec![],
                classifications: super::super::copy::domain::Classifications {
                    categories: vec![],
                    domains: vec![],
                    funds: vec![],
                    supports: vec![],
                    types: vec![],
                    subcategories: vec![],
                    flemish_shares: vec![],
                },
            }),
            push_state: PushState::Confirming,
            id_map: HashMap::new(),
            created_ids: Vec::new(),
            classifications_associated: 0,
            start_time: None,
            cancel_requested: false,
            show_undo_confirmation: false,
        }
    }
}

/// Entity types for progress tracking
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub enum EntityType {
    Questionnaire,
    Pages,
    PageLines,
    Groups,
    GroupLines,
    Questions,
    TemplateLines,
    Conditions,
    ConditionActions,
    Classifications,
    ConditionStatusUpdates,
}

impl EntityType {
    /// Get a display name for this entity type
    pub fn display_name(&self) -> &'static str {
        match self {
            EntityType::Questionnaire => "Questionnaire",
            EntityType::Pages => "Pages",
            EntityType::PageLines => "Page Lines",
            EntityType::Groups => "Groups",
            EntityType::GroupLines => "Group Lines",
            EntityType::Questions => "Questions",
            EntityType::TemplateLines => "Template Lines",
            EntityType::Conditions => "Conditions",
            EntityType::ConditionActions => "Condition Actions",
            EntityType::Classifications => "Classifications",
            EntityType::ConditionStatusUpdates => "Condition Status Updates",
        }
    }
}

/// State machine for the push/copy process
#[derive(Clone)]
pub enum PushState {
    /// Screen 1: Confirmation - show summary and wait for user to start
    Confirming,

    /// Screen 2: Copy in progress - show real-time progress
    Copying(CopyProgress),

    /// Screen 3a: Success - show results
    Success(CopyResult),

    /// Screen 3b: Failure - show error and partial progress
    Failed(CopyError),
}

/// Progress tracking for the copy operation
#[derive(Clone)]
pub struct CopyProgress {
    pub phase: CopyPhase,
    pub step: usize, // 1-10 (10 steps total)

    // Per-entity counts (done, total) - indexed by EntityType
    pub entity_progress: HashMap<EntityType, (usize, usize)>,

    // Overall progress
    pub total_created: usize,
    pub total_entities: usize,
    pub started_at: Instant,
}

impl CopyProgress {
    pub fn new(questionnaire: &Questionnaire) -> Self {
        let pages_count = questionnaire.pages.len();
        let groups_count: usize = questionnaire.pages.iter().map(|p| p.groups.len()).sum();
        let questions_count: usize = questionnaire
            .pages
            .iter()
            .flat_map(|p| &p.groups)
            .map(|g| g.questions.len())
            .sum();
        let page_lines_count = questionnaire.page_lines.len();
        let group_lines_count = questionnaire.group_lines.len();
        let template_lines_count = questionnaire.template_lines.len();
        let conditions_count = questionnaire.conditions.len();
        let condition_actions_count: usize = questionnaire
            .conditions
            .iter()
            .map(|c| c.actions.len())
            .sum();
        let classifications_count = questionnaire.classifications.categories.len()
            + questionnaire.classifications.domains.len()
            + questionnaire.classifications.funds.len()
            + questionnaire.classifications.supports.len()
            + questionnaire.classifications.types.len()
            + questionnaire.classifications.subcategories.len()
            + questionnaire.classifications.flemish_shares.len();

        let total_entities = questionnaire.total_entities();

        // Initialize the HashMap with all entity types
        let mut entity_progress = HashMap::new();
        entity_progress.insert(EntityType::Questionnaire, (0, 1));
        entity_progress.insert(EntityType::Pages, (0, pages_count));
        entity_progress.insert(EntityType::PageLines, (0, page_lines_count));
        entity_progress.insert(EntityType::Groups, (0, groups_count));
        entity_progress.insert(EntityType::GroupLines, (0, group_lines_count));
        entity_progress.insert(EntityType::Questions, (0, questions_count));
        entity_progress.insert(EntityType::TemplateLines, (0, template_lines_count));
        entity_progress.insert(EntityType::Conditions, (0, conditions_count));
        entity_progress.insert(EntityType::ConditionActions, (0, condition_actions_count));
        entity_progress.insert(EntityType::Classifications, (0, classifications_count));
        entity_progress.insert(EntityType::ConditionStatusUpdates, (0, conditions_count));

        Self {
            phase: CopyPhase::CreatingQuestionnaire,
            step: 1,
            entity_progress,
            total_created: 0,
            total_entities,
            started_at: Instant::now(),
        }
    }

    /// Update progress for a specific entity type by marking it as complete
    pub fn complete(&mut self, entity_type: EntityType) {
        if let Some((done, total)) = self.entity_progress.get_mut(&entity_type) {
            let created = *total - *done;
            *done = *total;
            self.total_created += created;
        }
    }

    /// Get the progress counts for a specific entity type
    pub fn get(&self, entity_type: EntityType) -> (usize, usize) {
        self.entity_progress
            .get(&entity_type)
            .copied()
            .unwrap_or((0, 0))
    }

    /// Set progress for a specific entity type (mainly for testing or manual updates)
    pub fn set(&mut self, entity_type: EntityType, done: usize, total: usize) {
        self.entity_progress.insert(entity_type, (done, total));
    }

    /// Calculate overall percentage based on step progress (equal weight per step)
    pub fn percentage(&self) -> usize {
        // Each step ~9% (11 steps total)
        (self.step * 100) / 11
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }
}

/// Phases of the copy operation
#[derive(Clone, Debug, PartialEq)]
pub enum CopyPhase {
    CreatingQuestionnaire,    // Step 1
    CreatingPages,            // Step 2
    CreatingPageLines,        // Step 3
    CreatingGroups,           // Step 4
    CreatingGroupLines,       // Step 5
    CreatingQuestions,        // Step 6
    CreatingTemplateLines,    // Step 7
    CreatingConditions,       // Step 8
    CreatingConditionActions, // Step 9
    CreatingClassifications,  // Step 10
    PublishingConditions,     // Step 11
}

impl CopyPhase {
    pub fn step_number(&self) -> usize {
        match self {
            CopyPhase::CreatingQuestionnaire => 1,
            CopyPhase::CreatingPages => 2,
            CopyPhase::CreatingPageLines => 3,
            CopyPhase::CreatingGroups => 4,
            CopyPhase::CreatingGroupLines => 5,
            CopyPhase::CreatingQuestions => 6,
            CopyPhase::CreatingTemplateLines => 7,
            CopyPhase::CreatingConditions => 8,
            CopyPhase::CreatingConditionActions => 9,
            CopyPhase::CreatingClassifications => 10,
            CopyPhase::PublishingConditions => 11,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            CopyPhase::CreatingQuestionnaire => "Creating Questionnaire",
            CopyPhase::CreatingPages => "Creating Pages",
            CopyPhase::CreatingPageLines => "Creating Page Lines",
            CopyPhase::CreatingGroups => "Creating Groups",
            CopyPhase::CreatingGroupLines => "Creating Group Lines",
            CopyPhase::CreatingQuestions => "Creating Questions",
            CopyPhase::CreatingTemplateLines => "Creating Template Lines",
            CopyPhase::CreatingConditions => "Creating Conditions",
            CopyPhase::CreatingConditionActions => "Creating Condition Actions",
            CopyPhase::CreatingClassifications => "Creating Classifications",
            CopyPhase::PublishingConditions => "Restoring Condition Status",
        }
    }
}

/// Result of a successful copy
#[derive(Clone)]
pub struct CopyResult {
    pub new_questionnaire_id: String,
    pub new_questionnaire_name: String,
    pub entities_created: HashMap<String, usize>, // entity_type -> count
    pub total_entities: usize,
    pub duration: std::time::Duration,
}

/// Error during copy operation
#[derive(Clone)]
pub struct CopyError {
    pub phase: CopyPhase,
    pub step: usize,
    pub error_message: String,
    pub partial_counts: HashMap<String, usize>,
    pub rollback_complete: bool,
    pub orphaned_entities_csv: Option<String>, // Path to CSV if rollback failed
}

#[derive(Clone)]
pub enum Msg {
    // Screen 1: Confirmation
    StartCopy,
    Cancel,

    // Screen 2: Progress (per-step messages)
    Step1Complete(Result<String, CopyError>), // Returns new questionnaire ID
    Step2Complete(Result<(HashMap<String, String>, Vec<(String, String)>), CopyError>),
    Step3Complete(Result<(HashMap<String, String>, Vec<(String, String)>), CopyError>),
    Step4Complete(Result<(HashMap<String, String>, Vec<(String, String)>), CopyError>),
    Step5Complete(Result<(HashMap<String, String>, Vec<(String, String)>), CopyError>),
    Step6Complete(Result<(HashMap<String, String>, Vec<(String, String)>), CopyError>),
    Step7Complete(Result<(HashMap<String, String>, Vec<(String, String)>), CopyError>),
    Step8Complete(Result<(HashMap<String, String>, Vec<(String, String)>), CopyError>),
    Step9Complete(Result<(HashMap<String, String>, Vec<(String, String)>), CopyError>),
    Step10Complete(Result<(HashMap<String, String>, Vec<(String, String)>, usize), CopyError>),
    Step11Complete(Result<(HashMap<String, String>, Vec<(String, String)>), CopyError>),

    // Screen 3: Results
    CopySuccess(CopyResult),
    CopyFailed(CopyError),

    // Rollback
    RollbackComplete(Result<(), String>), // Ok if successful, Err(csv_path) if failed

    // Actions
    ViewCopy,
    CopyAnother,
    Retry,
    Done,
    Back,
    UndoCopy,    // Show undo confirmation
    ConfirmUndo, // Actually rollback after confirmation
    CancelUndo,  // Cancel the undo confirmation
    CancelCopy,  // Cancel during copy (triggers rollback)
}

pub struct PushQuestionnaireParams {
    pub questionnaire_id: String,
    pub copy_name: String,
    pub copy_code: String,
    pub questionnaire: Arc<Questionnaire>, // Shared, not cloned
}

impl Default for PushQuestionnaireParams {
    fn default() -> Self {
        Self {
            questionnaire_id: String::new(),
            copy_name: String::new(),
            copy_code: String::new(),
            questionnaire: Arc::new(Questionnaire {
                id: String::new(),
                name: String::new(),
                raw: serde_json::Value::Null,
                pages: vec![],
                page_lines: vec![],
                group_lines: vec![],
                template_lines: vec![],
                conditions: vec![],
                classifications: super::super::copy::domain::Classifications {
                    categories: vec![],
                    domains: vec![],
                    funds: vec![],
                    supports: vec![],
                    types: vec![],
                    subcategories: vec![],
                    flemish_shares: vec![],
                },
            }),
        }
    }
}
