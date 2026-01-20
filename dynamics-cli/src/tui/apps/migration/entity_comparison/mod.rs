mod app;
mod export;
mod fetch;
mod matching_adapter; // Adapter using service (Phase 1: excludes example support)
mod models;
mod tree_builder;
mod tree_items;
mod tree_sync;
mod update;
mod view;

pub use app::{EntityComparisonApp, EntityComparisonParams, State as EntityComparisonState};
pub use fetch::{
    FetchType, extract_entities, extract_relationships, fetch_example_pair_data, fetch_with_cache,
};
pub use models::*;
// Re-export matching types from service via adapter
pub use matching_adapter::{MatchInfo, MatchType};

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
    SourceViewportHeight(usize), // Called by renderer with actual area.height
    TargetViewportHeight(usize), // Called by renderer with actual area.height
    SourceTreeNodeClicked(String), // Node clicked in source tree
    TargetTreeNodeClicked(String), // Node clicked in target tree
    SourceTreeFocused,           // Source tree gained focus
    TargetTreeFocused,           // Target tree gained focus
    CreateManualMapping,         // Create mapping from selected source to selected target
    DeleteManualMapping,         // Delete mapping from selected field
    DeleteImportedMapping,       // Delete imported mapping from selected field
    CycleHideMode, // Cycle through hide modes (Off -> HideMatched -> HideIgnored -> HideBoth)
    ToggleSortMode, // Toggle between Alphabetical and MatchesFirst sorting
    ToggleSortDirection, // Toggle sort direction (Ascending <-> Descending)
    ToggleTechnicalNames, // Toggle between technical (logical) and display names
    ToggleMirrorMode, // Toggle mirror mode (Off -> Source -> Target -> Off)
    MappingsLoaded(
        std::collections::HashMap<String, Vec<String>>,
        std::collections::HashMap<String, Vec<String>>,
        std::collections::HashMap<String, Vec<String>>,
        Option<String>,
        Vec<ExamplePair>,
        std::collections::HashSet<String>,
        std::collections::HashSet<String>,
    ), // field_mappings, prefix_mappings, imported_mappings, import_source_file, example_pairs, ignored_items, negative_matches

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
    ExampleDataFetched(
        String,
        Result<(serde_json::Value, serde_json::Value), String>,
    ), // pair_id, (source_data, target_data)
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

    // Negative matches modal messages
    OpenNegativeMatchesModal,
    CloseNegativeMatchesModal,
    NegativeMatchesListNavigate(crossterm::event::KeyCode),
    NegativeMatchesListSelect(usize),
    DeleteNegativeMatch,
    AddNegativeMatchFromTree, // Context-aware 'd' key on prefix-matched field

    // Manual mappings modal messages
    OpenManualMappingsModal,
    CloseManualMappingsModal,
    ManualMappingsListNavigate(crossterm::event::KeyCode),
    ManualMappingsListSelect(usize),
    DeleteManualMappingFromModal,

    // Search messages
    ToggleSearch,     // Focus search (triggered by `/`)
    ToggleSearchMode, // Toggle between Unified and Independent modes (Ctrl+/)
    ToggleMatchMode,  // Toggle between Fuzzy and Substring match modes (f)
    SearchInputEvent(crate::tui::widgets::TextInputEvent), // Unified search
    SourceSearchEvent(crate::tui::widgets::TextInputEvent), // Independent: source search
    TargetSearchEvent(crate::tui::widgets::TextInputEvent), // Independent: target search
    SearchInputBlur,  // Unified search lost focus
    SourceSearchBlur, // Source search lost focus
    TargetSearchBlur, // Target search lost focus
    ClearSearch,      // Clear search (Esc when focused)
    SearchSelectFirstMatch, // Enter in search box

    // Type filter messages
    ToggleTypeFilterMode, // Toggle between Unified and Independent modes (Shift+T)
    CycleSourceTypeFilter, // Cycle through source types (t)
    CycleTargetTypeFilter, // Cycle through target types (T)

    // Export
    ExportToExcel,
    ExportUnmappedToCsv,

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
    SourceFields(String, Vec<crate::api::metadata::FieldMetadata>), // entity_name, fields
    SourceForms(String, Vec<crate::api::metadata::FormMetadata>),   // entity_name, forms
    SourceViews(String, Vec<crate::api::metadata::ViewMetadata>),   // entity_name, views
    TargetFields(String, Vec<crate::api::metadata::FieldMetadata>), // entity_name, fields
    TargetForms(String, Vec<crate::api::metadata::FormMetadata>),   // entity_name, forms
    TargetViews(String, Vec<crate::api::metadata::ViewMetadata>),   // entity_name, views
    ExampleData(String, serde_json::Value, serde_json::Value), // pair_id, source_data, target_data
}
