//! State and messages for the Transfer Preview app

use crate::transfer::{ResolvedTransfer, RecordAction, Value};
use crate::tui::resource::Resource;
use crate::tui::widgets::{ListState, TextInputField, TextInputEvent};

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

/// State for editing a single field in the record details modal
#[derive(Clone)]
pub struct FieldEditState {
    /// Field name (logical name)
    pub field_name: String,
    /// Original value (for dirty checking and reset)
    pub original_value: Value,
    /// Current string value for editing
    pub input: TextInputField,
    /// Whether this field has been modified
    pub is_dirty: bool,
}

impl FieldEditState {
    /// Create a new field edit state from a field name and value
    pub fn new(field_name: String, value: &Value) -> Self {
        let display_value = format_value_for_edit(value);
        let mut input = TextInputField::new();
        input.set_value(display_value);
        Self {
            field_name,
            original_value: value.clone(),
            input,
            is_dirty: false,
        }
    }

    /// Check if the current value differs from the original
    pub fn update_dirty(&mut self) {
        let original_str = format_value_for_edit(&self.original_value);
        self.is_dirty = self.input.value() != original_str;
    }

    /// Reset to original value
    pub fn reset(&mut self) {
        let original_str = format_value_for_edit(&self.original_value);
        self.input.set_value(original_str);
        self.is_dirty = false;
    }

    /// Parse the current input back to a Value
    pub fn parse_value(&self) -> Value {
        parse_value_from_string(self.input.value(), &self.original_value)
    }
}

/// Format a Value for editing in a text input
fn format_value_for_edit(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::DateTime(dt) => dt.to_rfc3339(),
        Value::Guid(g) => g.to_string(),
        Value::OptionSet(n) => n.to_string(),
        Value::Dynamic(dv) => format!("{:?}", dv),
    }
}

/// Parse a string back into a Value, using original type as hint
fn parse_value_from_string(s: &str, original: &Value) -> Value {
    let trimmed = s.trim();

    // Empty string -> Null
    if trimmed.is_empty() {
        return Value::Null;
    }

    // Parse based on original type
    match original {
        Value::Null => {
            // Could be any type - try string
            Value::String(trimmed.to_string())
        }
        Value::String(_) => Value::String(trimmed.to_string()),
        Value::Int(_) => trimmed.parse::<i64>().map(Value::Int).unwrap_or(Value::String(trimmed.to_string())),
        Value::Float(_) => trimmed.parse::<f64>().map(Value::Float).unwrap_or(Value::String(trimmed.to_string())),
        Value::Bool(_) => match trimmed.to_lowercase().as_str() {
            "true" | "1" | "yes" => Value::Bool(true),
            "false" | "0" | "no" => Value::Bool(false),
            _ => Value::String(trimmed.to_string()),
        },
        Value::DateTime(_) => {
            chrono::DateTime::parse_from_rfc3339(trimmed)
                .map(|dt| Value::DateTime(dt.with_timezone(&chrono::Utc)))
                .unwrap_or(Value::String(trimmed.to_string()))
        }
        Value::Guid(_) => {
            uuid::Uuid::parse_str(trimmed)
                .map(Value::Guid)
                .unwrap_or(Value::String(trimmed.to_string()))
        }
        Value::OptionSet(_) => trimmed.parse::<i32>().map(Value::OptionSet).unwrap_or(Value::String(trimmed.to_string())),
        Value::Dynamic(_) => Value::String(trimmed.to_string()), // Can't edit dynamics
    }
}

/// State for the record details/edit modal
#[derive(Clone)]
pub struct RecordDetailState {
    /// Index into the filtered records list
    pub record_idx: usize,
    /// Whether we're in edit mode (true) or view mode (false)
    pub editing: bool,
    /// Whether we're actively editing the focused field's value
    pub editing_field: bool,
    /// Original action (for reset)
    pub original_action: RecordAction,
    /// Current selected action
    pub current_action: RecordAction,
    /// Editable field states
    pub fields: Vec<FieldEditState>,
    /// Currently focused field index (for keyboard navigation in edit mode)
    pub focused_field_idx: usize,
}

