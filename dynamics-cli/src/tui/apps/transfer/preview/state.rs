//! State and messages for the Transfer Preview app

use std::collections::HashMap;
use std::path::PathBuf;

use crossterm::event::KeyCode;

use crate::api::metadata::FieldMetadata;
use crate::transfer::{LookupBindingContext, ResolvedTransfer, RecordAction, Value};
use crate::tui::resource::Resource;
use crate::tui::widgets::{FileBrowserState, ListState, TextInputField, TextInputEvent};

/// Lookup metadata for a single entity
#[derive(Debug, Clone)]
pub struct EntityLookupMetadata {
    /// Field metadata for the target entity
    pub fields: Vec<FieldMetadata>,
    /// Entity set name (e.g., "accounts")
    pub entity_set: String,
}

/// Result of building lookup contexts for all entities
pub type LookupContextResult = Result<HashMap<String, LookupBindingContext>, String>;

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
    /// Loaded transfer config (kept for refresh)
    pub config: Option<crate::transfer::TransferConfig>,
    /// Number of pending fetch tasks
    pub pending_fetches: usize,
    /// Number of pending metadata fetch tasks
    pub pending_metadata_fetches: usize,
    /// Number of pending source metadata fetch tasks
    pub pending_source_metadata_fetches: usize,
    /// Number of pending target metadata fetch tasks
    pub pending_target_metadata_fetches: usize,
    /// Number of pending related entity metadata fetch tasks (for lookup traversals)
    pub pending_related_metadata_fetches: usize,
    /// Whether we're currently refreshing (vs initial load)
    pub is_refreshing: bool,
    /// Accumulated source records by entity name (kept for refresh comparison)
    pub source_data: std::collections::HashMap<String, Vec<serde_json::Value>>,
    /// Accumulated target records by entity name (kept for refresh comparison)
    pub target_data: std::collections::HashMap<String, Vec<serde_json::Value>>,
    /// Source entity field metadata (for knowing which fields are lookups when fetching)
    pub source_metadata: std::collections::HashMap<String, Vec<FieldMetadata>>,
    /// Target entity field metadata (for lookup binding)
    pub target_metadata: std::collections::HashMap<String, Vec<FieldMetadata>>,
    /// Entity set names (entity_logical_name -> entity_set_name)
    pub entity_set_map: std::collections::HashMap<String, String>,
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
    /// Horizontal scroll offset (column index) for wide tables
    pub horizontal_scroll: usize,
    /// Calculated column widths for each field (excluding fixed columns)
    pub column_widths: Vec<usize>,
    /// Terminal width for calculating visible columns
    pub terminal_width: usize,
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
    /// Export modal - file browser for directory selection
    pub export_file_browser: FileBrowserState,
    /// Export modal - filename input
    pub export_filename: TextInputField,
    /// Import modal - file browser for file selection
    pub import_file_browser: FileBrowserState,
    /// Import confirmation - pending edits to apply
    pub pending_import: Option<PendingImport>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            config_name: String::new(),
            source_env: String::new(),
            target_env: String::new(),
            config: None,
            pending_fetches: 0,
            pending_metadata_fetches: 0,
            pending_source_metadata_fetches: 0,
            pending_target_metadata_fetches: 0,
            pending_related_metadata_fetches: 0,
            is_refreshing: false,
            source_data: std::collections::HashMap::new(),
            target_data: std::collections::HashMap::new(),
            source_metadata: std::collections::HashMap::new(),
            target_metadata: std::collections::HashMap::new(),
            entity_set_map: std::collections::HashMap::new(),
            resolved: Resource::NotAsked,
            current_entity_idx: 0,
            filter: RecordFilter::All,
            search_field: TextInputField::new(),
            list_state: ListState::with_selection(),
            horizontal_scroll: 0,
            column_widths: Vec::new(),
            terminal_width: 120, // Reasonable default
            active_modal: None,
            viewport_height: 50, // Reasonable default until on_render provides actual value
            record_detail_state: None,
            bulk_action_scope: BulkActionScope::default(),
            bulk_action_selection: BulkAction::default(),
            export_file_browser: FileBrowserState::new(get_default_export_dir()),
            export_filename: TextInputField::new(),
            import_file_browser: FileBrowserState::new(get_default_export_dir()),
            pending_import: None,
        }
    }
}

