//! Import handlers for C# mapping file import

use super::super::Msg;
use super::super::app::State;
use crate::tui::{Command, Resource};
use crate::tui::widgets::{FileBrowserEvent, FileBrowserAction};
use std::path::PathBuf;
use std::collections::HashMap;
use crossterm::event::KeyCode;
use super::super::matching_adapter::{recompute_all_matches, recompute_all_matches_multi};

/// Open the import modal with file browser
pub fn handle_open_modal(state: &mut State) -> Command<Msg> {
    state.show_import_modal = true;

    // Set filter to show only .cs/.csv files and directories
    state.import_file_browser.set_filter(|entry| {
        entry.is_dir || entry.name.to_lowercase().ends_with(".cs") || entry.name.to_lowercase().ends_with(".csv")
    });

    // Refresh to apply filter
    let _ = state.import_file_browser.refresh();

    Command::set_focus(crate::tui::FocusId::new("import-file-browser"))
}

/// Close the import modal
pub fn handle_close_modal(state: &mut State) -> Command<Msg> {
    state.show_import_modal = false;
    Command::None
}

/// Handle navigation in file browser
pub fn handle_navigate(state: &mut State, key: KeyCode) -> Command<Msg> {
    match key {
        KeyCode::Up => {
            state.import_file_browser.navigate_up();
            Command::None
        }
        KeyCode::Down => {
            state.import_file_browser.navigate_down();
            Command::None
        }
        KeyCode::Enter => {
            if let Some(action) = state.import_file_browser.handle_event(FileBrowserEvent::Activate) {
                match action {
                    FileBrowserAction::FileSelected(path) => {
                        Command::perform(async move { path }, Msg::ImportFileSelected)
                    }
                    FileBrowserAction::DirectoryEntered(_) => {
                        // Just stay in the modal, directory already changed
                        Command::None
                    }
                    _ => Command::None
                }
            } else {
                Command::None
            }
        }
        KeyCode::Backspace => {
            if let Some(_action) = state.import_file_browser.handle_event(FileBrowserEvent::GoUp) {
                Command::None
            } else {
                Command::None
            }
        }
        _ => {
            state.import_file_browser.handle_navigation_key(key);
            Command::None
        }
    }
}

/// Handle file selection - read and parse the file (.cs or .csv)
pub fn handle_file_selected(_state: &mut State, path: PathBuf) -> Command<Msg> {
    // Detect file type by extension
    let is_csv = path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase() == "csv")
        .unwrap_or(false);

    if is_csv {
        // Parse CSV file
        Command::perform(
            async move {
                // Read file
                let content = tokio::fs::read_to_string(&path).await
                    .map_err(|e| format!("Failed to read file: {}", e))?;

                // Parse CSV mappings
                let csv_data = crate::csv_parser::parse_csv_field_mappings(&content)?;

                // Extract filename
                let filename = path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                log::info!("Successfully parsed CSV from {}: {} manual, {} prefix, {} imported, {} source ignores, {} target ignores",
                    filename,
                    csv_data.manual_mappings.len(),
                    csv_data.prefix_mappings.len(),
                    csv_data.imported_mappings.len(),
                    csv_data.source_ignores.len(),
                    csv_data.target_ignores.len()
                );
                Ok((csv_data, filename))
            },
            |result: Result<(crate::csv_parser::CsvImportData, String), String>| {
                match result {
                    Ok((csv_data, filename)) => Msg::ImportCsvLoaded(csv_data, filename),
                    Err(err) => {
                        log::error!("Failed to parse CSV mappings: {}", err);
                        // TODO: Show error modal instead of just closing
                        Msg::CloseImportModal
                    }
                }
            }
        )
    } else {
        // Parse C# file
        Command::perform(
            async move {
                // Read file
                let content = tokio::fs::read_to_string(&path).await
                    .map_err(|e| format!("Failed to read file: {}", e))?;

                // Parse C# mappings
                let mappings = crate::cs_parser::parse_cs_field_mappings(&content)?;

                // Extract filename
                let filename = path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                log::info!("Successfully parsed {} mappings from {}", mappings.len(), filename);
                Ok((mappings, filename))
            },
            |result: Result<(HashMap<String, String>, String), String>| {
                match result {
                    Ok((mappings, filename)) => Msg::ImportMappingsLoaded(mappings, filename),
                    Err(err) => {
                        log::error!("Failed to parse C# mappings: {}", err);
                        // TODO: Show error modal instead of just closing
                        Msg::CloseImportModal
                    }
                }
            }
        )
    }
}

