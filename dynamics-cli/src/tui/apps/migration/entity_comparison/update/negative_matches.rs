use crate::tui::command::Command;
use crate::tui::Resource;
use super::super::Msg;
use super::super::app::State;
use super::super::matching_adapter::{recompute_all_matches, recompute_all_matches_multi};
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

            // Parse qualified name to get entity and field
            let default_source_entity = state.source_entities.first().map(|s| s.as_str()).unwrap_or("");
            let default_target_entity = state.target_entities.first().map(|s| s.as_str()).unwrap_or("");
            let (source_entity_str, source_field_name) = parse_qualified_name(&source_field, default_source_entity);
            let source_entity = source_entity_str.to_string();
            let source_field_name = source_field_name.to_string();

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

            // Delete from database for all target entities that might have had this negative match
            let target_entities = state.target_entities.clone();
            tokio::spawn(async move {
                let config = crate::global_config();
                for target_entity in &target_entities {
                    if let Err(e) = config.delete_negative_match(&source_entity, target_entity, &source_field_name).await {
                        log::error!("Failed to delete negative match for {}/{}: {}", source_entity, target_entity, e);
                    }
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
        // In multi-entity mode, this will be qualified (e.g., "contact.fullname")
        let source_key = selected_node.to_string();

        // Verify this field has a prefix match before adding negative match
        if let Some(match_info) = state.field_matches.get(&source_key) {
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
                log::warn!("Cannot add negative match: field '{}' is not prefix-matched", source_key);
                return Command::None;
            }

            // Parse qualified name to get entity and field
            let default_source_entity = state.source_entities.first().map(|s| s.as_str()).unwrap_or("");
            let (source_entity_str, source_field) = parse_qualified_name(&source_key, default_source_entity);
            let source_entity = source_entity_str.to_string();
            let source_field = source_field.to_string();

            // Add to state (keep qualified name in state for tree matching)
            state.negative_matches.insert(source_key.clone());

            // Recompute matches (this will now exclude the negative match)
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

            // Save to database for all target entities
            let target_entities = state.target_entities.clone();
            tokio::spawn(async move {
                let config = crate::global_config();
                for target_entity in &target_entities {
                    if let Err(e) = config.add_negative_match(&source_entity, target_entity, &source_field).await {
                        log::error!("Failed to add negative match for {}/{}: {}", source_entity, target_entity, e);
                    }
                }
            });

            log::info!("Added negative match for field: {}", source_key);
        } else {
            log::warn!("Cannot add negative match: field '{}' has no match", source_key);
        }
    } else {
        log::warn!("Cannot add negative match: no source field selected");
    }

    Command::None
}