/// Pending import waiting for user confirmation
#[derive(Clone)]
pub struct PendingImport {
    /// Path to the Excel file being imported
    pub path: String,
    /// Entity index this import applies to
    pub entity_idx: usize,
    /// Number of records that will be modified
    pub edit_count: usize,
    /// Source IDs of records with conflicts (dirty locally + changed in Excel)
    pub conflicts: Vec<uuid::Uuid>,
}

/// Get the default export directory (~/.config/dynamics-cli/exports/)
/// Creates the directory if it doesn't exist
fn get_default_export_dir() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("dynamics-cli")
        .join("exports");

    // Create directory if it doesn't exist
    if !config_dir.exists() {
        let _ = std::fs::create_dir_all(&config_dir);
    }

    config_dir
}

/// Filter for record actions in the table
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordFilter {
    #[default]
    All,
    Create,
    Update,
    NoChange,
    TargetOnly,
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
            RecordFilter::TargetOnly => "Target Only",
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
            RecordFilter::TargetOnly => action == RecordAction::TargetOnly,
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
            RecordFilter::TargetOnly,
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
            RecordFilter::NoChange => RecordFilter::TargetOnly,
            RecordFilter::TargetOnly => RecordFilter::Skip,
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
    /// Confirm sending to queue
    SendToQueue,
}

/// Messages for the Transfer Preview app
#[derive(Clone)]
pub enum Msg {
    // Data loading
    ConfigLoaded(Result<crate::transfer::TransferConfig, String>),
    SourceMetadataResult(Result<(String, Vec<FieldMetadata>), String>), // (entity_name, fields) - for source lookup detection
    TargetMetadataResult(Result<(String, Vec<FieldMetadata>, String), String>), // (entity_name, fields, entity_set_name) - for target lookup detection
    RelatedMetadataResult(Result<(String, Vec<FieldMetadata>), String>), // (entity_name, fields) - for lookup traversal entities
    FetchRelatedMetadata, // Triggered when related entity metadata needs to be fetched
    FetchRecords, // Triggered after both source and target metadata are loaded
    FetchResult(Result<(String, bool, Vec<serde_json::Value>), String>), // (entity_name, is_source, records)
    MetadataResult(Result<(String, Vec<FieldMetadata>, String), String>), // (entity_name, fields, entity_set_name)
    RunTransform, // Triggered after loading screen returns
    ResolvedLoaded(Result<ResolvedTransfer, String>),

    // Navigation
    ListEvent(crate::tui::widgets::ListEvent),
    ViewportHeightChanged(usize), // For virtual scrolling
    TerminalWidthChanged(usize),  // For horizontal scrolling
    ScrollLeft,
    ScrollRight,
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

    // Excel export
    ExportExcel,
    ExportFileNavigate(KeyCode),
    ExportFilenameChanged(TextInputEvent),
    ExportSetViewportHeight(usize),
    ConfirmExport,
    ExportCompleted(Result<String, String>),

    // Excel import
    ImportExcel,
    ImportFileNavigate(KeyCode),
    ImportSetViewportHeight(usize),
    ImportFileSelected(std::path::PathBuf),
    ImportPreviewLoaded(Result<PendingImport, String>),
    ConfirmImport,
    CancelImport,
    ImportCompleted(Result<crate::transfer::ResolvedEntity, String>),

    // Refresh
    Refresh,

    // Modal
    CloseModal,

    // Navigation
    Back,

