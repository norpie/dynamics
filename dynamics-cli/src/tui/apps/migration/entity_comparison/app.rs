use crate::tui::{
    app::App,
    command::{AppId, Command},
    element::{Element, LayoutConstraint},
    subscription::Subscription,
    state::theme::Theme,
    renderer::LayeredView,
    Resource,
    widgets::TreeState,
    Alignment as LayerAlignment,
};
use crate::api::EntityMetadata;
use crossterm::event::KeyCode;
use ratatui::{
    prelude::Stylize,
    style::Style,
    text::{Line, Span},
};
use std::collections::{HashMap, HashSet};
use super::{Msg, Side, ExamplesState, ExamplePair, ActiveTab, FetchType, fetch_with_cache, extract_relationships, extract_entities, MatchInfo};
use super::matching::recompute_all_matches;
use super::tree_sync::{update_mirrored_selection, mirror_container_toggle};
use super::view::{render_main_layout, render_back_confirmation_modal, render_examples_modal};

/// Deduplicate example pairs based on (source_record_id, target_record_id)
/// Logs warnings for any duplicates found and keeps only the first occurrence
fn deduplicate_example_pairs(pairs: Vec<ExamplePair>) -> Vec<ExamplePair> {
    let mut seen = HashSet::new();
    let mut deduplicated = Vec::new();
    let mut duplicates_found = 0;

    for pair in pairs {
        let key = (pair.source_record_id.clone(), pair.target_record_id.clone());

        if seen.insert(key) {
            // First time seeing this pair
            deduplicated.push(pair);
        } else {
            // Duplicate found
            duplicates_found += 1;
            log::warn!(
                "Skipping duplicate example pair: source={}, target={}, id={}",
                pair.source_record_id,
                pair.target_record_id,
                pair.id
            );
        }
    }

    if duplicates_found > 0 {
        log::warn!(
            "Removed {} duplicate example pair(s), {} unique pairs remain",
            duplicates_found,
            deduplicated.len()
        );
    }

    deduplicated
}

pub struct EntityComparisonApp;

#[derive(Clone, Debug)]
pub struct ImportResults {
    pub filename: String,
    pub added: Vec<(String, String)>,      // (source_field, target_field)
    pub updated: Vec<(String, String)>,    // (source_field, target_field)
    pub removed: Vec<(String, String)>,    // (source_field, target_field)
    pub unparsed: Vec<String>,             // Lines that couldn't be parsed
}

#[derive(Clone)]
pub struct State {
    // Context
    pub(super) migration_name: String,
    pub(super) source_env: String,
    pub(super) target_env: String,
    pub(super) source_entity: String,
    pub(super) target_entity: String,

    // Active tab
    pub(super) active_tab: ActiveTab,

    // Metadata (from API)
    pub(super) source_metadata: Resource<EntityMetadata>,
    pub(super) target_metadata: Resource<EntityMetadata>,

    // Mapping state
    pub(super) field_mappings: HashMap<String, Vec<String>>,  // source -> targets (manual, 1-to-N support)
    pub(super) prefix_mappings: HashMap<String, Vec<String>>, // source_prefix -> target_prefixes (1-to-N support)
    pub(super) imported_mappings: HashMap<String, Vec<String>>, // source -> targets (from C# file, 1-to-N support)
    pub(super) import_source_file: Option<String>,       // filename of imported C# file
    pub(super) hide_mode: super::models::HideMode,
    pub(super) sort_mode: super::models::SortMode,
    pub(super) mirror_mode: super::models::MirrorMode,
    pub(super) show_technical_names: bool, // true = logical names, false = display names

    // Computed matches (cached)
    pub(super) field_matches: HashMap<String, MatchInfo>,        // source_field -> match_info
    pub(super) relationship_matches: HashMap<String, MatchInfo>, // source_relationship -> match_info
    pub(super) entity_matches: HashMap<String, MatchInfo>,       // source_entity -> match_info

