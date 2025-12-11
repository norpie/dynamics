//! State and messages for the Transfer Preview app

use crate::transfer::{ResolvedTransfer, RecordAction};
use crate::tui::resource::Resource;
use crate::tui::widgets::ListState;

/// Parameters passed when starting the preview app
#[derive(Clone, Default)]
pub struct PreviewParams {
    /// Name of the transfer config
    pub config_name: String,
    /// Source environment name
    pub source_env: String,
    /// Target environment name
    pub target_env: String,
}

/// State for the Transfer Preview app
pub struct State {
    /// Config name being previewed
    pub config_name: String,
    /// Source environment name
    pub source_env: String,
    /// Target environment name
    pub target_env: String,
    /// Loaded transfer config (needed for transform step)
    pub config: Option<crate::transfer::TransferConfig>,
    /// Number of pending fetch tasks
    pub pending_fetches: usize,
    /// Accumulated source records by entity name
    pub source_data: std::collections::HashMap<String, Vec<serde_json::Value>>,
    /// Accumulated target records by entity name
    pub target_data: std::collections::HashMap<String, Vec<serde_json::Value>>,
    /// Resolved transfer data (loaded async)
    pub resolved: Resource<ResolvedTransfer>,
    /// Currently selected entity index
    pub current_entity_idx: usize,
    /// Filter for record actions
    pub filter: RecordFilter,
    /// Search query
    pub search_query: String,
    /// List state for record table
    pub list_state: ListState,
    /// Horizontal scroll offset for wide tables
    pub horizontal_scroll: usize,
    /// Currently active modal
    pub active_modal: Option<PreviewModal>,
    /// Viewport height for virtual scrolling (updated by on_render)
    pub viewport_height: usize,
}

impl Default for State {
    fn default() -> Self {
        Self {
            config_name: String::new(),
            source_env: String::new(),
            target_env: String::new(),
            config: None,
            pending_fetches: 0,
            source_data: std::collections::HashMap::new(),
            target_data: std::collections::HashMap::new(),
            resolved: Resource::NotAsked,
            current_entity_idx: 0,
            filter: RecordFilter::All,
            search_query: String::new(),
            list_state: ListState::with_selection(),
            horizontal_scroll: 0,
            active_modal: None,
            viewport_height: 50, // Reasonable default until on_render provides actual value
        }
    }
}

/// Filter for record actions in the table
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordFilter {
    #[default]
    All,
    Create,
    Update,
    NoChange,
    Skip,
    Error,
}

impl RecordFilter {
    /// Get display name for the filter
    pub fn display_name(&self) -> &'static str {
        match self {
            RecordFilter::All => "All",
            RecordFilter::Create => "Create",
            RecordFilter::Update => "Update",
            RecordFilter::NoChange => "No Change",
            RecordFilter::Skip => "Skip",
            RecordFilter::Error => "Error",
        }
    }

    /// Check if a record action matches this filter
    pub fn matches(&self, action: RecordAction) -> bool {
        match self {
            RecordFilter::All => true,
            RecordFilter::Create => action == RecordAction::Create,
            RecordFilter::Update => action == RecordAction::Update,
            RecordFilter::NoChange => action == RecordAction::NoChange,
            RecordFilter::Skip => action == RecordAction::Skip,
            RecordFilter::Error => action == RecordAction::Error,
        }
    }

    /// Get all filter variants
    pub fn all_variants() -> &'static [RecordFilter] {
        &[
            RecordFilter::All,
            RecordFilter::Create,
            RecordFilter::Update,
            RecordFilter::NoChange,
            RecordFilter::Skip,
            RecordFilter::Error,
        ]
    }

    /// Cycle to next filter
    pub fn next(&self) -> Self {
        match self {
            RecordFilter::All => RecordFilter::Create,
            RecordFilter::Create => RecordFilter::Update,
            RecordFilter::Update => RecordFilter::NoChange,
            RecordFilter::NoChange => RecordFilter::Skip,
            RecordFilter::Skip => RecordFilter::Error,
            RecordFilter::Error => RecordFilter::All,
        }
    }
}

/// Modal types for the preview app
#[derive(Debug, Clone)]
pub enum PreviewModal {
    /// View details of a single record
    RecordDetails { record_idx: usize },
    /// Edit a record's field values
    EditRecord { record_idx: usize },
    /// Bulk actions on filtered/selected records
    BulkActions,
    /// Export to Excel file browser
    ExportExcel,
    /// Import from Excel file browser
    ImportExcel,
    /// Confirm import with edit conflicts
    ImportConfirm { path: String, conflicts: Vec<String> },
}

/// Messages for the Transfer Preview app
#[derive(Clone)]
pub enum Msg {
    // Data loading
    ConfigLoaded(Result<crate::transfer::TransferConfig, String>),
    FetchResult(Result<(String, bool, Vec<serde_json::Value>), String>), // (entity_name, is_source, records)
    RunTransform, // Triggered after loading screen returns
    ResolvedLoaded(Result<ResolvedTransfer, String>),

    // Navigation
    ListEvent(crate::tui::widgets::ListEvent),
    ViewportHeightChanged(usize), // For virtual scrolling
    NextEntity,
    PrevEntity,
    SelectEntity(usize),

    // Filtering & search
    SetFilter(RecordFilter),
    CycleFilter,
    SearchChanged(crate::tui::widgets::TextInputEvent),

    // Record actions
    ToggleSkip,
    ViewDetails,
    EditRecord,
    SaveRecord,

    // Bulk actions
    OpenBulkActions,
    ApplyBulkAction(BulkAction),

    // Excel
    ExportExcel,
    ImportExcel,
    ExportCompleted(Result<String, String>),
    ImportCompleted(Result<ResolvedTransfer, String>),

    // Refresh
    Refresh,

    // Modal
    CloseModal,

    // Navigation
    Back,
    GoToExecute,
}

/// Bulk action types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkAction {
    MarkSkip,
    UnmarkSkip,
    ResetToOriginal,
}

impl BulkAction {
    pub fn display_name(&self) -> &'static str {
        match self {
            BulkAction::MarkSkip => "Mark as Skip",
            BulkAction::UnmarkSkip => "Unmark Skip (restore)",
            BulkAction::ResetToOriginal => "Reset to Original",
        }
    }
}
