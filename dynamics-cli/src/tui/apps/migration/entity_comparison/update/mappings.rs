use crate::tui::command::Command;
use crate::tui::Resource;
use super::super::{Msg, ActiveTab};
use super::super::app::State;
use super::super::matching_adapter::{recompute_all_matches, recompute_all_matches_multi};

/// Parse a qualified field name into (entity, field) parts
/// Returns (entity, field) if qualified (e.g., "contact.fullname" -> ("contact", "fullname"))
/// Returns (default_entity, name) if unqualified
fn parse_qualified_name<'a>(name: &'a str, default_entity: &'a str) -> (&'a str, &'a str) {
    if let Some((entity, field)) = name.split_once('.') {
        (entity, field)
    } else {
        (default_entity, name)
    }
}

pub fn handle_create_manual_mapping(state: &mut State) -> Command<Msg> {
    // Get all selected items from source tree (multi-selection support)
    let source_tree = state.source_tree_for_tab();
    let mut source_ids = source_tree.all_selected();

    // If no explicit selection, use navigated item as implicit single selection
    if source_ids.is_empty() {
        if let Some(navigated) = source_tree.selected() {
            source_ids.push(navigated.to_string());
        }
    }

    // Get all selected items from target tree (multi-selection support for 1-to-N)
    let target_tree = state.target_tree_for_tab();
    let mut target_ids = target_tree.all_selected();

    // If no explicit target selection, use navigated item as implicit single selection
    if target_ids.is_empty() {
        if let Some(navigated) = target_tree.selected() {
            target_ids.push(navigated.to_string());
        }
    }

    // N-to-M Prevention: Don't allow multiple sources AND multiple targets in one operation
    if source_ids.len() > 1 && target_ids.len() > 1 {
        log::warn!(
            "Cannot create N-to-M mapping: {} sources to {} targets. Select either one source (for 1-to-N) or one target (for N-to-1).",
            source_ids.len(),
            target_ids.len()
        );
        return Command::None;
    }

    if !source_ids.is_empty() && !target_ids.is_empty() {
        let source_count = source_ids.len();
        let target_count = target_ids.len();

        // Determine mapping type for logging
        let mapping_type = if source_count == 1 && target_count > 1 {
            "1-to-N"
        } else if source_count > 1 && target_count == 1 {
            "N-to-1"
        } else {
            "1-to-1"
        };

        // Case 1: 1-to-N (one source, multiple targets)
        if source_count == 1 {
            let source_id = &source_ids[0];

            // Extract keys for all targets
            let target_keys: Vec<String> = target_ids.iter().map(|target_id| {
                match state.active_tab {
                    ActiveTab::Relationships => {
                        target_id.strip_prefix("rel_").unwrap_or(target_id).to_string()
                    }
                    ActiveTab::Entities => {
                        target_id.strip_prefix("entity_").unwrap_or(target_id).to_string()
                    }
                    _ => target_id.clone()
                }
            }).collect();

            // Extract source key
            let source_key = match state.active_tab {
                ActiveTab::Relationships => {
                    source_id.strip_prefix("rel_").unwrap_or(source_id).to_string()
                }
                ActiveTab::Entities => {
                    source_id.strip_prefix("entity_").unwrap_or(source_id).to_string()
                }
                _ => source_id.clone()
            };

            // Add all targets to state mappings (1-to-N support)
            state.field_mappings.insert(source_key.clone(), target_keys.clone());

            // Save to database: parse qualified names to extract entities
            let default_source_entity = state.source_entities.first().map(|s| s.as_str()).unwrap_or("");
            let default_target_entity = state.target_entities.first().map(|s| s.as_str()).unwrap_or("");

            // Parse source entity and field
            let (source_entity_str, source_field) = parse_qualified_name(&source_key, default_source_entity);
            let source_entity = source_entity_str.to_string();
            let source_field = source_field.to_string();

            // Parse each target and save
            let mut target_saves = Vec::new();
            for target_key in &target_keys {
                let (target_entity_str, target_field) = parse_qualified_name(target_key, default_target_entity);
                target_saves.push((target_entity_str.to_string(), target_field.to_string()));
            }

            tokio::spawn(async move {
                let config = crate::global_config();

                // Delete all existing mappings for this source field across all targets
                // (This handles the case where we're changing the mapping)
                for (target_entity, _) in &target_saves {
                    if let Err(e) = config.delete_field_mapping(&source_entity, target_entity, &source_field).await {
                        log::warn!("Failed to delete old field mappings for {}:{}: {}", source_entity, source_field, e);
                    }
                }

                // Then add new targets
                for (target_entity, target_field) in target_saves {
                    if let Err(e) = config.set_field_mapping(&source_entity, &target_entity, &source_field, &target_field).await {
                        log::error!("Failed to save field mapping {}:{} -> {}:{}: {}", source_entity, source_field, target_entity, target_field, e);
                    }
                }
            });
        }
        // Case 2: N-to-1 or 1-to-1 (multiple/single sources, one target)
        else {
            let target_id = &target_ids[0];

            // Extract target key
            let target_key = match state.active_tab {
                ActiveTab::Relationships => {
                    target_id.strip_prefix("rel_").unwrap_or(target_id).to_string()
                }
                ActiveTab::Entities => {
                    target_id.strip_prefix("entity_").unwrap_or(target_id).to_string()
                }
                _ => target_id.clone()
            };

            // Process each source ID
            for source_id in &source_ids {
                // Extract source key
                let source_key = match state.active_tab {
                    ActiveTab::Relationships => {
                        source_id.strip_prefix("rel_").unwrap_or(source_id).to_string()
                    }
                    ActiveTab::Entities => {
                        source_id.strip_prefix("entity_").unwrap_or(source_id).to_string()
                    }
                    _ => source_id.clone()
                };

                // Add to state mappings (wrap single target in Vec)
                state.field_mappings.insert(source_key.clone(), vec![target_key.clone()]);

                // Save to database: parse qualified names to extract entities
                let default_source_entity = state.source_entities.first().map(|s| s.as_str()).unwrap_or("");
                let default_target_entity = state.target_entities.first().map(|s| s.as_str()).unwrap_or("");

                // Parse source and target entities/fields
                let (source_entity_str, source_field) = parse_qualified_name(&source_key, default_source_entity);
                let (target_entity_str, target_field) = parse_qualified_name(&target_key, default_target_entity);

                let source_entity = source_entity_str.to_string();
                let source_field = source_field.to_string();
                let target_entity = target_entity_str.to_string();
                let target_field = target_field.to_string();

                tokio::spawn(async move {
                    let config = crate::global_config();

                    // First delete all existing targets for this source
                    if let Err(e) = config.delete_field_mapping(&source_entity, &target_entity, &source_field).await {
                        log::warn!("Failed to delete old field mappings for {}:{}: {}", source_entity, source_field, e);
                    }

                    // Then add new target
                    if let Err(e) = config.set_field_mapping(&source_entity, &target_entity, &source_field, &target_field).await {
                        log::error!("Failed to save field mapping {}:{} -> {}:{}: {}", source_entity, source_field, target_entity, target_field, e);
                    }
                });
            }
        }

        // Recompute matches after all mappings are added
        // Support both single-entity and multi-entity modes
        let is_multi_entity = state.source_entities.len() > 1 || state.target_entities.len() > 1;

        if is_multi_entity {
            // Multi-entity mode: recompute across all entity pairs
            let source_metadata_map: std::collections::HashMap<String, crate::api::EntityMetadata> =
                state.source_metadata.iter()
                    .filter_map(|(name, resource)| {
                        if let Resource::Success(metadata) = resource {
                            Some((name.clone(), metadata.clone()))
                        } else {
                            None
                        }
                    })
                    .collect();

            let target_metadata_map: std::collections::HashMap<String, crate::api::EntityMetadata> =
                state.target_metadata.iter()
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
            // Single-entity mode: backwards compatible
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

        // Log success message
        log::info!(
            "Created {} mapping: {} source(s) → {} target(s)",
            mapping_type,
            source_count,
            target_count
        );
    }
    Command::None
}

pub fn handle_delete_manual_mapping(state: &mut State) -> Command<Msg> {
    // Get selected item from source tree
    let source_id = state.source_tree_for_tab().selected().map(|s| s.to_string());

    if let Some(source_id) = source_id {
        // Extract the key based on tab type (same logic as CreateManualMapping)
        let source_key = match state.active_tab {
            ActiveTab::Fields => source_id.clone(),
            ActiveTab::Relationships => {
                source_id.strip_prefix("rel_").unwrap_or(&source_id).to_string()
            }
            ActiveTab::Entities => {
                source_id.strip_prefix("entity_").unwrap_or(&source_id).to_string()
            }
            ActiveTab::Forms | ActiveTab::Views => source_id.clone(),
        };

        // CONTEXT-AWARE 'd' KEY LOGIC:
        // Check if this field currently has a prefix match visible (including type mismatch from prefix)
        let has_prefix_match = state.field_matches.get(&source_key).map_or(false, |match_info| {
            use super::super::MatchType;
            // Check if it's explicitly a Prefix match
            if match_info.match_types.values().any(|mt| matches!(mt, MatchType::Prefix)) {
                return true;
            }
            // Check if it's a TypeMismatch that came from prefix transformation
            match_info.match_types.values().any(|mt| {
                matches!(mt, MatchType::TypeMismatch(inner) if matches!(**inner, MatchType::Prefix))
            })
        });

        // Check if this field has a manual mapping
        let has_manual_mapping = state.field_mappings.contains_key(&source_key);

        log::debug!("DeleteManualMapping: field='{}', has_manual={}, has_prefix={}", source_key, has_manual_mapping, has_prefix_match);

        // Priority: If has manual mapping, delete it. Otherwise, if has prefix match, add negative match.
        if has_manual_mapping {
            // Try to remove from field_mappings and get the targets that were deleted
            if let Some(deleted_targets) = state.field_mappings.remove(&source_key) {
                let target_count = deleted_targets.len();

                // Log what's being deleted
                if target_count > 1 {
                    log::info!(
                        "Deleting 1-to-N mapping: {} → {} ({} targets)",
                        source_key,
                        deleted_targets.join(", "),
                        target_count
                    );
                } else {
                    log::info!("Deleting mapping: {} → {}", source_key, deleted_targets.join(", "));
                }

                // Recompute matches - support both single-entity and multi-entity modes
                let is_multi_entity = state.source_entities.len() > 1 || state.target_entities.len() > 1;

                if is_multi_entity {
                    // Multi-entity mode: recompute across all entity pairs
                    let source_metadata_map: std::collections::HashMap<String, crate::api::EntityMetadata> =
                        state.source_metadata.iter()
                            .filter_map(|(name, resource)| {
                                if let Resource::Success(metadata) = resource {
                                    Some((name.clone(), metadata.clone()))
                                } else {
                                    None
                                }
                            })
                            .collect();

                    let target_metadata_map: std::collections::HashMap<String, crate::api::EntityMetadata> =
                        state.target_metadata.iter()
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
                    // Single-entity mode: backwards compatible
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

                // Delete from database: parse qualified names to get correct entities
                let default_source_entity = state.source_entities.first().map(|s| s.as_str()).unwrap_or("");
                let default_target_entity = state.target_entities.first().map(|s| s.as_str()).unwrap_or("");

                let (source_entity_str, source_field) = parse_qualified_name(&source_key, default_source_entity);
                let source_entity = source_entity_str.to_string();
                let source_field = source_field.to_string();

                // Need to delete from all target entities that had mappings
                let mut target_entities_to_delete = Vec::new();
                for target_key in &deleted_targets {
                    let (target_entity_str, _) = parse_qualified_name(target_key, default_target_entity);
                    if !target_entities_to_delete.contains(&target_entity_str.to_string()) {
                        target_entities_to_delete.push(target_entity_str.to_string());
                    }
                }

                tokio::spawn(async move {
                    let config = crate::global_config();
                    for target_entity in target_entities_to_delete {
                        if let Err(e) = config.delete_field_mapping(&source_entity, &target_entity, &source_field).await {
                            log::error!("Failed to delete field mapping {}:{}: {}", source_entity, source_field, e);
                        }
                    }
                });
            }
        } else if has_prefix_match {
            // No manual mapping, but has prefix match → add negative match to block it
            return super::negative_matches::handle_add_negative_match_from_tree(state);
        } else {
            log::warn!("Cannot delete mapping: field '{}' has neither manual mapping nor prefix match", source_key);
        }
    }
    Command::None
}

pub fn handle_cycle_hide_mode(state: &mut State) -> Command<Msg> {
    state.hide_mode = state.hide_mode.toggle();
    Command::None
}

pub fn handle_toggle_sort_mode(state: &mut State) -> Command<Msg> {
    state.sort_mode = state.sort_mode.toggle();
    Command::None
}

pub fn handle_toggle_sort_direction(state: &mut State) -> Command<Msg> {
    state.sort_direction = state.sort_direction.toggle();
    Command::None
}

pub fn handle_toggle_technical_names(state: &mut State) -> Command<Msg> {
    state.show_technical_names = !state.show_technical_names;
    Command::None
}

pub fn handle_toggle_mirror_mode(state: &mut State) -> Command<Msg> {
    state.mirror_mode = state.mirror_mode.toggle();
    Command::None
}

pub fn handle_export_to_excel(state: &mut State) -> Command<Msg> {
    // Check if metadata is loaded
    if !state.all_metadata_loaded() {
        log::warn!("Cannot export: metadata not fully loaded");
        return Command::None;
    }

    // Generate filename with timestamp
    // TODO: Support multi-entity mode - for now use first entity
    let first_source_entity = state.source_entities.first().cloned().unwrap_or_default();
    let first_target_entity = state.target_entities.first().cloned().unwrap_or_default();
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!(
        "{}_{}_to_{}_{}.xlsx",
        state.migration_name,
        first_source_entity,
        first_target_entity,
        timestamp
    );

    // Get output directory from config or use current directory
    let output_path = std::path::PathBuf::from(&filename);

    // Perform export in background
    let state_clone = state.clone();
    tokio::spawn(async move {
        match super::super::export::MigrationExporter::export_and_open(&state_clone, output_path.to_str().unwrap()) {
            Ok(_) => {
                log::info!("Successfully exported to {}", filename);
            }
            Err(e) => {
                log::error!("Failed to export to Excel: {}", e);
            }
        }
    });

    Command::None
}

pub fn handle_export_unmapped_to_csv(state: &mut State) -> Command<Msg> {
    // Check if source metadata is loaded
    if !state.all_source_metadata_loaded() {
        log::warn!("Cannot export: source metadata not loaded");
        return Command::None;
    }

    // Generate filename with timestamp
    // TODO: Support multi-entity mode - for now use first entity
    let first_source_entity = state.source_entities.first().cloned().unwrap_or_default();
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!(
        "{}_{}_unmapped_{}.csv",
        state.migration_name,
        first_source_entity,
        timestamp
    );

    // Get output directory from config or use current directory
    let output_path = std::path::PathBuf::from(&filename);

    // Perform export in background (no auto-open for CSV)
    let state_clone = state.clone();
    tokio::spawn(async move {
        match super::super::export::csv_exporter::export_unmapped_fields_to_csv(&state_clone, output_path.to_str().unwrap()) {
            Ok(_) => {
                log::info!("Successfully exported unmapped fields to {}", filename);
            }
            Err(e) => {
                log::error!("Failed to export to CSV: {}", e);
            }
        }
    });

    Command::None
}