    // Entity lists (extracted from relationships)
    pub(super) source_entities: Vec<(String, usize)>,  // (entity_name, usage_count)
    pub(super) target_entities: Vec<(String, usize)>,

    // Tree UI state - one tree state per tab per side
    pub(super) source_fields_tree: TreeState,
    pub(super) source_relationships_tree: TreeState,
    pub(super) source_views_tree: TreeState,
    pub(super) source_forms_tree: TreeState,
    pub(super) source_entities_tree: TreeState,
    pub(super) target_fields_tree: TreeState,
    pub(super) target_relationships_tree: TreeState,
    pub(super) target_views_tree: TreeState,
    pub(super) target_forms_tree: TreeState,
    pub(super) target_entities_tree: TreeState,
    pub(super) focused_side: Side,

    // Examples
    pub(super) examples: ExamplesState,

    // Examples modal state
    pub(super) show_examples_modal: bool,
    pub(super) examples_list_state: crate::tui::widgets::ListState,
    pub(super) examples_source_input: crate::tui::widgets::TextInputField,
    pub(super) examples_target_input: crate::tui::widgets::TextInputField,
    pub(super) examples_label_input: crate::tui::widgets::TextInputField,

    // Prefix mappings modal state
    pub(super) show_prefix_mappings_modal: bool,
    pub(super) prefix_mappings_list_state: crate::tui::widgets::ListState,
    pub(super) prefix_source_input: crate::tui::widgets::TextInputField,
    pub(super) prefix_target_input: crate::tui::widgets::TextInputField,

    // Manual mappings modal state
    pub(super) show_manual_mappings_modal: bool,
    pub(super) manual_mappings_list_state: crate::tui::widgets::ListState,

    // Import modal state
    pub(super) show_import_modal: bool,
    pub(super) import_file_browser: crate::tui::widgets::FileBrowserState,
    pub(super) show_import_results_modal: bool,
    pub(super) import_results: Option<ImportResults>,
    pub(super) import_results_list: crate::tui::widgets::ListState,

    // Ignore state
    pub(super) ignored_items: std::collections::HashSet<String>,
    pub(super) show_ignore_modal: bool,
    pub(super) ignore_list_state: crate::tui::widgets::ListState,

    // Search state
    pub(super) search_mode: super::models::SearchMode,
    pub(super) match_mode: super::models::MatchMode,
    pub(super) unified_search: crate::tui::widgets::TextInputField,
    pub(super) source_search: crate::tui::widgets::TextInputField,
    pub(super) target_search: crate::tui::widgets::TextInputField,

    // Modal state
    pub(super) show_back_confirmation: bool,
}

pub struct EntityComparisonParams {
    pub migration_name: String,
    pub source_env: String,
    pub target_env: String,
    pub source_entity: String,
    pub target_entity: String,
}

impl Default for EntityComparisonParams {
    fn default() -> Self {
        Self {
            migration_name: String::new(),
            source_env: String::new(),
            target_env: String::new(),
            source_entity: String::new(),
            target_entity: String::new(),
        }
    }
}

impl crate::tui::AppState for State {}