    // Send to Queue
    OpenSendToQueue,
    ConfirmSendToQueue,
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

/// Column width configuration
pub const MIN_COLUMN_WIDTH: usize = 8;
pub const MAX_COLUMN_WIDTH: usize = 50; // Needs to fit lookup format: →entity(uuid) ~47 chars
/// Source ID display width (full UUID is 36 chars, show more)
pub const SOURCE_ID_WIDTH: usize = 36;
/// Fixed columns: scroll_indicator(2) + checkbox(4) + action(10) + separator(3) + source_id(36) + separator(3)
pub const FIXED_COLUMNS_WIDTH: usize = 58;

/// Calculate column widths based on field names and values (free function to avoid borrow issues)
pub fn calculate_column_widths(entity: &crate::transfer::ResolvedEntity) -> Vec<usize> {
    log::debug!("calculate_column_widths: starting for entity with {} fields, {} records",
        entity.field_names.len(), entity.records.len());

    let mut widths: Vec<usize> = Vec::with_capacity(entity.field_names.len());

    for field_name in &entity.field_names {
        // Start with header width
        let mut max_width = field_name.chars().count();

        // Check all record values for this field
        for record in &entity.records {
            if let Some(value) = record.fields.get(field_name) {
                let value_len = format_value_for_width(value).chars().count();
                max_width = max_width.max(value_len);
            }
        }

        // Clamp to min/max
        let width = max_width.clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH);
        widths.push(width);
    }

    log::debug!("calculate_column_widths: calculated {} widths: {:?}", widths.len(), widths);
    widths
}

impl State {
    /// Get current terminal width (with fallback)
    fn get_terminal_width() -> usize {
        crossterm::terminal::size()
            .map(|(w, _)| w as usize)
            .unwrap_or(120)
    }

    /// Get the number of visible columns based on terminal width and scroll offset
    pub fn visible_column_range(&self, field_count: usize) -> std::ops::Range<usize> {
        if self.column_widths.is_empty() {
            // Fallback: show first 5 columns
            return 0..field_count.min(5);
        }

        let terminal_width = Self::get_terminal_width();
        let mut available_width = terminal_width.saturating_sub(FIXED_COLUMNS_WIDTH);
        let start = self.horizontal_scroll.min(field_count.saturating_sub(1));
        let mut end = start;

        // Count how many columns fit
        for (i, &width) in self.column_widths.iter().enumerate().skip(start) {
            let col_width = width + 3; // +3 for " │ " separator
            if col_width > available_width {
                break;
            }
            available_width -= col_width;
            end = i + 1;
        }

        // Ensure at least one column is visible
        if end == start && start < field_count {
            end = start + 1;
        }

        start..end
    }

    /// Check if there are more columns to the left
    pub fn has_columns_left(&self) -> bool {
        self.horizontal_scroll > 0
    }

    /// Check if there are more columns to the right
    pub fn has_columns_right(&self, field_count: usize) -> bool {
        let range = self.visible_column_range(field_count);
        range.end < field_count
    }

    /// Scroll left by one column
    pub fn scroll_left(&mut self) {
        self.horizontal_scroll = self.horizontal_scroll.saturating_sub(1);
    }

    /// Scroll right by one column
    pub fn scroll_right(&mut self, field_count: usize) {
        if self.horizontal_scroll < field_count.saturating_sub(1) {
            self.horizontal_scroll += 1;
        }
    }
}

/// Format a Value for width calculation
/// Note: GUIDs may be displayed as lookups (→entity(uuid)) which is ~47 chars
fn format_value_for_width(value: &crate::transfer::Value) -> String {
    use crate::transfer::Value;
    match value {
        Value::Null => "(null)".to_string(),
        Value::String(s) => s.clone(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => format!("{:.2}", f),
        Value::Bool(b) => b.to_string(),
        Value::DateTime(dt) => dt.format("%Y-%m-%d").to_string(),
        // GUIDs displayed as lookups need ~47 chars: →entity(36-char-uuid)
        // Use placeholder that represents typical lookup display width
        Value::Guid(_) => "→entities(xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)".to_string(),
        Value::OptionSet(n) => n.to_string(),
        Value::Dynamic(dv) => format!("{:?}", dv),
    }
}
