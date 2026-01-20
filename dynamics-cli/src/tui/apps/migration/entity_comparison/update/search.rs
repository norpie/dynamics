use super::super::Msg;
use super::super::app::State;
use super::super::models::SearchMode;
use crate::tui::FocusId;
use crate::tui::command::Command;
use crate::tui::widgets::TextInputEvent;

/// Handle toggle search - focus the search box
pub fn handle_toggle_search(state: &mut State) -> Command<Msg> {
    // Clear multi-selection when starting a search
    // to avoid confusion with filtered items
    clear_all_multi_selections(state);

    // Focus the appropriate search input based on mode
    let focus_id = match state.search_mode {
        SearchMode::Unified => "unified-search-input",
        SearchMode::Independent => "source-search-input",
    };

    Command::SetFocus(FocusId::new(focus_id))
}

/// Handle toggle search mode - switch between Unified and Independent modes
pub fn handle_toggle_search_mode(state: &mut State) -> Command<Msg> {
    match state.search_mode {
        SearchMode::Unified => {
            // Copy unified term to both source and target
            let term = state.unified_search.value().to_string();
            state.source_search.set_value(term.clone());
            state.target_search.set_value(term);
            state.search_mode = SearchMode::Independent;

            // Invalidate tree caches so they rebuild with new filtering
            invalidate_all_tree_caches(state);

            Command::SetFocus(FocusId::new("source-search-input"))
        }
        SearchMode::Independent => {
            // Copy source term to unified (or target if source is empty)
            let term = if !state.source_search.value().is_empty() {
                state.source_search.value().to_string()
            } else {
                state.target_search.value().to_string()
            };
            state.unified_search.set_value(term);
            state.source_search.set_value(String::new());
            state.target_search.set_value(String::new());
            state.search_mode = SearchMode::Unified;

            // Invalidate tree caches so they rebuild with new filtering
            invalidate_all_tree_caches(state);

            Command::SetFocus(FocusId::new("unified-search-input"))
        }
    }
}

/// Handle toggle match mode - switch between Fuzzy and Substring match algorithms
pub fn handle_toggle_match_mode(state: &mut State) -> Command<Msg> {
    // Toggle the match mode
    state.match_mode = state.match_mode.toggle();

    // Clear multi-selection since filtered items may change
    clear_all_multi_selections(state);

    // Invalidate tree caches so they rebuild with new filtering algorithm
    invalidate_all_tree_caches(state);

    Command::None
}

/// Handle unified search input event
pub fn handle_search_input_event(state: &mut State, event: TextInputEvent) -> Command<Msg> {
    let old_value = state.unified_search.value().to_string();
    state.unified_search.handle_event(event, None);
    let new_value = state.unified_search.value();

    // Clear multi-selection AND invalidate tree cache when search text changes
    // This prevents selecting items that are filtered out
    if old_value != new_value {
        clear_all_multi_selections(state);
        invalidate_all_tree_caches(state);
    }

    Command::None
}

/// Handle source search input event
pub fn handle_source_search_event(state: &mut State, event: TextInputEvent) -> Command<Msg> {
    let old_value = state.source_search.value().to_string();
    state.source_search.handle_event(event, None);
    let new_value = state.source_search.value();

    // Clear multi-selection AND invalidate tree cache when search text changes
    if old_value != new_value {
        clear_all_multi_selections(state);
        invalidate_all_tree_caches(state);
    }

    Command::None
}

/// Handle target search input event
pub fn handle_target_search_event(state: &mut State, event: TextInputEvent) -> Command<Msg> {
    let old_value = state.target_search.value().to_string();
    state.target_search.handle_event(event, None);
    let new_value = state.target_search.value();

    // Clear multi-selection AND invalidate tree cache when search text changes
    if old_value != new_value {
        clear_all_multi_selections(state);
        invalidate_all_tree_caches(state);
    }

    Command::None
}

/// Handle unified search input blur - called when search input loses focus
pub fn handle_search_input_blur(state: &mut State) -> Command<Msg> {
    log::debug!("Unified search input blurred");
    Command::None
}

/// Handle source search input blur
pub fn handle_source_search_blur(state: &mut State) -> Command<Msg> {
    log::debug!("Source search input blurred");
    Command::None
}

/// Handle target search input blur
pub fn handle_target_search_blur(state: &mut State) -> Command<Msg> {
    log::debug!("Target search input blurred");
    Command::None
}

/// Handle clear search - clear text
pub fn handle_clear_search(state: &mut State) -> Command<Msg> {
    // Clear the appropriate search fields based on mode
    match state.search_mode {
        SearchMode::Unified => {
            state.unified_search.set_value(String::new());
        }
        SearchMode::Independent => {
            state.source_search.set_value(String::new());
            state.target_search.set_value(String::new());
        }
    }

    // Clear multi-selection when clearing search
    clear_all_multi_selections(state);

    // Invalidate tree caches so they rebuild without filtering
    invalidate_all_tree_caches(state);

    Command::ClearFocus
}

/// Helper to clear multi-selections from all tree states
fn clear_all_multi_selections(state: &mut State) {
    state.source_fields_tree.clear_multi_selection();
    state.target_fields_tree.clear_multi_selection();
    state.source_relationships_tree.clear_multi_selection();
    state.target_relationships_tree.clear_multi_selection();
    state.source_views_tree.clear_multi_selection();
    state.target_views_tree.clear_multi_selection();
    state.source_forms_tree.clear_multi_selection();
    state.target_forms_tree.clear_multi_selection();
    state.source_entities_tree.clear_multi_selection();
    state.target_entities_tree.clear_multi_selection();
}

/// Helper to invalidate all tree caches
fn invalidate_all_tree_caches(state: &mut State) {
    state.source_fields_tree.invalidate_cache();
    state.target_fields_tree.invalidate_cache();
    state.source_relationships_tree.invalidate_cache();
    state.target_relationships_tree.invalidate_cache();
    state.source_views_tree.invalidate_cache();
    state.target_views_tree.invalidate_cache();
    state.source_forms_tree.invalidate_cache();
    state.target_forms_tree.invalidate_cache();
    state.source_entities_tree.invalidate_cache();
    state.target_entities_tree.invalidate_cache();
}

/// Get search terms for source and target sides
/// Returns (source_term, target_term) where each is Option<&str>
pub fn get_search_terms(state: &State) -> (Option<&str>, Option<&str>) {
    match state.search_mode {
        SearchMode::Unified => {
            let term = state.unified_search.value();
            if term.is_empty() {
                (None, None)
            } else {
                (Some(term), Some(term))
            }
        }
        SearchMode::Independent => {
            let source = state.source_search.value();
            let target = state.target_search.value();
            (
                if source.is_empty() {
                    None
                } else {
                    Some(source)
                },
                if target.is_empty() {
                    None
                } else {
                    Some(target)
                },
            )
        }
    }
}

/// Handle search select first match - select first filtered item
pub fn handle_search_select_first_match(state: &mut State) -> Command<Msg> {
    // We'll need to get the first filtered item
    // For now, we'll just keep the search open as per requirements
    // The actual selection logic will be handled in view.rs by passing filtered items
    // to the tree, which already handles selection

    // Since the tree already shows filtered results, we can leverage the tree's
    // built-in selection. We just need to ensure the search term remains in the box.

    // For now, return None to keep search open.
    // In a more complete implementation, we'd need to:
    // 1. Get the filtered tree items
    // 2. Find the first item's node ID
    // 3. Send a SourceTreeNodeClicked or TargetTreeNodeClicked message

    Command::None
}