impl Default for State {
    fn default() -> Self {
        Self {
            migration_name: String::new(),
            source_env: String::new(),
            target_env: String::new(),
            source_entity: String::new(),
            target_entity: String::new(),
            active_tab: ActiveTab::default(),
            source_metadata: Resource::NotAsked,
            target_metadata: Resource::NotAsked,
            field_mappings: HashMap::new(),
            prefix_mappings: HashMap::new(),
            imported_mappings: HashMap::new(),
            import_source_file: None,
            hide_mode: super::models::HideMode::default(),
            sort_mode: super::models::SortMode::default(),
            mirror_mode: super::models::MirrorMode::default(),
            show_technical_names: true,
            field_matches: HashMap::new(),
            relationship_matches: HashMap::new(),
            entity_matches: HashMap::new(),
            source_entities: Vec::new(),
            target_entities: Vec::new(),
            source_fields_tree: TreeState::with_selection(),
            source_relationships_tree: TreeState::with_selection(),
            source_views_tree: TreeState::with_selection(),
            source_forms_tree: TreeState::with_selection(),
            source_entities_tree: TreeState::with_selection(),
            target_fields_tree: TreeState::with_selection(),
            target_relationships_tree: TreeState::with_selection(),
            target_views_tree: TreeState::with_selection(),
            target_forms_tree: TreeState::with_selection(),
            target_entities_tree: TreeState::with_selection(),
            focused_side: Side::Source,
            examples: ExamplesState::new(),
            show_examples_modal: false,
            examples_list_state: crate::tui::widgets::ListState::new(),
            examples_source_input: crate::tui::widgets::TextInputField::new(),
            examples_target_input: crate::tui::widgets::TextInputField::new(),
            examples_label_input: crate::tui::widgets::TextInputField::new(),
            show_prefix_mappings_modal: false,
            prefix_mappings_list_state: crate::tui::widgets::ListState::new(),
            prefix_source_input: crate::tui::widgets::TextInputField::new(),
            prefix_target_input: crate::tui::widgets::TextInputField::new(),
            show_manual_mappings_modal: false,
            manual_mappings_list_state: crate::tui::widgets::ListState::new(),
            show_import_modal: false,
            import_file_browser: crate::tui::widgets::FileBrowserState::new(
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
            ),
            show_import_results_modal: false,
            import_results: None,
            import_results_list: crate::tui::widgets::ListState::new(),
            ignored_items: std::collections::HashSet::new(),
            show_ignore_modal: false,
            ignore_list_state: crate::tui::widgets::ListState::new(),
            search_mode: super::models::SearchMode::default(),
            match_mode: super::models::MatchMode::default(),
            unified_search: crate::tui::widgets::TextInputField::new(),
            source_search: crate::tui::widgets::TextInputField::new(),
            target_search: crate::tui::widgets::TextInputField::new(),
            show_back_confirmation: false,
        }
    }
}

impl State {
    /// Get the appropriate source tree state for the active tab
    pub(super) fn source_tree_for_tab(&mut self) -> &mut TreeState {
        match self.active_tab {
            ActiveTab::Fields => &mut self.source_fields_tree,
            ActiveTab::Relationships => &mut self.source_relationships_tree,
            ActiveTab::Views => &mut self.source_views_tree,
            ActiveTab::Forms => &mut self.source_forms_tree,
            ActiveTab::Entities => &mut self.source_entities_tree,
        }
    }

    /// Get the appropriate target tree state for the active tab
    pub(super) fn target_tree_for_tab(&mut self) -> &mut TreeState {
        match self.active_tab {
            ActiveTab::Fields => &mut self.target_fields_tree,
            ActiveTab::Relationships => &mut self.target_relationships_tree,
            ActiveTab::Views => &mut self.target_views_tree,
            ActiveTab::Forms => &mut self.target_forms_tree,
            ActiveTab::Entities => &mut self.target_entities_tree,
        }
    }
}

impl App for EntityComparisonApp {
    type State = State;
    type Msg = Msg;
    type InitParams = EntityComparisonParams;

