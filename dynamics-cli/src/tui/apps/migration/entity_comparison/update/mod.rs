pub mod navigation;
pub mod tree_events;
pub mod mappings;
pub mod examples;
pub mod prefix_mappings;
pub mod manual_mappings;
pub mod data_loading;
pub mod import;
pub mod ignore;
pub mod search;

use crate::tui::command::Command;
use super::Msg;
use super::app::State;

pub fn update(state: &mut State, msg: Msg) -> Command<Msg> {
    // Rebuild tree cache if dependencies changed
    // This happens once per state change, not every frame
    if state.should_rebuild_cache() {
        state.rebuild_tree_cache();
    }

    match msg {
        // Navigation
        Msg::Back => navigation::handle_back(state),
        Msg::ConfirmBack => navigation::handle_confirm_back(),
        Msg::CancelBack => navigation::handle_cancel_back(state),
        Msg::SwitchTab(n) => navigation::handle_switch_tab(state, n),

        // Tree events
        Msg::SourceTreeEvent(event) => tree_events::handle_source_tree_event(state, event),
        Msg::TargetTreeEvent(event) => tree_events::handle_target_tree_event(state, event),
        Msg::SourceViewportHeight(h) => tree_events::handle_source_viewport_height(state, h),
        Msg::TargetViewportHeight(h) => tree_events::handle_target_viewport_height(state, h),
        Msg::SourceTreeNodeClicked(node_id) => tree_events::handle_source_node_clicked(state, node_id),
        Msg::TargetTreeNodeClicked(node_id) => tree_events::handle_target_node_clicked(state, node_id),
        Msg::SourceTreeFocused => tree_events::handle_source_tree_focused(state),
        Msg::TargetTreeFocused => tree_events::handle_target_tree_focused(state),

        // Data loading
        Msg::ParallelDataLoaded(idx, result) => data_loading::handle_parallel_data_loaded(state, idx, result),
        Msg::MappingsLoaded(fm, pm, im, isf, ep, ig) => data_loading::handle_mappings_loaded(state, fm, pm, im, isf, ep, ig),
        Msg::Refresh => data_loading::handle_refresh(state),

        // Mappings
        Msg::CreateManualMapping => mappings::handle_create_manual_mapping(state),
        Msg::DeleteManualMapping => mappings::handle_delete_manual_mapping(state),
        Msg::CycleHideMode => mappings::handle_cycle_hide_mode(state),
        Msg::ToggleSortMode => mappings::handle_toggle_sort_mode(state),
        Msg::ToggleSortDirection => mappings::handle_toggle_sort_direction(state),
        Msg::ToggleTechnicalNames => mappings::handle_toggle_technical_names(state),
        Msg::ToggleMirrorMode => mappings::handle_toggle_mirror_mode(state),

        // Examples
        Msg::OpenExamplesModal => examples::handle_open_modal(state),
        Msg::CloseExamplesModal => examples::handle_close_modal(state),
        Msg::ExamplesListNavigate(key) => examples::handle_list_navigate(state, key),
        Msg::ExamplesListSelect(idx) => examples::handle_list_select(state, idx),
        Msg::SourceInputEvent(event) => examples::handle_source_input_event(state, event),
        Msg::TargetInputEvent(event) => examples::handle_target_input_event(state, event),
        Msg::LabelInputEvent(event) => examples::handle_label_input_event(state, event),
        Msg::AddExamplePair => examples::handle_add_example_pair(state),
        Msg::DeleteExamplePair => examples::handle_delete_example_pair(state),
        Msg::ExampleDataFetched(id, result) => examples::handle_example_data_fetched(state, id, result),
        Msg::CycleExamplePair => examples::handle_cycle_example_pair(state),
        Msg::ToggleExamples => examples::handle_toggle_examples(state),

        // Prefix mappings
        Msg::OpenPrefixMappingsModal => prefix_mappings::handle_open_modal(state),
        Msg::ClosePrefixMappingsModal => prefix_mappings::handle_close_modal(state),
        Msg::PrefixMappingsListNavigate(key) => prefix_mappings::handle_list_navigate(state, key),
        Msg::PrefixMappingsListSelect(idx) => prefix_mappings::handle_list_select(state, idx),
        Msg::PrefixSourceInputEvent(event) => prefix_mappings::handle_source_input_event(state, event),
        Msg::PrefixTargetInputEvent(event) => prefix_mappings::handle_target_input_event(state, event),
        Msg::AddPrefixMapping => prefix_mappings::handle_add_prefix_mapping(state),
        Msg::DeletePrefixMapping => prefix_mappings::handle_delete_prefix_mapping(state),

        // Manual mappings
        Msg::OpenManualMappingsModal => manual_mappings::handle_open_modal(state),
        Msg::CloseManualMappingsModal => manual_mappings::handle_close_modal(state),
        Msg::ManualMappingsListNavigate(key) => manual_mappings::handle_list_navigate(state, key),
        Msg::ManualMappingsListSelect(idx) => manual_mappings::handle_list_select(state, idx),
        Msg::DeleteManualMappingFromModal => manual_mappings::handle_delete_manual_mapping(state),

        // Search
        Msg::ToggleSearch => search::handle_toggle_search(state),
        Msg::ToggleSearchMode => search::handle_toggle_search_mode(state),
        Msg::ToggleMatchMode => search::handle_toggle_match_mode(state),
        Msg::SearchInputEvent(event) => search::handle_search_input_event(state, event),
        Msg::SourceSearchEvent(event) => search::handle_source_search_event(state, event),
        Msg::TargetSearchEvent(event) => search::handle_target_search_event(state, event),
        Msg::SearchInputBlur => search::handle_search_input_blur(state),
        Msg::SourceSearchBlur => search::handle_source_search_blur(state),
        Msg::TargetSearchBlur => search::handle_target_search_blur(state),
        Msg::ClearSearch => search::handle_clear_search(state),
        Msg::SearchSelectFirstMatch => search::handle_search_select_first_match(state),

        // Export
        Msg::ExportToExcel => mappings::handle_export_to_excel(state),

        // Import from C# file or CSV
        Msg::OpenImportModal => import::handle_open_modal(state),
        Msg::CloseImportModal => import::handle_close_modal(state),
        Msg::ImportFileSelected(path) => import::handle_file_selected(state, path),
        Msg::ImportMappingsLoaded(mappings, file) => import::handle_mappings_loaded(state, mappings, file),
        Msg::ImportCsvLoaded(csv_data, file) => import::handle_csv_loaded(state, csv_data, file),
        Msg::ClearImportedMappings => import::handle_clear_imported(state),
        Msg::ImportNavigate(key) => import::handle_navigate(state, key),
        Msg::ImportSetViewportHeight(h) => import::handle_set_viewport_height(state, h),
        Msg::CloseImportResultsModal => import::handle_close_results_modal(state),
        Msg::ImportResultsNavigate(key) => import::handle_results_navigate(state, key),
        Msg::ImportResultsSelect(idx) => import::handle_results_select(state, idx),
        Msg::ImportResultsSetViewportHeight(h) => import::handle_results_set_viewport_height(state, h),

        // Ignore functionality
        Msg::IgnoreItem => ignore::handle_ignore_item(state),
        Msg::OpenIgnoreModal => ignore::handle_open_modal(state),
        Msg::CloseIgnoreModal => ignore::handle_close_modal(state),
        Msg::IgnoreListNavigate(key) => ignore::handle_navigate(state, key),
        Msg::IgnoreListSelect(idx) => ignore::handle_select(state, idx),
        Msg::DeleteIgnoredItem => ignore::handle_delete_item(state),
        Msg::ClearAllIgnored => ignore::handle_clear_all(state),
        Msg::IgnoreSetViewportHeight(h) => ignore::handle_set_viewport_height(state, h),
        Msg::IgnoredItemsSaved => Command::None, // No-op message
    }
}
