use crate::tui::command::Command;
use crate::tui::Resource;
use super::super::Msg;
use super::super::app::State;
use super::super::matching_adapter::recompute_all_matches;

pub fn handle_open_modal(state: &mut State) -> Command<Msg> {
    state.show_negative_matches_modal = true;
    Command::None
}

pub fn handle_close_modal(state: &mut State) -> Command<Msg> {
    state.show_negative_matches_modal = false;
    Command::None
}

pub fn handle_list_navigate(state: &mut State, key: crossterm::event::KeyCode) -> Command<Msg> {
    state.negative_matches_list_state.handle_key(key, state.negative_matches.len(), 10);
    Command::None
}

pub fn handle_list_select(state: &mut State, index: usize) -> Command<Msg> {
    let item_count = state.negative_matches.len();
    state.negative_matches_list_state.select_and_scroll(Some(index), item_count);
    Command::None
}

pub fn handle_delete_negative_match(state: &mut State) -> Command<Msg> {
    // Get selected negative match from list
    if let Some(selected_idx) = state.negative_matches_list_state.selected() {
        // Convert HashSet to sorted Vec to get consistent ordering
        let mut matches_vec: Vec<_> = state.negative_matches.iter().cloned().collect();
        matches_vec.sort();

        if let Some(source_field) = matches_vec.get(selected_idx) {
            let source_field = source_field.clone();

            // Remove from state
            state.negative_matches.remove(&source_field);

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
                if let Err(e) = config.delete_negative_match(&source_entity, &target_entity, &source_field).await {
                    log::error!("Failed to delete negative match: {}", e);
                }
            });
        }
    }

    Command::None
}

pub fn handle_add_negative_match_from_tree(state: &mut State) -> Command<Msg> {
    // Get currently selected source field from tree
    let source_tree = state.source_tree_for_tab();
    if let Some(selected_node) = source_tree.selected() {
        // Get the field name from the selected node
        // The node_id format varies by tab (fields, forms, views, relationships, entities)
        let source_field = selected_node.to_string();

        // Verify this field has a prefix match before adding negative match
        if let Some(match_info) = state.field_matches.get(&source_field) {
            use super::super::MatchType;

            // Check if it's a prefix match (including type mismatch from prefix)
            let is_prefix_match = match_info.match_types.values().any(|mt| matches!(mt, MatchType::Prefix));

            // Also check if it's a TypeMismatch from prefix transformation
            let is_prefix_type_mismatch = if !is_prefix_match {
                match_info.match_types.values().any(|mt| {
                    matches!(mt, MatchType::TypeMismatch(inner) if matches!(**inner, MatchType::Prefix))
                })
            } else {
                false
            };

            if !is_prefix_match && !is_prefix_type_mismatch {
                log::warn!("Cannot add negative match: field '{}' is not prefix-matched", source_field);
                return Command::None;
            }

            // Add to state
            state.negative_matches.insert(source_field.clone());

            // Recompute matches (this will now exclude the negative match)
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

            // Save to database
            let source_entity = first_source_entity;
            let target_entity = first_target_entity;
            let source_field_clone = source_field.clone();
            tokio::spawn(async move {
                let config = crate::global_config();
                if let Err(e) = config.add_negative_match(&source_entity, &target_entity, &source_field_clone).await {
                    log::error!("Failed to add negative match: {}", e);
                }
            });

            log::info!("Added negative match for field: {}", source_field);
        } else {
            log::warn!("Cannot add negative match: field '{}' has no match", source_field);
        }
    } else {
        log::warn!("Cannot add negative match: no source field selected");
    }

    Command::None
}