    fn init(params: EntityComparisonParams) -> (State, Command<Msg>) {
        let mut state = State {
            migration_name: params.migration_name.clone(),
            source_env: params.source_env.clone(),
            target_env: params.target_env.clone(),
            source_entity: params.source_entity.clone(),
            target_entity: params.target_entity.clone(),
            active_tab: ActiveTab::default(),
            source_metadata: Resource::Loading,
            target_metadata: Resource::Loading,
            field_mappings: HashMap::new(),
            prefix_mappings: HashMap::new(),
            imported_mappings: HashMap::new(),
            import_source_file: None,
            hide_mode: super::models::HideMode::default(),
            sort_mode: super::models::SortMode::default(),
            mirror_mode: super::models::MirrorMode::default(),
            show_technical_names: true, // Default to showing logical/technical names
            field_matches: HashMap::new(),
            relationship_matches: HashMap::new(),
            entity_matches: HashMap::new(),
            source_entities: Vec::new(),
            target_entities: Vec::new(),
            source_fields_tree: TreeState::with_selection(),
            source_relationships_tree: TreeState::with_selection(),
            source_views_tree: TreeState::with_selection(),
            source_forms_tree: TreeState::with_selection(),
            source_entities_tree: TreeState::with_selection(),
            target_fields_tree: TreeState::with_selection(),
            target_relationships_tree: TreeState::with_selection(),
            target_views_tree: TreeState::with_selection(),
            target_forms_tree: TreeState::with_selection(),
            target_entities_tree: TreeState::with_selection(),
            focused_side: Side::Source,
            examples: ExamplesState::new(),
            show_examples_modal: false,
            examples_list_state: crate::tui::widgets::ListState::new(),
            examples_source_input: crate::tui::widgets::TextInputField::new(),
            examples_target_input: crate::tui::widgets::TextInputField::new(),
            examples_label_input: crate::tui::widgets::TextInputField::new(),
            show_prefix_mappings_modal: false,
            prefix_mappings_list_state: crate::tui::widgets::ListState::new(),
            prefix_source_input: crate::tui::widgets::TextInputField::new(),
            prefix_target_input: crate::tui::widgets::TextInputField::new(),
            show_manual_mappings_modal: false,
            manual_mappings_list_state: crate::tui::widgets::ListState::new(),
            show_import_modal: false,
            import_file_browser: crate::tui::widgets::FileBrowserState::new(
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
            ),
            show_import_results_modal: false,
            import_results: None,
            import_results_list: crate::tui::widgets::ListState::new(),
            ignored_items: std::collections::HashSet::new(),
            show_ignore_modal: false,
            ignore_list_state: crate::tui::widgets::ListState::new(),
            search_mode: super::models::SearchMode::default(),
            match_mode: super::models::MatchMode::default(),
            unified_search: crate::tui::widgets::TextInputField::new(),
            source_search: crate::tui::widgets::TextInputField::new(),
            target_search: crate::tui::widgets::TextInputField::new(),
            show_back_confirmation: false,
        };

        // First, load mappings to know which example pairs to fetch
        let init_cmd = Command::perform({
            let source_entity = params.source_entity.clone();
            let target_entity = params.target_entity.clone();
            async move {
                let config = crate::global_config();
                let field_mappings = config.get_field_mappings(&source_entity, &target_entity).await
                    .unwrap_or_else(|e| {
                        log::error!("Failed to load field mappings: {}", e);
                        HashMap::new()
                    });
                let prefix_mappings = config.get_prefix_mappings(&source_entity, &target_entity).await
                    .unwrap_or_else(|e| {
                        log::error!("Failed to load prefix mappings: {}", e);
                        HashMap::new()
                    });
                let (imported_mappings, import_source_file) = config.get_imported_mappings(&source_entity, &target_entity).await
                    .unwrap_or_else(|e| {
                        log::error!("Failed to load imported mappings: {}", e);
                        (HashMap::new(), None)
                    });
                let example_pairs_raw = config.get_example_pairs(&source_entity, &target_entity).await
                    .unwrap_or_else(|e| {
                        log::error!("Failed to load example pairs: {}", e);
                        Vec::new()
                    });

                // Deduplicate example pairs to prevent issues
                let example_pairs = deduplicate_example_pairs(example_pairs_raw);

                let ignored_items = config.get_ignored_items(&source_entity, &target_entity).await
                    .unwrap_or_else(|e| {
                        log::error!("Failed to load ignored items: {}", e);
                        std::collections::HashSet::new()
                    });
                (field_mappings, prefix_mappings, imported_mappings, import_source_file, example_pairs, ignored_items)
            }
        }, |(field_mappings, prefix_mappings, imported_mappings, import_source_file, example_pairs, ignored_items)| {
            Msg::MappingsLoaded(field_mappings, prefix_mappings, imported_mappings, import_source_file, example_pairs, ignored_items)
        });

        (state, init_cmd)
    }

