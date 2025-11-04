use crate::tui::Command;
use super::super::app::State;
use super::super::Msg;
use super::super::models::{cycle_type_filter, TypeFilterMode};

pub fn handle_toggle_type_filter_mode(state: &mut State) -> Command<Msg> {
    state.type_filter_mode = state.type_filter_mode.toggle();

    // When switching to Unified mode, sync both filters to the source filter
    if state.type_filter_mode == TypeFilterMode::Unified {
        state.unified_type_filter = state.source_type_filter.clone();
        state.target_type_filter = state.source_type_filter.clone();
    }

    // Reset scroll on all trees when filter mode changes
    state.source_fields_tree.reset_scroll();
    state.source_relationships_tree.reset_scroll();
    state.source_views_tree.reset_scroll();
    state.source_forms_tree.reset_scroll();
    state.source_entities_tree.reset_scroll();
    state.target_fields_tree.reset_scroll();
    state.target_relationships_tree.reset_scroll();
    state.target_views_tree.reset_scroll();
    state.target_forms_tree.reset_scroll();
    state.target_entities_tree.reset_scroll();

    state.invalidate_tree_cache();
    Command::None
}

pub fn handle_cycle_source_type_filter(state: &mut State) -> Command<Msg> {
    match state.type_filter_mode {
        TypeFilterMode::Unified => {
            // Cycle unified filter using source types (or combined types)
            let available_types = &state.available_source_types;
            state.unified_type_filter = cycle_type_filter(&state.unified_type_filter, available_types);
            // Sync both sides
            state.source_type_filter = state.unified_type_filter.clone();
            state.target_type_filter = state.unified_type_filter.clone();

            // Reset scroll on both sides when unified filter changes
            state.source_fields_tree.reset_scroll();
            state.source_relationships_tree.reset_scroll();
            state.source_views_tree.reset_scroll();
            state.source_forms_tree.reset_scroll();
            state.source_entities_tree.reset_scroll();
            state.target_fields_tree.reset_scroll();
            state.target_relationships_tree.reset_scroll();
            state.target_views_tree.reset_scroll();
            state.target_forms_tree.reset_scroll();
            state.target_entities_tree.reset_scroll();
        }
        TypeFilterMode::Independent => {
            // Cycle source filter only
            let available_types = &state.available_source_types;
            state.source_type_filter = cycle_type_filter(&state.source_type_filter, available_types);

            // Reset scroll on source side only when source filter changes
            state.source_fields_tree.reset_scroll();
            state.source_relationships_tree.reset_scroll();
            state.source_views_tree.reset_scroll();
            state.source_forms_tree.reset_scroll();
            state.source_entities_tree.reset_scroll();
        }
    }

    state.invalidate_tree_cache();
    Command::None
}

pub fn handle_cycle_target_type_filter(state: &mut State) -> Command<Msg> {
    match state.type_filter_mode {
        TypeFilterMode::Unified => {
            // Same as source in unified mode
            let available_types = &state.available_target_types;
            state.unified_type_filter = cycle_type_filter(&state.unified_type_filter, available_types);
            // Sync both sides
            state.source_type_filter = state.unified_type_filter.clone();
            state.target_type_filter = state.unified_type_filter.clone();

            // Reset scroll on both sides when unified filter changes
            state.source_fields_tree.reset_scroll();
            state.source_relationships_tree.reset_scroll();
            state.source_views_tree.reset_scroll();
            state.source_forms_tree.reset_scroll();
            state.source_entities_tree.reset_scroll();
            state.target_fields_tree.reset_scroll();
            state.target_relationships_tree.reset_scroll();
            state.target_views_tree.reset_scroll();
            state.target_forms_tree.reset_scroll();
            state.target_entities_tree.reset_scroll();
        }
        TypeFilterMode::Independent => {
            // Cycle target filter only
            let available_types = &state.available_target_types;
            state.target_type_filter = cycle_type_filter(&state.target_type_filter, available_types);

            // Reset scroll on target side only when target filter changes
            state.target_fields_tree.reset_scroll();
            state.target_relationships_tree.reset_scroll();
            state.target_views_tree.reset_scroll();
            state.target_forms_tree.reset_scroll();
            state.target_entities_tree.reset_scroll();
        }
    }

    state.invalidate_tree_cache();
    Command::None
}
