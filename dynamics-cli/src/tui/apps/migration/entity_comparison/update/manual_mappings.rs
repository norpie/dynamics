use super::super::Msg;
use super::super::app::State;
use super::super::matching_adapter::{recompute_all_matches, recompute_all_matches_multi};
use crate::tui::Resource;
use crate::tui::command::Command;
use std::collections::HashMap;

/// Parse a potentially qualified field name into (entity, field)
/// Examples:
/// - "contact.fullname" -> ("contact", "fullname")
/// - "fullname" -> (default_entity, "fullname")
fn parse_qualified_name<'a>(name: &'a str, default_entity: &'a str) -> (&'a str, &'a str) {
    if let Some((entity, field)) = name.split_once('.') {
        (entity, field)
    } else {
        (default_entity, name)
    }
}

pub fn handle_open_modal(state: &mut State) -> Command<Msg> {
    state.show_manual_mappings_modal = true;
    Command::None
}

pub fn handle_close_modal(state: &mut State) -> Command<Msg> {
    state.show_manual_mappings_modal = false;
    Command::None
}

pub fn handle_list_navigate(state: &mut State, key: crossterm::event::KeyCode) -> Command<Msg> {
    state
        .manual_mappings_list_state
        .handle_key(key, state.field_mappings.len(), 10);
    Command::None
}

pub fn handle_list_select(state: &mut State, index: usize) -> Command<Msg> {
    let item_count = state.field_mappings.len();
    state
        .manual_mappings_list_state
        .select_and_scroll(Some(index), item_count);
    Command::None
}

pub fn handle_delete_manual_mapping(state: &mut State) -> Command<Msg> {
    // Get selected mapping from list
    if let Some(selected_idx) = state.manual_mappings_list_state.selected() {
        // Get the mapping at this index
        let mappings_vec: Vec<_> = state.field_mappings.iter().collect();
        if let Some((source_key, target_keys)) = mappings_vec.get(selected_idx) {
            let source_key = source_key.to_string();
            let target_keys = target_keys.clone();

            // Parse qualified source name
            let default_source_entity = state
                .source_entities
                .first()
                .map(|s| s.as_str())
                .unwrap_or("");
            let default_target_entity = state
                .target_entities
                .first()
                .map(|s| s.as_str())
                .unwrap_or("");
            let (source_entity_str, source_field) =
                parse_qualified_name(&source_key, default_source_entity);
            let source_entity = source_entity_str.to_string();
            let source_field = source_field.to_string();

            // Parse qualified target names to find all entity pairs
            let mut entity_pairs = Vec::new();
            for target_key in target_keys.iter() {
                let (target_entity_str, _) =
                    parse_qualified_name(target_key, default_target_entity);
                let target_entity = target_entity_str.to_string();
                if !entity_pairs.contains(&(source_entity.clone(), target_entity.clone())) {
                    entity_pairs.push((source_entity.clone(), target_entity));
                }
            }

            // Remove from state
            state.field_mappings.remove(&source_key);

            // Recompute matches
            let is_multi_entity =
                state.source_entities.len() > 1 || state.target_entities.len() > 1;

            if is_multi_entity {
                // Multi-entity mode: use recompute_all_matches_multi()
                let source_metadata_map: HashMap<String, crate::api::EntityMetadata> = state
                    .source_metadata
                    .iter()
                    .filter_map(|(name, resource)| {
                        if let Resource::Success(metadata) = resource {
                            Some((name.clone(), metadata.clone()))
                        } else {
                            None
                        }
                    })
                    .collect();

                let target_metadata_map: HashMap<String, crate::api::EntityMetadata> = state
                    .target_metadata
                    .iter()
                    .filter_map(|(name, resource)| {
                        if let Resource::Success(metadata) = resource {
                            Some((name.clone(), metadata.clone()))
                        } else {
                            None
                        }
                    })
                    .collect();

                let (
                    field_matches,
                    relationship_matches,
                    entity_matches,
                    source_related_entities,
                    target_related_entities,
                ) = recompute_all_matches_multi(
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
                let first_source_entity =
                    state.source_entities.first().cloned().unwrap_or_default();
                let first_target_entity =
                    state.target_entities.first().cloned().unwrap_or_default();

                if let (Some(Resource::Success(source)), Some(Resource::Success(target))) = (
                    state.source_metadata.get(&first_source_entity),
                    state.target_metadata.get(&first_target_entity),
                ) {
                    let (
                        field_matches,
                        relationship_matches,
                        entity_matches,
                        source_related_entities,
                        target_related_entities,
                    ) = recompute_all_matches(
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

            // Delete from database for all entity pairs
            tokio::spawn(async move {
                let config = crate::global_config();
                for (source_entity, target_entity) in entity_pairs {
                    if let Err(e) = config
                        .delete_field_mapping(&source_entity, &target_entity, &source_field)
                        .await
                    {
                        log::error!(
                            "Failed to delete field mapping for {}/{}: {}",
                            source_entity,
                            target_entity,
                            e
                        );
                    }
                }
            });
        }
    }

    Command::None
}
