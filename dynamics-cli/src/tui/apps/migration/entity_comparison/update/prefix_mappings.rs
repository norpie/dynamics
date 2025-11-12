use crate::tui::command::Command;
use crate::tui::Resource;
use super::super::Msg;
use super::super::app::State;
use super::super::matching_adapter::{recompute_all_matches, recompute_all_matches_multi};
use std::collections::HashMap;

pub fn handle_open_modal(state: &mut State) -> Command<Msg> {
    state.show_prefix_mappings_modal = true;
    // Clear input fields
    state.prefix_source_input.value.clear();
    state.prefix_target_input.value.clear();
    Command::None
}

pub fn handle_close_modal(state: &mut State) -> Command<Msg> {
    state.show_prefix_mappings_modal = false;
    Command::None
}

pub fn handle_list_navigate(state: &mut State, key: crossterm::event::KeyCode) -> Command<Msg> {
    state.prefix_mappings_list_state.handle_key(key, state.prefix_mappings.len(), 10);
    Command::None
}

pub fn handle_list_select(state: &mut State, index: usize) -> Command<Msg> {
    let item_count = state.prefix_mappings.len();
    state.prefix_mappings_list_state.select_and_scroll(Some(index), item_count);
    Command::None
}

pub fn handle_source_input_event(
    state: &mut State,
    event: crate::tui::widgets::TextInputEvent,
) -> Command<Msg> {
    state.prefix_source_input.handle_event(event, None);
    Command::None
}

pub fn handle_target_input_event(
    state: &mut State,
    event: crate::tui::widgets::TextInputEvent,
) -> Command<Msg> {
    state.prefix_target_input.handle_event(event, None);
    Command::None
}

pub fn handle_add_prefix_mapping(state: &mut State) -> Command<Msg> {
    let source_prefix = state.prefix_source_input.value.trim().to_string();
    let target_prefix = state.prefix_target_input.value.trim().to_string();

    // Validate inputs
    if source_prefix.is_empty() || target_prefix.is_empty() {
        log::warn!("Cannot add prefix mapping: both source and target prefixes must be provided");
        return Command::None;
    }

    // Add to state (wrap single target in Vec for 1-to-N support)
    state.prefix_mappings.insert(source_prefix.clone(), vec![target_prefix.clone()]);

    // Recompute matches
    let is_multi_entity = state.source_entities.len() > 1 || state.target_entities.len() > 1;

    if is_multi_entity {
        // Multi-entity mode: use recompute_all_matches_multi()
        let source_metadata_map: HashMap<String, crate::api::EntityMetadata> = state.source_metadata.iter()
            .filter_map(|(name, resource)| {
                if let Resource::Success(metadata) = resource {
                    Some((name.clone(), metadata.clone()))
                } else {
                    None
                }
            })
            .collect();

        let target_metadata_map: HashMap<String, crate::api::EntityMetadata> = state.target_metadata.iter()
            .filter_map(|(name, resource)| {
                if let Resource::Success(metadata) = resource {
                    Some((name.clone(), metadata.clone()))
                } else {
                    None
                }
            })
            .collect();

        let (field_matches, relationship_matches, entity_matches, source_related_entities, target_related_entities) =
            recompute_all_matches_multi(
                &source_metadata_map,
                &target_metadata_map,
                &state.source_entities,
                &state.target_entities,
                &state.field_mappings,
                &state.imported_mappings,
                &state.prefix_mappings,
                &state.examples,
                &state.negative_matches,
            );
        state.field_matches = field_matches;
        state.relationship_matches = relationship_matches;
        state.entity_matches = entity_matches;
        state.source_related_entities = source_related_entities;
        state.target_related_entities = target_related_entities;
    } else {
        // Single-entity mode: use first entity (backwards compatible)
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
    }

    // Save to database for ALL entity pairs
    let source_entities = state.source_entities.clone();
    let target_entities = state.target_entities.clone();
    tokio::spawn(async move {
        let config = crate::global_config();
        for source_entity in &source_entities {
            for target_entity in &target_entities {
                if let Err(e) = config.set_prefix_mapping(source_entity, target_entity, &source_prefix, &target_prefix).await {
                    log::error!("Failed to save prefix mapping for {}/{}: {}", source_entity, target_entity, e);
                }
            }
        }
    });

    // Clear inputs
    state.prefix_source_input.value.clear();
    state.prefix_target_input.value.clear();

    Command::None
}

pub fn handle_delete_prefix_mapping(state: &mut State) -> Command<Msg> {
    // Get selected mapping from list
    if let Some(selected_idx) = state.prefix_mappings_list_state.selected() {
        // Get the mapping at this index
        let mappings_vec: Vec<_> = state.prefix_mappings.iter().collect();
        if let Some((source_prefix, _)) = mappings_vec.get(selected_idx) {
            let source_prefix = source_prefix.to_string();

            // Remove from state
            state.prefix_mappings.remove(&source_prefix);

            // Recompute matches
            let is_multi_entity = state.source_entities.len() > 1 || state.target_entities.len() > 1;

            if is_multi_entity {
                // Multi-entity mode: use recompute_all_matches_multi()
                let source_metadata_map: HashMap<String, crate::api::EntityMetadata> = state.source_metadata.iter()
                    .filter_map(|(name, resource)| {
                        if let Resource::Success(metadata) = resource {
                            Some((name.clone(), metadata.clone()))
                        } else {
                            None
                        }
                    })
                    .collect();

                let target_metadata_map: HashMap<String, crate::api::EntityMetadata> = state.target_metadata.iter()
                    .filter_map(|(name, resource)| {
                        if let Resource::Success(metadata) = resource {
                            Some((name.clone(), metadata.clone()))
                        } else {
                            None
                        }
                    })
                    .collect();

                let (field_matches, relationship_matches, entity_matches, source_related_entities, target_related_entities) =
                    recompute_all_matches_multi(
                        &source_metadata_map,
                        &target_metadata_map,
                        &state.source_entities,
                        &state.target_entities,
                        &state.field_mappings,
                        &state.imported_mappings,
                        &state.prefix_mappings,
                        &state.examples,
                        &state.negative_matches,
                    );
                state.field_matches = field_matches;
                state.relationship_matches = relationship_matches;
                state.entity_matches = entity_matches;
                state.source_related_entities = source_related_entities;
                state.target_related_entities = target_related_entities;
            } else {
                // Single-entity mode: use first entity (backwards compatible)
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
            }

            // Delete from database for ALL entity pairs
            let source_entities = state.source_entities.clone();
            let target_entities = state.target_entities.clone();
            tokio::spawn(async move {
                let config = crate::global_config();
                for source_entity in &source_entities {
                    for target_entity in &target_entities {
                        if let Err(e) = config.delete_prefix_mapping(source_entity, target_entity, &source_prefix).await {
                            log::error!("Failed to delete prefix mapping for {}/{}: {}", source_entity, target_entity, e);
                        }
                    }
                }
            });
        }
    }

    Command::None
}