    fn update(state: &mut Self::State, msg: Self::Msg) -> Command<Self::Msg> {
        super::update::update(state, msg)
    }

    fn view(state: &mut Self::State) -> LayeredView<Self::Msg> {
        let main_ui = render_main_layout(state);
        let mut view = LayeredView::new(main_ui);

        if state.show_back_confirmation {
            view = view.with_app_modal(render_back_confirmation_modal(), LayerAlignment::Center);
        }

        if state.show_examples_modal {
            view = view.with_app_modal(render_examples_modal(state), LayerAlignment::Center);
        }

        if state.show_prefix_mappings_modal {
            view = view.with_app_modal(super::view::render_prefix_mappings_modal(state), LayerAlignment::Center);
        }

        if state.show_manual_mappings_modal {
            view = view.with_app_modal(super::view::render_manual_mappings_modal(state), LayerAlignment::Center);
        }

        if state.show_import_modal {
            view = view.with_app_modal(super::view::render_import_modal(state), LayerAlignment::Center);
        }

        if state.show_import_results_modal {
            view = view.with_app_modal(super::view::render_import_results_modal(state), LayerAlignment::Center);
        }

        if state.show_ignore_modal {
            view = view.with_app_modal(super::view::render_ignore_modal(state), LayerAlignment::Center);
        }

        view
    }