impl RecordDetailState {
    /// Create a new record detail state
    pub fn new(
        record_idx: usize,
        action: RecordAction,
        field_names: &[String],
        field_values: &std::collections::HashMap<String, Value>,
    ) -> Self {
        let fields = field_names
            .iter()
            .map(|name| {
                let value = field_values.get(name).cloned().unwrap_or(Value::Null);
                FieldEditState::new(name.clone(), &value)
            })
            .collect();

        Self {
            record_idx,
            editing: false,
            editing_field: false,
            original_action: action,
            current_action: action,
            fields,
            focused_field_idx: 0,
        }
    }

    /// Check if any changes have been made
    pub fn has_changes(&self) -> bool {
        self.current_action != self.original_action || self.fields.iter().any(|f| f.is_dirty)
    }

    /// Reset all changes
    pub fn reset_all(&mut self) {
        self.current_action = self.original_action;
        for field in &mut self.fields {
            field.reset();
        }
    }

    /// Get available actions for the action selector
    pub fn available_actions() -> &'static [RecordAction] {
        &[
            RecordAction::Create,
            RecordAction::Update,
            RecordAction::Skip,
            RecordAction::NoChange,
            RecordAction::Error,
        ]
    }

    /// Get index of current action in available_actions
    pub fn current_action_idx(&self) -> usize {
        Self::available_actions()
            .iter()
            .position(|a| *a == self.current_action)
            .unwrap_or(0)
    }
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
    /// Search input field
    pub search_field: TextInputField,
    /// List state for record table
    pub list_state: ListState,
    /// Horizontal scroll offset for wide tables
    pub horizontal_scroll: usize,
    /// Currently active modal
    pub active_modal: Option<PreviewModal>,
    /// Viewport height for virtual scrolling (updated by on_render)
    pub viewport_height: usize,
    /// State for the record details/edit modal (when open)
    pub record_detail_state: Option<RecordDetailState>,
    /// Bulk action modal - selected scope
    pub bulk_action_scope: BulkActionScope,
    /// Bulk action modal - selected action
    pub bulk_action_selection: BulkAction,
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
            search_field: TextInputField::new(),
            list_state: ListState::with_selection(),
            horizontal_scroll: 0,
            active_modal: None,
            viewport_height: 50, // Reasonable default until on_render provides actual value
            record_detail_state: None,
            bulk_action_scope: BulkActionScope::default(),
            bulk_action_selection: BulkAction::default(),
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

    // Record details modal
    ToggleEditMode,
    RecordDetailActionChanged(RecordAction),
    RecordDetailFieldNavigate(crossterm::event::KeyCode),
    StartFieldEdit,              // Enter on a field to start editing
    FocusedFieldInput(TextInputEvent), // Input events for the focused field
    FinishFieldEdit,             // Enter/Tab to finish editing field
    CancelFieldEdit,             // Esc while editing a field
    SaveRecordEdits,
    CancelRecordEdits,

    // Multi-selection
    ListMultiSelect(crate::tui::widgets::ListEvent),

    // Bulk actions
    OpenBulkActions,
    SetBulkActionScope(BulkActionScope),
    SetBulkAction(BulkAction),
    ConfirmBulkAction,

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BulkAction {
    #[default]
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

    /// Get all variants for iteration
    pub fn all_variants() -> &'static [BulkAction] {
        &[
            BulkAction::MarkSkip,
            BulkAction::UnmarkSkip,
            BulkAction::ResetToOriginal,
        ]
    }
}

/// Scope for bulk actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BulkActionScope {
    #[default]
    Filtered,   // Apply to all filtered records
    All,        // Apply to all records in entity
    Selected,   // Apply to multi-selected records only
}

impl BulkActionScope {
    pub fn display_name(&self) -> &'static str {
        match self {
            BulkActionScope::Filtered => "Filtered",
            BulkActionScope::All => "All",
            BulkActionScope::Selected => "Selected",
        }
    }
}
