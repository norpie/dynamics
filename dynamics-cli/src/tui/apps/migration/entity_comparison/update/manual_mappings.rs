use crate::tui::command::Command;
use crate::tui::Resource;
use super::super::Msg;
use super::super::app::State;
use super::super::matching_adapter::recompute_all_matches;

pub fn handle_open_modal(state: &mut State) -> Command<Msg> {
    state.show_manual_mappings_modal = true;
    Command::None
}

pub fn handle_close_modal(state: &mut State) -> Command<Msg> {
    state.show_manual_mappings_modal = false;
    Command::None
}

pub fn handle_list_navigate(state: &mut State, key: crossterm::event::KeyCode) -> Command<Msg> {
    state.manual_mappings_list_state.handle_key(key, state.field_mappings.len(), 10);
    Command::None
}

pub fn handle_list_select(state: &mut State, index: usize) -> Command<Msg> {
    let item_count = state.field_mappings.len();
    state.manual_mappings_list_state.select_and_scroll(Some(index), item_count);
    Command::None
}

pub fn handle_delete_manual_mapping(state: &mut State) -> Command<Msg> {
    // Get selected mapping from list
    if let Some(selected_idx) = state.manual_mappings_list_state.selected() {
        // Get the mapping at this index
        let mappings_vec: Vec<_> = state.field_mappings.iter().collect();
        if let Some((source_field, _)) = mappings_vec.get(selected_idx) {
            let source_field = source_field.to_string();

            // Remove from state
            state.field_mappings.remove(&source_field);

            // Recompute matches
            // TODO: Support multi-entity mode - for now use first entity
            let first_source_entity = state.source_entities.first().cloned().unwrap_or_default();
            let first_target_entity = state.target_entities.first().cloned().unwrap_or_default();

            if let (Some(Resource::Success(source)), Some(Resource::Success(target))) =
                (state.source_metadata.get(&first_source_entity), state.target_metadata.get(&first_target_entity))
            {
                let (field_matches, relationship_matches, entity_matches, source_related_entities, target_related_entities) =
                    recompute_all_matches(
                        source,
                        target,
                        &state.field_mappings,
                        &state.imported_mappings,
                        &state.prefix_mappings,
                        &state.examples,
                        &first_source_entity,
                        &first_target_entity,
                        &state.negative_matches,
                    );
                state.field_matches = field_matches;
                state.relationship_matches = relationship_matches;
                state.entity_matches = entity_matches;
                state.source_related_entities = source_related_entities;
                state.target_related_entities = target_related_entities;
            }

            // Delete from database
            let source_entity = first_source_entity;
            let target_entity = first_target_entity;
            tokio::spawn(async move {
                let config = crate::global_config();
                if let Err(e) = config.delete_field_mapping(&source_entity, &target_entity, &source_field).await {
                    log::error!("Failed to delete field mapping: {}", e);
                }
            });
        }
    }

    Command::None
}