    fn subscriptions(state: &Self::State) -> Vec<Subscription<Self::Msg>> {
        let config = crate::global_runtime_config();

        let mut subs = vec![
            Subscription::keyboard(KeyCode::Esc, "Back to comparison list", Msg::Back),
            Subscription::keyboard(config.get_keybind("entity_comparison.back"), "Back to comparison list", Msg::Back),

            // Tab switching
            Subscription::keyboard(config.get_keybind("entity_comparison.tab_fields"), "Switch to Fields", Msg::SwitchTab(1)),
            Subscription::keyboard(config.get_keybind("entity_comparison.tab_relationships"), "Switch to Relationships", Msg::SwitchTab(2)),
            Subscription::keyboard(config.get_keybind("entity_comparison.tab_views"), "Switch to Views", Msg::SwitchTab(3)),
            Subscription::keyboard(config.get_keybind("entity_comparison.tab_forms"), "Switch to Forms", Msg::SwitchTab(4)),
            Subscription::keyboard(config.get_keybind("entity_comparison.tab_entities"), "Switch to Entities", Msg::SwitchTab(5)),


            // Refresh metadata
            Subscription::keyboard(config.get_keybind("entity_comparison.refresh"), "Refresh metadata", Msg::Refresh),

            // Manual mapping actions (supports 1-to-N and N-to-1 via multi-select)
            Subscription::keyboard(config.get_keybind("entity_comparison.create_mapping"), "Create manual mapping (multi-select supported)", Msg::CreateManualMapping),
            Subscription::keyboard(config.get_keybind("entity_comparison.delete_mapping"), "Delete manual mapping", Msg::DeleteManualMapping),

            // Cycle hide mode
            Subscription::keyboard(config.get_keybind("entity_comparison.toggle_hide_matched"), "Cycle hide mode", Msg::CycleHideMode),

            // Sort mode toggle
            Subscription::keyboard(config.get_keybind("entity_comparison.toggle_sort"), "Toggle sort mode", Msg::ToggleSortMode),

            // Technical/display name toggle
            Subscription::keyboard(config.get_keybind("entity_comparison.toggle_technical_names"), "Toggle technical names", Msg::ToggleTechnicalNames),

            // Mirror mode toggle
            Subscription::keyboard(config.get_keybind("entity_comparison.toggle_mirror"), "Toggle mirror mode", Msg::ToggleMirrorMode),

            // Examples management
            Subscription::keyboard(config.get_keybind("entity_comparison.cycle_example"), "Cycle example pairs", Msg::CycleExamplePair),
            Subscription::keyboard(config.get_keybind("entity_comparison.open_examples"), "Open examples modal", Msg::OpenExamplesModal),

            // Prefix mappings
            Subscription::keyboard(config.get_keybind("entity_comparison.open_prefix_mappings"), "Open prefix mappings modal", Msg::OpenPrefixMappingsModal),

            // Manual mappings
            Subscription::keyboard(config.get_keybind("entity_comparison.open_manual_mappings"), "View manual mappings modal", Msg::OpenManualMappingsModal),

            // Import from C# file
            Subscription::keyboard(config.get_keybind("entity_comparison.import_cs"), "Import from C# file", Msg::OpenImportModal),

            // Ignore functionality
            Subscription::keyboard(config.get_keybind("entity_comparison.ignore_item"), "Ignore item", Msg::IgnoreItem),
            Subscription::keyboard(config.get_keybind("entity_comparison.ignore_manager"), "Ignore manager", Msg::OpenIgnoreModal),

            // Export
            Subscription::keyboard(config.get_keybind("entity_comparison.export"), "Export to Excel", Msg::ExportToExcel),
        ];

        // Multi-selection shortcuts (active when no modal is open and search is not focused)
        // Only apply to source tree for now
        let any_modal_open = state.show_back_confirmation
            || state.show_examples_modal
            || state.show_prefix_mappings_modal
            || state.show_manual_mappings_modal
            || state.show_import_modal
            || state.show_import_results_modal
            || state.show_ignore_modal;

        if !any_modal_open {
            use crate::tui::widgets::TreeEvent;
            use crossterm::event::KeyCode;

            let search_value = match state.search_mode {
                super::models::SearchMode::Unified => state.unified_search.value(),
                super::models::SearchMode::Independent => {
                    &format!("source:'{}', target:'{}'", state.source_search.value(), state.target_search.value())
                }
            };
            log::debug!("✓ Registering multi-select shortcuts (search_value='{}')", search_value);

            // Multi-select shortcuts - route to focused tree based on state.focused_side
            match state.focused_side {
                Side::Source => {
                    // Space: Toggle multi-select on current node
                    subs.push(Subscription::keyboard(
                        KeyCode::Char(' '),
                        "Toggle multi-select",
                        Msg::SourceTreeEvent(TreeEvent::ToggleMultiSelect)
                    ));

                    // Ctrl+D: Clear multi-selection
                    subs.push(Subscription::ctrl_key(
                        KeyCode::Char('d'),
                        "Clear selection",
                        Msg::SourceTreeEvent(TreeEvent::ClearMultiSelection)
                    ));

                    // Shift+Up: Extend selection up
                    subs.push(Subscription::shift_key(
                        KeyCode::Up,
                        "Extend selection up",
                        Msg::SourceTreeEvent(TreeEvent::ExtendSelectionUp)
                    ));

                    // Shift+Down: Extend selection down
                    subs.push(Subscription::shift_key(
                        KeyCode::Down,
                        "Extend selection down",
                        Msg::SourceTreeEvent(TreeEvent::ExtendSelectionDown)
                    ));
                }
                Side::Target => {
                    // Space: Toggle multi-select on current node
                    subs.push(Subscription::keyboard(
                        KeyCode::Char(' '),
                        "Toggle multi-select",
                        Msg::TargetTreeEvent(TreeEvent::ToggleMultiSelect)
                    ));

                    // Ctrl+D: Clear multi-selection
                    subs.push(Subscription::ctrl_key(
                        KeyCode::Char('d'),
                        "Clear selection",
                        Msg::TargetTreeEvent(TreeEvent::ClearMultiSelection)
                    ));

                    // Shift+Up: Extend selection up
                    subs.push(Subscription::shift_key(
                        KeyCode::Up,
                        "Extend selection up",
                        Msg::TargetTreeEvent(TreeEvent::ExtendSelectionUp)
                    ));

                    // Shift+Down: Extend selection down
                    subs.push(Subscription::shift_key(
                        KeyCode::Down,
                        "Extend selection down",
                        Msg::TargetTreeEvent(TreeEvent::ExtendSelectionDown)
                    ));
                }
            }
        } else {
            let search_value = match state.search_mode {
                super::models::SearchMode::Unified => state.unified_search.value(),
                super::models::SearchMode::Independent => {
                    &format!("source:'{}', target:'{}'", state.source_search.value(), state.target_search.value())
                }
            };
            log::debug!("✗ Skipping multi-select shortcuts (any_modal_open={}, search_value='{}')",
                       any_modal_open, search_value);
        }

        // Search - add global `/` key unless a modal is open
        let any_modal_open = state.show_back_confirmation
            || state.show_examples_modal
            || state.show_prefix_mappings_modal
            || state.show_manual_mappings_modal
            || state.show_import_modal
            || state.show_import_results_modal
            || state.show_ignore_modal;

        if !any_modal_open {
            subs.push(Subscription::keyboard(KeyCode::Char('/'), "Focus search", Msg::ToggleSearch));
            subs.push(Subscription::keyboard(KeyCode::Char('?'), "Toggle search mode", Msg::ToggleSearchMode));
        }

        // Match mode toggle - always available
        subs.push(Subscription::keyboard(KeyCode::Char('f'), "Toggle match mode", Msg::ToggleMatchMode));

        // When showing confirmation modal, add y/n hotkeys
        if state.show_back_confirmation {
            subs.push(Subscription::keyboard(KeyCode::Char('y'), "Confirm", Msg::ConfirmBack));
            subs.push(Subscription::keyboard(KeyCode::Char('Y'), "Confirm", Msg::ConfirmBack));
            subs.push(Subscription::keyboard(KeyCode::Char('n'), "Cancel", Msg::CancelBack));
            subs.push(Subscription::keyboard(KeyCode::Char('N'), "Cancel", Msg::CancelBack));
            subs.push(Subscription::keyboard(KeyCode::Enter, "Confirm", Msg::ConfirmBack));
        }

        // When showing examples modal, add hotkeys
        if state.show_examples_modal {
            subs.push(Subscription::keyboard(KeyCode::Char('a'), "Add example pair", Msg::AddExamplePair));
            subs.push(Subscription::keyboard(KeyCode::Char('d'), "Delete example pair", Msg::DeleteExamplePair));
            subs.push(Subscription::keyboard(KeyCode::Char('c'), "Close modal", Msg::CloseExamplesModal));
            subs.push(Subscription::keyboard(KeyCode::Esc, "Close modal", Msg::CloseExamplesModal));
        }

        // When showing prefix mappings modal, add hotkeys
        if state.show_prefix_mappings_modal {
            subs.push(Subscription::keyboard(KeyCode::Char('a'), "Add prefix mapping", Msg::AddPrefixMapping));
            subs.push(Subscription::keyboard(KeyCode::Char('d'), "Delete prefix mapping", Msg::DeletePrefixMapping));
            subs.push(Subscription::keyboard(KeyCode::Char('c'), "Close modal", Msg::ClosePrefixMappingsModal));
            subs.push(Subscription::keyboard(KeyCode::Esc, "Close modal", Msg::ClosePrefixMappingsModal));
        }

        // When showing manual mappings modal, add hotkeys
        if state.show_manual_mappings_modal {
            subs.push(Subscription::keyboard(KeyCode::Char('d'), "Delete manual mapping", Msg::DeleteManualMappingFromModal));
            subs.push(Subscription::keyboard(KeyCode::Char('c'), "Close modal", Msg::CloseManualMappingsModal));
            subs.push(Subscription::keyboard(KeyCode::Esc, "Close modal", Msg::CloseManualMappingsModal));
        }

        // When showing import modal, add hotkeys
        if state.show_import_modal {
            subs.push(Subscription::keyboard(KeyCode::Char('c'), "Close modal", Msg::CloseImportModal));
            subs.push(Subscription::keyboard(KeyCode::Esc, "Close modal", Msg::CloseImportModal));
        }

        // When showing import results modal, add hotkeys
        if state.show_import_results_modal {
            subs.push(Subscription::keyboard(KeyCode::Up, "Navigate up", Msg::ImportResultsNavigate(KeyCode::Up)));
            subs.push(Subscription::keyboard(KeyCode::Down, "Navigate down", Msg::ImportResultsNavigate(KeyCode::Down)));
            subs.push(Subscription::keyboard(KeyCode::Char('c'), "Clear imports", Msg::ClearImportedMappings));
            subs.push(Subscription::keyboard(KeyCode::Esc, "Close modal", Msg::CloseImportResultsModal));
        }

        // When showing ignore modal, add hotkeys
        if state.show_ignore_modal {
            subs.push(Subscription::keyboard(KeyCode::Up, "Navigate up", Msg::IgnoreListNavigate(KeyCode::Up)));
            subs.push(Subscription::keyboard(KeyCode::Down, "Navigate down", Msg::IgnoreListNavigate(KeyCode::Down)));
            subs.push(Subscription::keyboard(KeyCode::Char('d'), "Delete ignored item", Msg::DeleteIgnoredItem));
            subs.push(Subscription::keyboard(KeyCode::Char('C'), "Clear all ignored", Msg::ClearAllIgnored));
            subs.push(Subscription::keyboard(KeyCode::Esc, "Close modal", Msg::CloseIgnoreModal));
        }

        subs
    }

