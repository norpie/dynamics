mod app;
mod models;
mod tree_items;
mod fetch;
mod tree_builder;
mod matching;
mod view;
mod tree_sync;
mod update;
mod export;

pub use app::{EntityComparisonApp, EntityComparisonParams, State as EntityComparisonState};
pub use models::*;
pub use fetch::{FetchType, fetch_with_cache, extract_relationships, extract_entities, fetch_example_pair_data};

// Internal message type for the app
#[derive(Clone)]
pub enum Msg {
    Back,
    ConfirmBack,
    CancelBack,
    SwitchTab(usize), // 1-indexed tab number
    ParallelDataLoaded(usize, Result<FetchedData, String>),
    Refresh,
    SourceTreeEvent(crate::tui::widgets::TreeEvent),
    TargetTreeEvent(crate::tui::widgets::TreeEvent),
    SourceViewportHeight(usize),  // Called by renderer with actual area.height
    TargetViewportHeight(usize),  // Called by renderer with actual area.height
    SourceTreeNodeClicked(String), // Node clicked in source tree
    TargetTreeNodeClicked(String), // Node clicked in target tree
    SourceTreeFocused,   // Source tree gained focus
    TargetTreeFocused,   // Target tree gained focus
    CreateManualMapping,  // Create mapping from selected source to selected target
    DeleteManualMapping,  // Delete mapping from selected field
    CycleHideMode,        // Cycle through hide modes (Off -> HideMatched -> HideIgnored -> HideBoth)
    ToggleSortMode,       // Toggle between Alphabetical and MatchesFirst sorting
    ToggleSortDirection,  // Toggle sort direction (Ascending <-> Descending)
    ToggleTechnicalNames, // Toggle between technical (logical) and display names
    ToggleMirrorMode,     // Toggle mirror mode (Off -> Source -> Target -> Off)
    MappingsLoaded(std::collections::HashMap<String, Vec<String>>, std::collections::HashMap<String, Vec<String>>, std::collections::HashMap<String, Vec<String>>, Option<String>, Vec<ExamplePair>, std::collections::HashSet<String>), // field_mappings, prefix_mappings, imported_mappings, import_source_file, example_pairs, ignored_items

    // Examples modal messages
    OpenExamplesModal,
    CloseExamplesModal,
    ExamplesListNavigate(crossterm::event::KeyCode),
    ExamplesListSelect(usize),
    SourceInputEvent(crate::tui::widgets::TextInputEvent),
    TargetInputEvent(crate::tui::widgets::TextInputEvent),
    LabelInputEvent(crate::tui::widgets::TextInputEvent),
    AddExamplePair,
    DeleteExamplePair,
    ExampleDataFetched(String, Result<(serde_json::Value, serde_json::Value), String>), // pair_id, (source_data, target_data)
    CycleExamplePair,
    ToggleExamples,

    // Prefix mappings modal messages
    OpenPrefixMappingsModal,
    ClosePrefixMappingsModal,
    PrefixMappingsListNavigate(crossterm::event::KeyCode),
    PrefixMappingsListSelect(usize),
    PrefixSourceInputEvent(crate::tui::widgets::TextInputEvent),
    PrefixTargetInputEvent(crate::tui::widgets::TextInputEvent),
    AddPrefixMapping,
    DeletePrefixMapping,

    // Manual mappings modal messages
    OpenManualMappingsModal,
    CloseManualMappingsModal,
    ManualMappingsListNavigate(crossterm::event::KeyCode),
    ManualMappingsListSelect(usize),
    DeleteManualMappingFromModal,

    // Search messages
    ToggleSearch,              // Focus search (triggered by `/`)
    ToggleSearchMode,          // Toggle between Unified and Independent modes (Ctrl+/)
    ToggleMatchMode,           // Toggle between Fuzzy and Substring match modes (f)
    SearchInputEvent(crate::tui::widgets::TextInputEvent),  // Unified search
    SourceSearchEvent(crate::tui::widgets::TextInputEvent), // Independent: source search
    TargetSearchEvent(crate::tui::widgets::TextInputEvent), // Independent: target search
    SearchInputBlur,           // Unified search lost focus
    SourceSearchBlur,          // Source search lost focus
    TargetSearchBlur,          // Target search lost focus
    ClearSearch,               // Clear search (Esc when focused)
    SearchSelectFirstMatch,    // Enter in search box

    // Export
    ExportToExcel,

    // Import from C# file
    OpenImportModal,
    CloseImportModal,
    ImportFileSelected(std::path::PathBuf),
    ImportMappingsLoaded(std::collections::HashMap<String, String>, String), // mappings, filename (for .cs files - gets converted to Vec in handler)
    ImportCsvLoaded(crate::csv_parser::CsvImportData, String), // csv_data, filename (for .csv files)
    ClearImportedMappings,
    ImportNavigate(crossterm::event::KeyCode),
    ImportSetViewportHeight(usize),
    CloseImportResultsModal,
    ImportResultsNavigate(crossterm::event::KeyCode),
    ImportResultsSelect(usize),
    ImportResultsSetViewportHeight(usize),

    // Ignore functionality
    IgnoreItem,
    OpenIgnoreModal,
    CloseIgnoreModal,
    IgnoreListNavigate(crossterm::event::KeyCode),
    IgnoreListSelect(usize),
    DeleteIgnoredItem,
    ClearAllIgnored,
    IgnoreSetViewportHeight(usize),
    IgnoredItemsSaved, // Dummy message after async save completes
}

#[derive(Clone)]
pub enum FetchedData {
    SourceFields(Vec<crate::api::metadata::FieldMetadata>),
    SourceForms(Vec<crate::api::metadata::FormMetadata>),
    SourceViews(Vec<crate::api::metadata::ViewMetadata>),
    TargetFields(Vec<crate::api::metadata::FieldMetadata>),
    TargetForms(Vec<crate::api::metadata::FormMetadata>),
    TargetViews(Vec<crate::api::metadata::ViewMetadata>),
    ExampleData(String, serde_json::Value, serde_json::Value), // pair_id, source_data, target_data
}