/// Handle CSV data loaded - merge into multiple mapping types
pub fn handle_csv_loaded(state: &mut State, csv_data: crate::csv_parser::CsvImportData, filename: String) -> Command<Msg> {
    log::info!("Loading CSV data from {}", filename);

    // Determine if we're in multi-entity mode
    let is_multi_entity = state.source_entities.len() > 1 || state.target_entities.len() > 1;

    // Track changes across all mapping types for import results
    let mut all_added = Vec::new();
    let mut all_updated = Vec::new();
    let mut all_removed = Vec::new(); // CSV import doesn't remove, but keep for consistency

    // Helper to expand mappings in multi-entity mode
    let expand_mappings = |mappings: &HashMap<String, String>| -> HashMap<String, Vec<String>> {
        if is_multi_entity {
            let mut expanded = HashMap::new();
            for (simple_src, simple_tgt) in mappings {
                // Expand to all source/target entity combinations
                for source_entity in &state.source_entities {
                    for target_entity in &state.target_entities {
                        let qualified_src = format!("{}.{}", source_entity, simple_src);
                        let qualified_tgt = format!("{}.{}", target_entity, simple_tgt);
                        expanded.entry(qualified_src)
                            .or_insert_with(Vec::new)
                            .push(qualified_tgt);
                    }
                }
            }
            expanded
        } else {
            mappings.iter()
                .map(|(k, v)| (k.clone(), vec![v.clone()]))
                .collect()
        }
    };

    // Process manual mappings (MERGE) - convert single values to Vec for 1-to-N support
    let converted_manual = expand_mappings(&csv_data.manual_mappings);
    for (src, tgts) in &converted_manual {
        for tgt in tgts {
            if let Some(old_tgts) = state.field_mappings.get(src) {
                // Check if this target already exists in the vector
                if !old_tgts.contains(tgt) {
                    all_updated.push((format!("[manual] {}", src), tgt.clone()));
                }
            } else {
                all_added.push((format!("[manual] {}", src), tgt.clone()));
            }
        }
    }
    state.field_mappings.extend(converted_manual);

    // Process prefix mappings (MERGE) - convert single values to Vec for 1-to-N support
    let converted_prefix = expand_mappings(&csv_data.prefix_mappings);
    for (src, tgts) in &converted_prefix {
        for tgt in tgts {
            if let Some(old_tgts) = state.prefix_mappings.get(src) {
                // Check if this target already exists in the vector
                if !old_tgts.contains(tgt) {
                    all_updated.push((format!("[prefix] {}", src), tgt.clone()));
                }
            } else {
                all_added.push((format!("[prefix] {}", src), tgt.clone()));
            }
        }
    }
    state.prefix_mappings.extend(converted_prefix);

    // Process imported mappings (MERGE) - convert single values to Vec for 1-to-N support
    let converted_imported = expand_mappings(&csv_data.imported_mappings);
    for (src, tgts) in &converted_imported {
        for tgt in tgts {
            if let Some(old_tgts) = state.imported_mappings.get(src) {
                // Check if this target already exists in the vector
                if !old_tgts.contains(tgt) {
                    all_updated.push((format!("[import] {}", src), tgt.clone()));
                }
            } else {
                all_added.push((format!("[import] {}", src), tgt.clone()));
            }
        }
    }
    state.imported_mappings.extend(converted_imported);

    // Update import source file (append if exists)
    if let Some(existing_file) = &state.import_source_file {
        state.import_source_file = Some(format!("{}, {}", existing_file, filename));
    } else {
        state.import_source_file = Some(filename.clone());
    }

    // Process ignores (MERGE)
    for ignore_id in &csv_data.source_ignores {
        if state.ignored_items.insert(ignore_id.clone()) {
            all_added.push((format!("[ignore] {}", ignore_id), String::new()));
        }
    }
    for ignore_id in &csv_data.target_ignores {
        if state.ignored_items.insert(ignore_id.clone()) {
            all_added.push((format!("[ignore] {}", ignore_id), String::new()));
        }
    }

    log::info!("CSV import merged: {} added, {} updated, {} removed",
        all_added.len(), all_updated.len(), all_removed.len());

    // Store results for modal
    state.import_results = Some(super::super::app::ImportResults {
        filename: filename.clone(),
        added: all_added,
        updated: all_updated,
        removed: all_removed,
        unparsed: vec![],
    });
    state.show_import_results_modal = true;
    state.show_import_modal = false;

    // Recompute all matches with updated mappings
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
        // Single-entity mode: backwards compatible
        let first_source_entity = state.source_entities.first().cloned().unwrap_or_default();
        let first_target_entity = state.target_entities.first().cloned().unwrap_or_default();

        if let (Some(Resource::Success(source_metadata)), Some(Resource::Success(target_metadata))) = (
            state.source_metadata.get(&first_source_entity),
            state.target_metadata.get(&first_target_entity),
        ) {
            let (field_matches, relationship_matches, entity_matches, source_related_entities, target_related_entities) =
                recompute_all_matches(
                    source_metadata,
                    target_metadata,
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

    // Persist all mapping types to config (async, don't wait)
    // Note: In multi-entity mode, mappings are already qualified (e.g., "contact.fullname")
    // In single-entity mode, use first entity for backwards compat
    let source_entities = state.source_entities.clone();
    let target_entities = state.target_entities.clone();
    let field_mappings = state.field_mappings.clone();
    let prefix_mappings = state.prefix_mappings.clone();
    let imported_mappings = state.imported_mappings.clone();
    let import_file = state.import_source_file.clone();
    let ignored_items = state.ignored_items.clone();

    Command::perform(
        async move {
            let config = crate::global_config();

            // Helper to parse qualified names
            fn parse_qualified_name<'a>(name: &'a str, default_entity: &'a str) -> (&'a str, &'a str) {
                if let Some((entity, field)) = name.split_once('.') {
                    (entity, field)
                } else {
                    (default_entity, name)
                }
            }

            let default_source = source_entities.first().map(|s| s.as_str()).unwrap_or("");
            let default_target = target_entities.first().map(|s| s.as_str()).unwrap_or("");

            // Save field mappings (parse qualified names and save to correct entity pairs)
            for (src, tgts) in field_mappings {
                let (src_entity, src_field) = parse_qualified_name(&src, default_source);
                for tgt in tgts {
                    let (tgt_entity, tgt_field) = parse_qualified_name(&tgt, default_target);
                    if let Err(e) = config.set_field_mapping(src_entity, tgt_entity, src_field, tgt_field).await {
                        log::error!("Failed to save field mapping {} -> {}: {}", src, tgt, e);
                    }
                }
            }

            // Save prefix mappings (global across all entity pairs)
            for (src, tgts) in prefix_mappings {
                for tgt in tgts {
                    for source_entity in &source_entities {
                        for target_entity in &target_entities {
                            if let Err(e) = config.set_prefix_mapping(source_entity, target_entity, &src, &tgt).await {
                                log::error!("Failed to save prefix mapping {}/{} {} -> {}: {}", source_entity, target_entity, src, tgt, e);
                            }
                        }
                    }
                }
            }

            // Save imported mappings - group by entity pair and use batch method
            if let Some(file) = import_file {
                // Group mappings by entity pair
                let mut mappings_by_pair: std::collections::HashMap<(String, String), std::collections::HashMap<String, Vec<String>>> = std::collections::HashMap::new();

                for (src, tgts) in imported_mappings {
                    let (src_entity, src_field) = parse_qualified_name(&src, default_source);
                    for tgt in &tgts {
                        let (tgt_entity, tgt_field) = parse_qualified_name(tgt, default_target);
                        let pair_key = (src_entity.to_string(), tgt_entity.to_string());

                        mappings_by_pair
                            .entry(pair_key)
                            .or_insert_with(std::collections::HashMap::new)
                            .entry(src_field.to_string())
                            .or_insert_with(Vec::new)
                            .push(tgt_field.to_string());
                    }
                }

                // Save each entity pair's mappings
                for ((src_entity, tgt_entity), mappings) in mappings_by_pair {
                    if let Err(e) = config.set_imported_mappings(&src_entity, &tgt_entity, &mappings, &file).await {
                        log::error!("Failed to save imported mappings for {}/{}: {}", src_entity, tgt_entity, e);
                    }
                }
            }

            // Save ignored items - group by entity pair and use batch method
            let mut ignored_by_pair: std::collections::HashMap<(String, String), std::collections::HashSet<String>> = std::collections::HashMap::new();

            for item in ignored_items {
                // Ignored items format: "fields:source:field_name" or "fields:source:entity.field_name"
                if let Some(field_part) = item.strip_prefix("fields:source:") {
                    let (item_entity, item_field) = parse_qualified_name(field_part, default_source);
                    let ignore_id = format!("fields:source:{}", item_field);
                    for target_entity in &target_entities {
                        let pair_key = (item_entity.to_string(), target_entity.clone());
                        ignored_by_pair
                            .entry(pair_key)
                            .or_insert_with(std::collections::HashSet::new)
                            .insert(ignore_id.clone());
                    }
                } else if let Some(field_part) = item.strip_prefix("fields:target:") {
                    let (item_entity, item_field) = parse_qualified_name(field_part, default_target);
                    let ignore_id = format!("fields:target:{}", item_field);
                    for source_entity in &source_entities {
                        let pair_key = (source_entity.clone(), item_entity.to_string());
                        ignored_by_pair
                            .entry(pair_key)
                            .or_insert_with(std::collections::HashSet::new)
                            .insert(ignore_id.clone());
                    }
                }
            }

            // Save each entity pair's ignored items
            for ((src_entity, tgt_entity), items) in ignored_by_pair {
                if let Err(e) = config.set_ignored_items(&src_entity, &tgt_entity, &items).await {
                    log::error!("Failed to save ignored items for {}/{}: {}", src_entity, tgt_entity, e);
                }
            }
        },
        |_| Msg::CloseImportModal  // Dummy message, modal already closed
    )
}

/// Handle imported mappings loaded - update state and recompute matches
/// Note: C# parser returns HashMap<String, String>, convert to HashMap<String, Vec<String>> for 1-to-N support
pub fn handle_mappings_loaded(state: &mut State, mappings: HashMap<String, String>, filename: String) -> Command<Msg> {
    log::info!("Loading {} imported mappings from {}", mappings.len(), filename);
    log::debug!("Old mappings count: {}", state.imported_mappings.len());

    // Determine if we're in multi-entity mode
    let is_multi_entity = state.source_entities.len() > 1 || state.target_entities.len() > 1;

    // Convert HashMap<String, String> to HashMap<String, Vec<String>> for 1-to-N support
    // In multi-entity mode, expand simple names to qualified names for all entity combinations
    let mappings_vec: HashMap<String, Vec<String>> = if is_multi_entity {
        log::info!("Multi-entity mode: expanding {} mappings to all entity combinations", mappings.len());
        let mut expanded = HashMap::new();

        for (simple_src, simple_tgt) in &mappings {
            // Expand to all source/target entity combinations
            for source_entity in &state.source_entities {
                for target_entity in &state.target_entities {
                    let qualified_src = format!("{}.{}", source_entity, simple_src);
                    let qualified_tgt = format!("{}.{}", target_entity, simple_tgt);
                    expanded.entry(qualified_src)
                        .or_insert_with(Vec::new)
                        .push(qualified_tgt);
                }
            }
        }

        log::info!("Expanded {} simple mappings to {} qualified mappings", mappings.len(), expanded.len());
        expanded
    } else {
        // Single-entity mode: simple conversion
        mappings.iter()
            .map(|(k, v)| (k.clone(), vec![v.clone()]))
            .collect()
    };

    // Compute results by comparing old vs new mappings
    let old_mappings = &state.imported_mappings;
    let mut added = Vec::new();
    let mut updated = Vec::new();
    let mut removed = Vec::new();

    // Find added and updated mappings (iterate over expanded mappings)
    for (src, tgts) in &mappings_vec {
        for tgt in tgts {
            if let Some(old_tgts) = old_mappings.get(src) {
                // Check if this specific target already exists
                if !old_tgts.contains(tgt) {
                    updated.push((src.clone(), tgt.clone()));
                }
            } else {
                added.push((src.clone(), tgt.clone()));
            }
        }
    }

    // Don't track removed mappings - we're merging, not replacing
    // (Removed section commented out as we want to keep existing mappings)

    log::info!("Import results: {} added, {} updated, 0 removed (merging mode)", added.len(), updated.len());

    // Store results
    state.import_results = Some(super::super::app::ImportResults {
        filename: filename.clone(),
        added,
        updated,
        removed, // Empty vec - no removals in merge mode
        unparsed: vec![],  // TODO: capture unparsed lines from parser
    });
    state.show_import_results_modal = true;

    // MERGE imported mappings instead of replacing
    for (src, tgts) in mappings_vec {
        state.imported_mappings
            .entry(src)
            .or_insert_with(Vec::new)
            .extend(tgts);
    }

    // Update import source file (append if exists)
    if let Some(existing_file) = &state.import_source_file {
        state.import_source_file = Some(format!("{}, {}", existing_file, filename));
    } else {
        state.import_source_file = Some(filename.clone());
    }
    state.show_import_modal = false;

    // Recompute all matches with imported mappings
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
        // Single-entity mode: backwards compatible
        let first_source_entity = state.source_entities.first().cloned().unwrap_or_default();
        let first_target_entity = state.target_entities.first().cloned().unwrap_or_default();

        if let (Some(Resource::Success(source_metadata)), Some(Resource::Success(target_metadata))) = (
            state.source_metadata.get(&first_source_entity),
            state.target_metadata.get(&first_target_entity),
        ) {
            let (field_matches, relationship_matches, entity_matches, source_related_entities, target_related_entities) =
                recompute_all_matches(
                    source_metadata,
                    target_metadata,
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

    // Persist to config (async, don't wait)
    let source_entities = state.source_entities.clone();
    let target_entities = state.target_entities.clone();
    let imported = state.imported_mappings.clone();
    let file = state.import_source_file.clone();

    Command::perform(
        async move {
            let config = crate::global_config();
            if let Some(file) = file {
                // Helper to parse qualified names
                fn parse_qualified_name<'a>(name: &'a str, default_entity: &'a str) -> (&'a str, &'a str) {
                    if let Some((entity, field)) = name.split_once('.') {
                        (entity, field)
                    } else {
                        (default_entity, name)
                    }
                }

                let default_source = source_entities.first().map(|s| s.as_str()).unwrap_or("");
                let default_target = target_entities.first().map(|s| s.as_str()).unwrap_or("");

                // Save imported mappings - group by entity pair and use batch method
                // Group mappings by entity pair
                let mut mappings_by_pair: std::collections::HashMap<(String, String), std::collections::HashMap<String, Vec<String>>> = std::collections::HashMap::new();

                for (src, tgts) in imported {
                    let (src_entity, src_field) = parse_qualified_name(&src, default_source);
                    for tgt in &tgts {
                        let (tgt_entity, tgt_field) = parse_qualified_name(tgt, default_target);
                        let pair_key = (src_entity.to_string(), tgt_entity.to_string());

                        mappings_by_pair
                            .entry(pair_key)
                            .or_insert_with(std::collections::HashMap::new)
                            .entry(src_field.to_string())
                            .or_insert_with(Vec::new)
                            .push(tgt_field.to_string());
                    }
                }

                // Save each entity pair's mappings
                for ((src_entity, tgt_entity), mappings) in mappings_by_pair {
                    if let Err(e) = config.set_imported_mappings(&src_entity, &tgt_entity, &mappings, &file).await {
                        log::error!("Failed to save imported mappings for {}/{}: {}", src_entity, tgt_entity, e);
                    }
                }
            }
        },
        |_| Msg::CloseImportModal  // Dummy message, already closed
    )
}

/// Clear imported mappings
pub fn handle_clear_imported(state: &mut State) -> Command<Msg> {
    log::info!("Clearing imported mappings");

    state.imported_mappings.clear();
    state.import_source_file = None;

    // Close results modal if it's open
    state.show_import_results_modal = false;
    state.import_results = None;

    // Recompute matches without imported mappings
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
                &state.imported_mappings, // Now empty
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

        if let (Some(Resource::Success(source_metadata)), Some(Resource::Success(target_metadata))) = (
            state.source_metadata.get(&first_source_entity),
            state.target_metadata.get(&first_target_entity),
        ) {
            let (field_matches, relationship_matches, entity_matches, source_related_entities, target_related_entities) =
                recompute_all_matches(
                    source_metadata,
                    target_metadata,
                    &state.field_mappings,
                    &state.imported_mappings, // Now empty
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

    // Persist cleared state to config for all entity pairs
    let source_entities = state.source_entities.clone();
    let target_entities = state.target_entities.clone();

    Command::perform(
        async move {
            let config = crate::global_config();
            for source_entity in &source_entities {
                for target_entity in &target_entities {
                    if let Err(e) = config.clear_imported_mappings(source_entity, target_entity).await {
                        log::error!("Failed to clear imported mappings for {}/{}: {}", source_entity, target_entity, e);
                    }
                }
            }
        },
        |_| Msg::CloseImportModal
    )
}

/// Update viewport height for file browser scrolling
pub fn handle_set_viewport_height(state: &mut State, height: usize) -> Command<Msg> {
    let item_count = state.import_file_browser.entries().len();
    let list_state = state.import_file_browser.list_state_mut();
    list_state.set_viewport_height(height);
    list_state.update_scroll(height, item_count);
    Command::None
}

/// Close the import results modal
pub fn handle_close_results_modal(state: &mut State) -> Command<Msg> {
    state.show_import_results_modal = false;
    Command::None
}

/// Handle navigation in import results list
pub fn handle_results_navigate(state: &mut State, key: KeyCode) -> Command<Msg> {
    if let Some(results) = &state.import_results {
        // Calculate total number of lines
        let mut line_count = 2; // header + blank line

        if !results.added.is_empty() {
            line_count += 1 + results.added.len() + 1;
        }
        if !results.updated.is_empty() {
            line_count += 1 + results.updated.len() + 1;
        }
        if !results.removed.is_empty() {
            line_count += 1 + results.removed.len() + 1;
        }
        if !results.unparsed.is_empty() {
            line_count += 1 + results.unparsed.len();
        }

        // Use approximate viewport height - the actual height is set by on_render
        state.import_results_list.handle_key(key, line_count, 20);
    }
    Command::None
}

/// Handle selecting an item in import results list (no-op for read-only list)
pub fn handle_results_select(state: &mut State, index: usize) -> Command<Msg> {
    // Calculate total number of lines (same as in handle_results_set_viewport_height)
    let line_count = if let Some(results) = &state.import_results {
        let mut count = 2; // header + blank line
        if !results.added.is_empty() {
            count += 1 + results.added.len() + 1;
        }
        if !results.updated.is_empty() {
            count += 1 + results.updated.len() + 1;
        }
        if !results.removed.is_empty() {
            count += 1 + results.removed.len() + 1;
        }
        if !results.unparsed.is_empty() {
            count += 1 + results.unparsed.len();
        }
        count
    } else {
        0
    };

    state.import_results_list.select_and_scroll(Some(index), line_count);
    Command::None
}

/// Update viewport height for results list scrolling
pub fn handle_results_set_viewport_height(state: &mut State, height: usize) -> Command<Msg> {
    if let Some(results) = &state.import_results {
        // Calculate total number of lines
        let mut line_count = 2; // header + blank line

        if !results.added.is_empty() {
            line_count += 1 + results.added.len() + 1; // header + items + blank
        }
        if !results.updated.is_empty() {
            line_count += 1 + results.updated.len() + 1;
        }
        if !results.removed.is_empty() {
            line_count += 1 + results.removed.len() + 1;
        }
        if !results.unparsed.is_empty() {
            line_count += 1 + results.unparsed.len();
        }

        state.import_results_list.set_viewport_height(height);
        state.import_results_list.update_scroll(height, line_count);
    }
    Command::None
}