    fn title() -> &'static str {
        "Entity Comparison"
    }

    fn status(state: &Self::State) -> Option<Line<'static>> {
        // Build tab indicator with active tab highlighted
        let theme = &crate::global_runtime_config().theme;
        let tabs = [
            ActiveTab::Fields,
            ActiveTab::Relationships,
            ActiveTab::Views,
            ActiveTab::Forms,
            ActiveTab::Entities,
        ];

        let mut spans = vec![];

        for (i, tab) in tabs.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" ", Style::default()));
            }

            let is_active = *tab == state.active_tab;
            let label = format!("[{}] {}", tab.number(), tab.label());

            spans.push(Span::styled(
                label,
                if is_active {
                    Style::default().fg(theme.accent_primary).italic()
                } else {
                    Style::default().fg(theme.text_secondary)
                },
            ));
        }

        // Add separator
        spans.push(Span::styled(" | ", Style::default().fg(theme.border_primary)));

        // Hide mode
        spans.push(Span::styled(
            format!("Hide: {}", state.hide_mode.label()),
            Style::default().fg(theme.text_secondary),
        ));

        // Sort mode
        spans.push(Span::styled(" | ", Style::default().fg(theme.border_primary)));
        spans.push(Span::styled(
            format!("Sort: {}", state.sort_mode.label()),
            Style::default().fg(theme.text_secondary),
        ));

        // Match mode
        spans.push(Span::styled(" | ", Style::default().fg(theme.border_primary)));
        spans.push(Span::styled(
            format!("Match: {}", state.match_mode.label()),
            Style::default().fg(theme.text_secondary),
        ));

        // Technical/display names indicator
        spans.push(Span::styled(" | ", Style::default().fg(theme.border_primary)));
        spans.push(Span::styled(
            if state.show_technical_names { "Names: Technical" } else { "Names: Display" },
            Style::default().fg(theme.text_secondary),
        ));

        // Mirror mode
        spans.push(Span::styled(" | ", Style::default().fg(theme.border_primary)));
        spans.push(Span::styled(
            state.mirror_mode.label(),
            Style::default().fg(theme.text_secondary),
        ));

        Some(Line::from(spans))
    }
}
