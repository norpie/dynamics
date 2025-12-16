//! Transfer Preview app - displays resolved records after transform

use std::collections::HashMap;

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::api::metadata::FieldMetadata;
use crate::config::repository::transfer::get_transfer_config;
use crate::transfer::{ExpandTree, LookupBindingContext, RecordAction, ResolvedTransfer, TransferConfig, TransformEngine};
use crate::tui::resource::Resource;
use crate::tui::{App, AppId, Command, LayeredView, Subscription};

use super::state::{Msg, PreviewParams, RecordDetailState, RecordFilter, State};
use super::view;

/// Transfer Preview App - shows resolved records before execution
pub struct TransferPreviewApp;

impl crate::tui::AppState for State {}

impl App for TransferPreviewApp {
    type State = State;
    type Msg = Msg;
    type InitParams = PreviewParams;

    fn init(params: PreviewParams) -> (State, Command<Msg>) {
        let state = State {
            config_name: params.config_name.clone(),
            source_env: params.source_env.clone(),
            target_env: params.target_env.clone(),
            resolved: Resource::Loading,
            ..Default::default()
        };

        // First load config to know which entities to fetch
        let cmd = Command::perform(
            load_config(params.config_name),
            Msg::ConfigLoaded,
        );

        (state, cmd)
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            // Data loading - Step 1: Config loaded, now fetch source AND target metadata
            Msg::ConfigLoaded(result) => {
                match result {
                    Ok(config) => {
                        // Get unique source entities that need metadata
                        let source_entities: Vec<String> = config
                            .entity_mappings
                            .iter()
                            .map(|m| m.source_entity.clone())
                            .collect::<std::collections::HashSet<_>>()
                            .into_iter()
                            .collect();

                        // Get unique target entities that need metadata (including resolver source entities)
                        let target_entities: Vec<String> = config
                            .entity_mappings
                            .iter()
                            .map(|m| m.target_entity.clone())
                            .chain(config.resolvers.iter().map(|r| r.source_entity.clone()))
                            .collect::<std::collections::HashSet<_>>()
                            .into_iter()
                            .collect();

                        state.pending_source_metadata_fetches = source_entities.len();
                        state.pending_target_metadata_fetches = target_entities.len();
                        state.config = Some(config.clone());

                        log::info!(
                            "Fetching metadata for {} source + {} target entities before records",
                            source_entities.len(),
                            target_entities.len()
                        );

                        // Build parallel fetch tasks for source AND target metadata
                        let mut builder = Command::perform_parallel()
                            .with_title("Fetching Entity Metadata");

                        // Track how many source tasks we add (to distinguish in callback)
                        let num_source_tasks = source_entities.len();

                        for entity in source_entities {
                            let env = config.source_env.clone();
                            let e = entity.clone();
                            builder = builder.add_task(
                                format!("Source: {}", e),
                                fetch_source_metadata(env, e),
                            );
                        }

                        for entity in target_entities {
                            let env = config.target_env.clone();
                            let e = entity.clone();
                            builder = builder.add_task(
                                format!("Target: {}", e),
                                fetch_target_metadata(env, e),
                            );
                        }

                        builder
                            .on_complete(AppId::TransferPreview)
                            .build(move |task_idx, result| {
                                if task_idx < num_source_tasks {
                                    // Source metadata returns (entity_name, fields)
                                    let data = result
                                        .downcast::<Result<(String, Vec<crate::api::metadata::FieldMetadata>), String>>()
                                        .unwrap();
                                    Msg::SourceMetadataResult(*data)
                                } else {
                                    // Target metadata returns (entity_name, fields, entity_set_name)
                                    let data = result
                                        .downcast::<Result<(String, Vec<crate::api::metadata::FieldMetadata>, String), String>>()
                                        .unwrap();
                                    Msg::TargetMetadataResult(*data)
                                }
                            })
                    }
                    Err(e) => {
                        state.resolved = Resource::Failure(e);
                        Command::None
                    }
                }
            }

            // Data loading - Step 1b: Source metadata loaded, accumulate
            Msg::SourceMetadataResult(result) => {
                match result {
                    Ok((entity_name, fields)) => {
                        log::info!(
                            "[{}] Source metadata loaded: {} fields ({} lookups)",
                            entity_name,
                            fields.len(),
                            fields.iter().filter(|f| f.related_entity.is_some()).count()
                        );
                        state.source_metadata.insert(entity_name, fields);
                        state.pending_source_metadata_fetches = state.pending_source_metadata_fetches.saturating_sub(1);

                        // Check if BOTH source and target metadata are loaded
                        if state.pending_source_metadata_fetches == 0 && state.pending_target_metadata_fetches == 0 {
                            log::info!("All metadata loaded, triggering record fetch");
                            return Command::perform(async { () }, |_| Msg::FetchRecords);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to fetch source metadata: {}", e);
                        state.resolved = Resource::Failure(e);
                        state.pending_source_metadata_fetches = 0;
                        state.pending_target_metadata_fetches = 0;
                    }
                }
                Command::None
            }

            // Data loading - Step 1c: Target metadata loaded, accumulate
            Msg::TargetMetadataResult(result) => {
                match result {
                    Ok((entity_name, fields, entity_set)) => {
                        log::info!(
                            "[{}] Target metadata loaded: {} fields ({} lookups), entity_set={}",
                            entity_name,
                            fields.len(),
                            fields.iter().filter(|f| f.related_entity.is_some()).count(),
                            entity_set
                        );
                        state.target_metadata.insert(entity_name.clone(), fields);
                        state.entity_set_map.insert(entity_name, entity_set);
                        state.pending_target_metadata_fetches = state.pending_target_metadata_fetches.saturating_sub(1);

                        // Check if BOTH source and target metadata are loaded
                        if state.pending_source_metadata_fetches == 0 && state.pending_target_metadata_fetches == 0 {
                            log::info!("All metadata loaded, triggering record fetch");
                            return Command::perform(async { () }, |_| Msg::FetchRecords);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to fetch target metadata: {}", e);
                        state.resolved = Resource::Failure(e);
                        state.pending_source_metadata_fetches = 0;
                        state.pending_target_metadata_fetches = 0;
                    }
                }
                Command::None
            }

            // Data loading - Step 2: Fetch records (after both source and target metadata are loaded)
            Msg::FetchRecords => {
                if let Some(ref config) = state.config {
                    // Build parallel fetch tasks for loading screen
                    let mut builder = Command::perform_parallel()
                        .with_title("Fetching Records");

                    let num_entities = config.entity_mappings.len();

                    // Add source fetch tasks with progress reporting
                    for mapping in &config.entity_mappings {
                        let entity = mapping.source_entity.clone();
                        let env = config.source_env.clone();

                        // Collect source fields from transforms + primary key
                        let mut source_fields: Vec<String> = mapping
                            .field_mappings
                            .iter()
                            .flat_map(|fm| fm.transform.source_fields())
                            .map(|s| s.to_string())
                            .collect();
                        source_fields.push(format!("{}id", entity)); // Primary key

                        // Add _fieldname_value for lookup fields (from source metadata)
                        if let Some(fields) = state.source_metadata.get(&mapping.source_entity) {
                            let lookup_fields: std::collections::HashSet<&str> = fields
                                .iter()
                                .filter(|f| f.related_entity.is_some())
                                .map(|f| f.logical_name.as_str())
                                .collect();

                            let lookup_value_fields: Vec<String> = source_fields
                                .iter()
                                .filter(|f| lookup_fields.contains(f.as_str()))
                                .map(|f| format!("_{}_value", f))
                                .collect();
                            source_fields.extend(lookup_value_fields);
                        }

                        source_fields.sort();
                        source_fields.dedup();

                        // Build expand tree for nested lookup traversals
                        let mut expand_tree = ExpandTree::new();
                        for fm in &mapping.field_mappings {
                            expand_tree.add_transform(&fm.transform);
                        }
                        let expands = expand_tree.build_expand_clauses();

                        log::info!(
                            "[{}] Source fetch will select {} fields, expand {} lookups",
                            entity,
                            source_fields.len(),
                            expands.len()
                        );
                        if !expands.is_empty() {
                            log::info!("[{}] Expands: {:?}", entity, expands);
                        }

                        builder = builder.add_task_with_progress(
                            format!("Source: {}", entity),
                            move |progress| fetch_entity_records(env, entity, true, source_fields, expands, Some(progress), false), // use cache
                        );
                    }

                    // Add target fetch tasks with progress reporting
                    for mapping in &config.entity_mappings {
                        let entity = mapping.target_entity.clone();
                        let env = config.target_env.clone();

                        // Collect target fields + primary key
                        let mut target_fields: Vec<String> = mapping
                            .field_mappings
                            .iter()
                            .map(|fm| fm.target_field.clone())
                            .collect();
                        target_fields.push(format!("{}id", entity)); // Primary key

                        // Replace lookup fields with _fieldname_value (from target metadata)
                        if let Some(fields) = state.target_metadata.get(&mapping.target_entity) {
                            let lookup_fields: std::collections::HashSet<&str> = fields
                                .iter()
                                .filter(|f| f.related_entity.is_some())
                                .map(|f| f.logical_name.as_str())
                                .collect();

                            // Replace lookup field names with _value versions
                            target_fields = target_fields
                                .into_iter()
                                .map(|f| {
                                    if lookup_fields.contains(f.as_str()) {
                                        format!("_{}_value", f)
                                    } else {
                                        f
                                    }
                                })
                                .collect();
                        }

                        target_fields.sort();
                        target_fields.dedup();

                        log::info!("[{}] Target fetch will select {} fields", entity, target_fields.len());

                        // Target fetch doesn't need expands - we compare final values
                        let no_expands: Vec<String> = vec![];

                        builder = builder.add_task_with_progress(
                            format!("Target: {}", entity),
                            move |progress| fetch_entity_records(env, entity, false, target_fields, no_expands, Some(progress), false), // use cache
                        );
                    }

                    // Add resolver source entity fetches (from target environment)
                    // These are needed to build the resolver lookup tables
                    let resolver_entities: Vec<String> = config
                        .resolvers
                        .iter()
                        .map(|r| r.source_entity.clone())
                        .filter(|e| !config.entity_mappings.iter().any(|m| &m.target_entity == e))
                        .collect::<std::collections::HashSet<_>>()
                        .into_iter()
                        .collect();

                    for resolver in &config.resolvers {
                        // Skip if already fetched as part of entity mappings
                        if config.entity_mappings.iter().any(|m| m.target_entity == resolver.source_entity) {
                            continue;
                        }

                        let entity = resolver.source_entity.clone();
                        let env = config.target_env.clone();
                        let match_field = resolver.match_field.clone();

                        // For resolver entities, we only need the primary key and match field
                        let mut resolver_fields = vec![
                            format!("{}id", entity),
                            match_field,
                        ];
                        resolver_fields.sort();
                        resolver_fields.dedup();

                        log::info!(
                            "[{}] Resolver entity fetch will select {} fields: {:?}",
                            entity,
                            resolver_fields.len(),
                            resolver_fields
                        );

                        let no_expands: Vec<String> = vec![];

                        builder = builder.add_task_with_progress(
                            format!("Resolver: {}", entity),
                            move |progress| fetch_entity_records(env, entity, false, resolver_fields, no_expands, Some(progress), false),
                        );
                    }

                    // Track how many fetches we're waiting for (source + target for each entity + resolver entities)
                    state.pending_fetches = num_entities * 2 + resolver_entities.len();

                    return builder
                        .on_complete(AppId::TransferPreview)
                        .build(|_task_idx, result| {
                            let data = result
                                .downcast::<Result<(String, bool, Vec<serde_json::Value>), String>>()
                                .unwrap();
                            Msg::FetchResult(*data)
                        });
                }
                Command::None
            }

            // Data loading - Step 3: Each fetch result comes in individually
            Msg::FetchResult(result) => {
                match result {
                    Ok((entity_name, is_source, records)) => {
                        if is_source {
                            state.source_data.insert(entity_name, records);
                        } else {
                            state.target_data.insert(entity_name, records);
                        }

                        state.pending_fetches = state.pending_fetches.saturating_sub(1);

                        // Just accumulate data - transform runs after loading screen returns
                        if state.pending_fetches == 0 {
                            log::info!("All fetches complete, data ready for transform");
                            // Mark as loading - transform will run when we become active again
                            state.resolved = Resource::Loading;
                        }
                    }
                    Err(e) => {
                        state.resolved = Resource::Failure(e);
                        state.pending_fetches = 0;
                    }
                }
                Command::None
            }

            // Data loading - Step 4: Run transform after returning from loading screen
            Msg::RunTransform => {
                if let Some(ref config) = state.config {
                    // Check if we need to fetch metadata for target entities
                    let missing_target_metadata: Vec<String> = config
                        .entity_mappings
                        .iter()
                        .map(|m| m.target_entity.clone())
                        .filter(|e| !state.target_metadata.contains_key(e))
                        .collect();

                    if !missing_target_metadata.is_empty() {
                        // Need to fetch target entity metadata first
                        log::info!(
                            "Fetching metadata for {} target entities",
                            missing_target_metadata.len()
                        );

                        state.pending_metadata_fetches = missing_target_metadata.len();

                        let target_env = config.target_env.clone();
                        let mut builder = Command::perform_parallel()
                            .with_title("Fetching Entity Metadata");

                        for entity in missing_target_metadata {
                            let env = target_env.clone();
                            let e = entity.clone();
                            builder = builder.add_task(
                                format!("Metadata: {}", e),
                                fetch_entity_metadata(env, e),
                            );
                        }

                        return builder
                            .on_complete(AppId::TransferPreview)
                            .build(|_task_idx, result| {
                                let data = result
                                    .downcast::<Result<(String, Vec<crate::api::metadata::FieldMetadata>, String), String>>()
                                    .unwrap();
                                Msg::MetadataResult(*data)
                            });
                    }

                    // Check if we need to fetch metadata for lookup target entities
                    let mut missing_lookup_targets: Vec<String> = Vec::new();
                    for mapping in &config.entity_mappings {
                        let mapped_fields: std::collections::HashSet<&str> = mapping
                            .field_mappings
                            .iter()
                            .map(|fm| fm.target_field.as_str())
                            .collect();

                        if let Some(fields) = state.target_metadata.get(&mapping.target_entity) {
                            for field in fields {
                                if mapped_fields.contains(field.logical_name.as_str()) {
                                    if let Some(ref related) = field.related_entity {
                                        if !state.entity_set_map.contains_key(related)
                                            && !missing_lookup_targets.contains(related)
                                        {
                                            missing_lookup_targets.push(related.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if !missing_lookup_targets.is_empty() {
                        log::info!(
                            "Fetching metadata for {} lookup targets",
                            missing_lookup_targets.len()
                        );

                        state.pending_metadata_fetches = missing_lookup_targets.len();

                        let target_env = config.target_env.clone();
                        let mut builder = Command::perform_parallel()
                            .with_title("Fetching Lookup Target Metadata");

                        for entity in missing_lookup_targets {
                            let env = target_env.clone();
                            let e = entity.clone();
                            builder = builder.add_task(
                                format!("Metadata: {}", e),
                                fetch_entity_metadata(env, e),
                            );
                        }

                        return builder
                            .on_complete(AppId::TransferPreview)
                            .build(|_task_idx, result| {
                                let data = result
                                    .downcast::<Result<(String, Vec<crate::api::metadata::FieldMetadata>, String), String>>()
                                    .unwrap();
                                Msg::MetadataResult(*data)
                            });
                    }

                    // All metadata loaded - proceed with transform
                    // Build primary keys map for both source and target entities
                    let mut primary_keys: HashMap<String, String> = HashMap::new();
                    for m in &config.entity_mappings {
                        primary_keys.insert(m.source_entity.clone(), format!("{}id", m.source_entity));
                        primary_keys.insert(m.target_entity.clone(), format!("{}id", m.target_entity));
                    }
                    // Also add resolver source entities
                    for r in &config.resolvers {
                        primary_keys.insert(r.source_entity.clone(), format!("{}id", r.source_entity));
                    }

                    // Run transform
                    let mut resolved = TransformEngine::transform_all(
                        config,
                        &state.source_data,
                        &state.target_data,
                        &primary_keys,
                    );

                    // Build lookup context for each entity (only for mapped fields)
                    for entity in &mut resolved.entities {
                        // Set entity_set_name for API calls (OData requires EntitySetName, not LogicalName)
                        if let Some(entity_set) = state.entity_set_map.get(&entity.entity_name) {
                            entity.set_entity_set_name(entity_set.clone());
                        }

                        // Get the mapped target fields for this entity
                        let mapped_fields: std::collections::HashSet<&str> = config
                            .entity_mappings
                            .iter()
                            .find(|m| m.target_entity == entity.entity_name)
                            .map(|m| {
                                m.field_mappings
                                    .iter()
                                    .map(|fm| fm.target_field.as_str())
                                    .collect()
                            })
                            .unwrap_or_default();

                        if let Some(all_fields) = state.target_metadata.get(&entity.entity_name) {
                            // Filter to only include mapped fields
                            let fields_to_use: Vec<_> = all_fields
                                .iter()
                                .filter(|f| mapped_fields.contains(f.logical_name.as_str()))
                                .cloned()
                                .collect();

                            match LookupBindingContext::from_field_metadata(&fields_to_use, &state.entity_set_map) {
                                Ok(ctx) => {
                                    log::info!(
                                        "[{}] Built lookup context with {} lookup fields (from {} mapped fields)",
                                        entity.entity_name,
                                        ctx.lookups.len(),
                                        fields_to_use.len()
                                    );
                                    entity.set_lookup_context(ctx);
                                }
                                Err(e) => {
                                    log::error!(
                                        "[{}] Failed to build lookup context: {}",
                                        entity.entity_name,
                                        e
                                    );
                                    state.resolved = Resource::Failure(format!(
                                        "Failed to build lookup context for {}: {}",
                                        entity.entity_name, e
                                    ));
                                    return Command::None;
                                }
                            }
                        }
                    }

                    // If refreshing, merge dirty records from previous state
                    if state.is_refreshing {
                        if let Resource::Success(ref old_resolved) = state.resolved {
                            merge_dirty_records(&mut resolved, old_resolved);
                            log::info!("Merged dirty records from previous state");
                        }
                        state.is_refreshing = false;
                    }

                    log::info!(
                        "Transform complete: {} records ({} create, {} update, {} nochange, {} target-only, {} skip, {} error)",
                        resolved.total_records(),
                        resolved.create_count(),
                        resolved.update_count(),
                        resolved.nochange_count(),
                        resolved.target_only_count(),
                        resolved.skip_count(),
                        resolved.error_count()
                    );

                    // Keep source_data and target_data for future refresh comparisons
                    // (Previously cleared here, but we need them for refresh)

                    state.resolved = Resource::Success(resolved);
                }
                Command::None
            }

            // Data loading - Step 3b: Metadata result (called for each entity after LoadingScreen completes)
            Msg::MetadataResult(result) => {
                match result {
                    Ok((entity_name, fields, entity_set)) => {
                        log::info!(
                            "[{}] Metadata loaded: {} fields, entity_set={}",
                            entity_name,
                            fields.len(),
                            entity_set
                        );
                        state.target_metadata.insert(entity_name.clone(), fields.clone());
                        state.entity_set_map.insert(entity_name, entity_set);

                        // Track pending results - we get one MetadataResult per task from perform_parallel
                        state.pending_metadata_fetches = state.pending_metadata_fetches.saturating_sub(1);

                        // When all results processed, trigger RunTransform to check for more metadata needs
                        if state.pending_metadata_fetches == 0 {
                            // Let RunTransform handle checking for lookup targets
                            return Command::perform(async { () }, |_| Msg::RunTransform);
                        }
                    }
                    Err(e) => {
                        log::error!("Metadata fetch failed: {}", e);
                        state.resolved = Resource::Failure(format!("Failed to fetch metadata: {}", e));
                        state.pending_metadata_fetches = 0;
                    }
                }
                Command::None
            }

            Msg::ResolvedLoaded(result) => {
                state.resolved = match result {
                    Ok(resolved) => Resource::Success(resolved),
                    Err(e) => Resource::Failure(e),
                };
                Command::None
            }

            // Navigation within table
            Msg::ListEvent(event) => {
                // Count filtered records for proper navigation bounds
                let item_count = if let Resource::Success(resolved) = &state.resolved {
                    let query = state.search_field.value().to_lowercase();
                    resolved.entities.get(state.current_entity_idx)
                        .map(|e| {
                            // Count only records matching current filter and search
                            e.records.iter()
                                .filter(|r| state.filter.matches(r.action))
                                .filter(|r| {
                                    if query.is_empty() {
                                        return true;
                                    }
                                    if r.source_id.to_string().to_lowercase().contains(&query) {
                                        return true;
                                    }
                                    r.fields.values().any(|v| {
                                        format!("{:?}", v).to_lowercase().contains(&query)
                                    })
                                })
                                .count()
                        })
                        .unwrap_or(0)
                } else {
                    0
                };
                state.list_state.handle_event(event, item_count, state.viewport_height);
                Command::None
            }

            // Viewport height changed (for virtual scrolling)
            Msg::ViewportHeightChanged(height) => {
                state.viewport_height = height;
                Command::None
            }

            Msg::NextEntity => {
                if let Resource::Success(resolved) = &state.resolved {
                    if state.current_entity_idx + 1 < resolved.entities.len() {
                        state.current_entity_idx += 1;
                        state.list_state = crate::tui::widgets::ListState::with_selection();
                    }
                }
                Command::None
            }

            Msg::PrevEntity => {
                if state.current_entity_idx > 0 {
                    state.current_entity_idx -= 1;
                    state.list_state = crate::tui::widgets::ListState::with_selection();
                }
                Command::None
            }

            Msg::SelectEntity(idx) => {
                if let Resource::Success(resolved) = &state.resolved {
                    if idx < resolved.entities.len() {
                        state.current_entity_idx = idx;
                        state.list_state = crate::tui::widgets::ListState::with_selection();
                    }
                }
                Command::None
            }

            // Filtering
            Msg::SetFilter(filter) => {
                state.filter = filter;
                state.list_state = crate::tui::widgets::ListState::with_selection();
                Command::None
            }

            Msg::CycleFilter => {
                state.filter = state.filter.next();
                state.list_state = crate::tui::widgets::ListState::with_selection();
                Command::None
            }

            Msg::SearchChanged(event) => {
                state.search_field.handle_event(event, None);
                // Reset list selection when search changes
                state.list_state = crate::tui::widgets::ListState::with_selection();
                Command::None
            }

            // Record actions
            Msg::ToggleSkip => {
                // Toggle skip on currently selected record
                if let Some(idx) = state.list_state.selected() {
                    if let Resource::Success(ref mut resolved) = state.resolved {
                        if let Some(entity) = resolved.entities.get_mut(state.current_entity_idx) {
                            // Get filtered record indices
                            let filter = state.filter;
                            let query = state.search_field.value().to_lowercase();

                            let mut match_idx = 0;
                            let mut target_source_id = None;
                            for record in &entity.records {
                                if !filter.matches(record.action) {
                                    continue;
                                }
                                let matches_search = if query.is_empty() {
                                    true
                                } else if record.source_id.to_string().to_lowercase().contains(&query) {
                                    true
                                } else {
                                    record.fields.values().any(|v| format!("{:?}", v).to_lowercase().contains(&query))
                                };
                                if !matches_search {
                                    continue;
                                }

                                if match_idx == idx {
                                    target_source_id = Some(record.source_id);
                                    break;
                                }
                                match_idx += 1;
                            }

                            // Toggle skip on found record
                            if let Some(source_id) = target_source_id {
                                if let Some(record) = entity.records.iter_mut().find(|r| r.source_id == source_id) {
                                    if record.action == RecordAction::Skip {
                                        // Restore original action (we'll use NoChange as fallback)
                                        // In a real implementation, we'd track original_action per record
                                        record.action = RecordAction::NoChange;
                                    } else {
                                        record.action = RecordAction::Skip;
                                    }
                                }
                                entity.mark_dirty(source_id);
                            }
                        }
                    }
                }
                Command::None
            }

            Msg::ViewDetails => {
                if let Some(idx) = state.list_state.selected() {
                    if let Resource::Success(resolved) = &state.resolved {
                        if let Some(entity) = resolved.entities.get(state.current_entity_idx) {
                            // Get filtered records to find the actual record
                            let filtered: Vec<_> = entity.records.iter()
                                .filter(|r| state.filter.matches(r.action))
                                .filter(|r| {
                                    let query = state.search_field.value().to_lowercase();
                                    if query.is_empty() { return true; }
                                    if r.source_id.to_string().to_lowercase().contains(&query) { return true; }
                                    r.fields.values().any(|v| format!("{:?}", v).to_lowercase().contains(&query))
                                })
                                .collect();

                            if let Some(record) = filtered.get(idx) {
                                state.record_detail_state = Some(RecordDetailState::new(
                                    idx,
                                    record.action,
                                    &entity.field_names,
                                    &record.fields,
                                ));
                                state.active_modal = Some(super::state::PreviewModal::RecordDetails {
                                    record_idx: idx,
                                });
                            }
                        }
                    }
                }
                Command::None
            }

            Msg::EditRecord => {
                if let Some(idx) = state.list_state.selected() {
                    if let Resource::Success(resolved) = &state.resolved {
                        if let Some(entity) = resolved.entities.get(state.current_entity_idx) {
                            let filtered: Vec<_> = entity.records.iter()
                                .filter(|r| state.filter.matches(r.action))
                                .filter(|r| {
                                    let query = state.search_field.value().to_lowercase();
                                    if query.is_empty() { return true; }
                                    if r.source_id.to_string().to_lowercase().contains(&query) { return true; }
                                    r.fields.values().any(|v| format!("{:?}", v).to_lowercase().contains(&query))
                                })
                                .collect();

                            if let Some(record) = filtered.get(idx) {
                                let mut detail_state = RecordDetailState::new(
                                    idx,
                                    record.action,
                                    &entity.field_names,
                                    &record.fields,
                                );
                                detail_state.editing = true; // Start in edit mode
                                state.record_detail_state = Some(detail_state);
                                state.active_modal = Some(super::state::PreviewModal::RecordDetails {
                                    record_idx: idx,
                                });
                            }
                        }
                    }
                }
                Command::None
            }

            Msg::SaveRecord => {
                state.active_modal = None;
                state.record_detail_state = None;
                Command::None
            }

            // Record details modal handlers
            Msg::ToggleEditMode => {
                if let Some(ref mut detail) = state.record_detail_state {
                    detail.editing = !detail.editing;
                }
                Command::None
            }

            Msg::RecordDetailActionChanged(action) => {
                if let Some(ref mut detail) = state.record_detail_state {
                    detail.current_action = action;
                }
                Command::None
            }

            Msg::RecordDetailFieldNavigate(key) => {
                if let Some(ref mut detail) = state.record_detail_state {
                    // Only navigate when not actively editing a field
                    if !detail.editing_field {
                        let field_count = detail.fields.len();
                        if field_count > 0 {
                            match key {
                                crossterm::event::KeyCode::Up => {
                                    if detail.focused_field_idx > 0 {
                                        detail.focused_field_idx -= 1;
                                    }
                                }
                                crossterm::event::KeyCode::Down => {
                                    if detail.focused_field_idx + 1 < field_count {
                                        detail.focused_field_idx += 1;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Command::None
            }

            Msg::StartFieldEdit => {
                if let Some(ref mut detail) = state.record_detail_state {
                    if detail.editing && !detail.editing_field {
                        detail.editing_field = true;
                    }
                }
                Command::None
            }

            Msg::FocusedFieldInput(event) => {
                if let Some(ref mut detail) = state.record_detail_state {
                    if detail.editing_field {
                        if let Some(field) = detail.fields.get_mut(detail.focused_field_idx) {
                            field.input.handle_event(event, None);
                            field.update_dirty();
                        }
                    }
                }
                Command::None
            }

            Msg::FinishFieldEdit => {
                if let Some(ref mut detail) = state.record_detail_state {
                    if detail.editing_field {
                        detail.editing_field = false;
                        // Move to next field
                        if detail.focused_field_idx + 1 < detail.fields.len() {
                            detail.focused_field_idx += 1;
                        }
                    }
                }
                Command::None
            }

            Msg::CancelFieldEdit => {
                if let Some(ref mut detail) = state.record_detail_state {
                    if detail.editing_field {
                        // Reset field to original value
                        if let Some(field) = detail.fields.get_mut(detail.focused_field_idx) {
                            field.reset();
                        }
                        detail.editing_field = false;
                    }
                }
                Command::None
            }

            Msg::SaveRecordEdits => {
                // Apply changes to the resolved record
                if let Some(ref detail) = state.record_detail_state {
                    if let Resource::Success(ref mut resolved) = state.resolved {
                        if let Some(entity) = resolved.entities.get_mut(state.current_entity_idx) {
                            // Find the actual record by filtering the same way
                            let filter = state.filter;
                            let query = state.search_field.value().to_lowercase();
                            let record_idx = detail.record_idx;

                            // First pass: find the record's source_id
                            let mut match_idx = 0;
                            let mut target_source_id = None;
                            for record in &entity.records {
                                if !filter.matches(record.action) {
                                    continue;
                                }
                                let matches_search = if query.is_empty() {
                                    true
                                } else if record.source_id.to_string().to_lowercase().contains(&query) {
                                    true
                                } else {
                                    record.fields.values().any(|v| format!("{:?}", v).to_lowercase().contains(&query))
                                };
                                if !matches_search {
                                    continue;
                                }

                                if match_idx == record_idx {
                                    target_source_id = Some(record.source_id);
                                    break;
                                }
                                match_idx += 1;
                            }

                            // Second pass: apply changes to the found record
                            if let Some(source_id) = target_source_id {
                                // Find and update the record
                                if let Some(record) = entity.records.iter_mut().find(|r| r.source_id == source_id) {
                                    // Apply action change
                                    if detail.current_action != detail.original_action {
                                        record.action = detail.current_action;
                                        if detail.current_action != RecordAction::Error {
                                            record.error = None;
                                        }
                                    }

                                    // Apply field changes
                                    for field_state in &detail.fields {
                                        if field_state.is_dirty {
                                            let new_value = field_state.parse_value();
                                            record.fields.insert(field_state.field_name.clone(), new_value);
                                        }
                                    }
                                }

                                // Mark as dirty in entity
                                entity.mark_dirty(source_id);
                            }
                        }
                    }
                }

                state.active_modal = None;
                state.record_detail_state = None;
                Command::None
            }

            Msg::CancelRecordEdits => {
                // Just close without saving
                if let Some(ref mut detail) = state.record_detail_state {
                    if detail.editing {
                        // If in edit mode, switch back to view mode and reset
                        detail.reset_all();
                        detail.editing = false;
                    } else {
                        // If in view mode, close the modal
                        state.active_modal = None;
                        state.record_detail_state = None;
                    }
                }
                Command::None
            }

            // Multi-selection
            Msg::ListMultiSelect(event) => {
                if let Resource::Success(resolved) = &state.resolved {
                    if let Some(entity) = resolved.entities.get(state.current_entity_idx) {
                        // Count filtered records for item_count
                        let query = state.search_field.value().to_lowercase();
                        let item_count = entity.records.iter()
                            .filter(|r| state.filter.matches(r.action))
                            .filter(|r| {
                                if query.is_empty() { return true; }
                                if r.source_id.to_string().to_lowercase().contains(&query) { return true; }
                                r.fields.values().any(|v| format!("{:?}", v).to_lowercase().contains(&query))
                            })
                            .count();
                        state.list_state.handle_event(event, item_count, state.viewport_height);
                    }
                }
                Command::None
            }

            // Bulk actions
            Msg::OpenBulkActions => {
                // Reset to defaults when opening
                state.bulk_action_scope = super::state::BulkActionScope::Filtered;
                state.bulk_action_selection = super::state::BulkAction::MarkSkip;
                state.active_modal = Some(super::state::PreviewModal::BulkActions);
                Command::None
            }

            Msg::SetBulkActionScope(scope) => {
                state.bulk_action_scope = scope;
                Command::None
            }

            Msg::SetBulkAction(action) => {
                state.bulk_action_selection = action;
                Command::None
            }

            Msg::ConfirmBulkAction => {
                // Apply bulk action to records based on scope
                if let Resource::Success(ref mut resolved) = state.resolved {
                    if let Some(entity) = resolved.entities.get_mut(state.current_entity_idx) {
                        let filter = state.filter;
                        let query = state.search_field.value().to_lowercase();

                        // Get indices to apply action to based on scope
                        let indices_to_apply: Vec<usize> = match state.bulk_action_scope {
                            super::state::BulkActionScope::All => {
                                (0..entity.records.len()).collect()
                            }
                            super::state::BulkActionScope::Filtered => {
                                entity.records.iter()
                                    .enumerate()
                                    .filter(|(_, r)| filter.matches(r.action))
                                    .filter(|(_, r)| {
                                        if query.is_empty() { return true; }
                                        if r.source_id.to_string().to_lowercase().contains(&query) { return true; }
                                        r.fields.values().any(|v| format!("{:?}", v).to_lowercase().contains(&query))
                                    })
                                    .map(|(i, _)| i)
                                    .collect()
                            }
                            super::state::BulkActionScope::Selected => {
                                // Convert filtered indices to actual record indices
                                let multi_selected = state.list_state.all_selected();
                                let mut actual_indices = Vec::new();
                                let mut filtered_idx = 0;
                                for (actual_idx, record) in entity.records.iter().enumerate() {
                                    if !filter.matches(record.action) {
                                        continue;
                                    }
                                    let matches_search = if query.is_empty() {
                                        true
                                    } else if record.source_id.to_string().to_lowercase().contains(&query) {
                                        true
                                    } else {
                                        record.fields.values().any(|v| format!("{:?}", v).to_lowercase().contains(&query))
                                    };
                                    if !matches_search {
                                        continue;
                                    }
                                    if multi_selected.contains(&filtered_idx) {
                                        actual_indices.push(actual_idx);
                                    }
                                    filtered_idx += 1;
                                }
                                actual_indices
                            }
                        };

                        // Collect source IDs to mark dirty after mutations
                        let mut dirty_ids = Vec::new();

                        // Apply action to selected records
                        for idx in indices_to_apply {
                            if let Some(record) = entity.records.get_mut(idx) {
                                match state.bulk_action_selection {
                                    super::state::BulkAction::MarkSkip => {
                                        record.action = RecordAction::Skip;
                                    }
                                    super::state::BulkAction::UnmarkSkip => {
                                        if record.action == RecordAction::Skip {
                                            record.action = RecordAction::NoChange;
                                        }
                                    }
                                    super::state::BulkAction::ResetToOriginal => {
                                        // Reset to NoChange (would need original_action tracking for full reset)
                                        record.action = RecordAction::NoChange;
                                    }
                                }
                                dirty_ids.push(record.source_id);
                            }
                        }

                        // Mark all affected records as dirty
                        for source_id in dirty_ids {
                            entity.mark_dirty(source_id);
                        }
                    }
                }
                state.active_modal = None;
                state.list_state.clear_multi_selection();
                Command::None
            }

            // Excel export
            Msg::ExportExcel => {
                // Initialize export modal with current entity name as default filename
                if let Resource::Success(resolved) = &state.resolved {
                    if let Some(entity) = resolved.entities.get(state.current_entity_idx) {
                        // Set default filename based on entity name
                        let default_filename = format!("{}_resolved.xlsx", entity.entity_name);
                        state.export_filename.set_value(default_filename);

                        // Set filter to show directories only (for navigation)
                        // But also show .xlsx files so user can see existing exports
                        state.export_file_browser.set_filter(|entry| {
                            entry.is_dir || entry.name.to_lowercase().ends_with(".xlsx")
                        });

                        // Refresh to apply filter
                        let _ = state.export_file_browser.refresh();

                        state.active_modal = Some(super::state::PreviewModal::ExportExcel);
                        return Command::set_focus(crate::tui::FocusId::new("export-file-browser"));
                    }
                }
                Command::None
            }

            Msg::ExportFileNavigate(key) => {
                use crate::tui::widgets::{FileBrowserEvent, FileBrowserAction};

                match key {
                    crossterm::event::KeyCode::Up => {
                        state.export_file_browser.navigate_up();
                    }
                    crossterm::event::KeyCode::Down => {
                        state.export_file_browser.navigate_down();
                    }
                    crossterm::event::KeyCode::Enter => {
                        // Enter directory if selected, otherwise do nothing (we select directory, not file)
                        if let Some(action) = state.export_file_browser.handle_event(FileBrowserEvent::Activate) {
                            match action {
                                FileBrowserAction::DirectoryEntered(_) => {
                                    // Directory changed, stay in modal
                                }
                                FileBrowserAction::FileSelected(_) => {
                                    // User selected existing file - could overwrite or ignore
                                    // For now, just update filename field
                                    if let Some(entry) = state.export_file_browser.selected_entry() {
                                        if !entry.is_dir {
                                            state.export_filename.set_value(entry.name.clone());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    crossterm::event::KeyCode::Backspace => {
                        // Go up one directory
                        let _ = state.export_file_browser.handle_event(FileBrowserEvent::GoUp);
                    }
                    _ => {
                        state.export_file_browser.handle_navigation_key(key);
                    }
                }
                Command::None
            }

            Msg::ExportFilenameChanged(event) => {
                state.export_filename.handle_event(event, None);
                Command::None
            }

            Msg::ExportSetViewportHeight(height) => {
                let item_count = state.export_file_browser.entries().len();
                let list_state = state.export_file_browser.list_state_mut();
                list_state.set_viewport_height(height);
                list_state.update_scroll(height, item_count);
                Command::None
            }

            Msg::ConfirmExport => {
                // Get current entity and build export path
                if let Resource::Success(resolved) = &state.resolved {
                    if let Some(entity) = resolved.entities.get(state.current_entity_idx) {
                        let filename = state.export_filename.value().to_string();
                        if filename.is_empty() {
                            log::warn!("Export filename is empty");
                            return Command::None;
                        }

                        let dir = state.export_file_browser.current_path().to_path_buf();
                        let full_path = dir.join(&filename);

                        // Clone entity for async task
                        let entity_clone = entity.clone();
                        let path_str = full_path.to_string_lossy().to_string();

                        state.active_modal = None;

                        return Command::perform(
                            async move {
                                export_entity_to_excel(entity_clone, path_str).await
                            },
                            Msg::ExportCompleted,
                        );
                    }
                }
                Command::None
            }

            Msg::ImportExcel => {
                // Initialize import modal with file browser
                state.import_file_browser.set_filter(|entry| {
                    entry.is_dir || entry.name.to_lowercase().ends_with(".xlsx")
                });
                let _ = state.import_file_browser.refresh();
                state.active_modal = Some(super::state::PreviewModal::ImportExcel);
                Command::set_focus(crate::tui::FocusId::new("import-file-browser"))
            }

            Msg::ImportFileNavigate(key) => {
                use crate::tui::widgets::{FileBrowserEvent, FileBrowserAction};

                match key {
                    crossterm::event::KeyCode::Up => {
                        state.import_file_browser.navigate_up();
                    }
                    crossterm::event::KeyCode::Down => {
                        state.import_file_browser.navigate_down();
                    }
                    crossterm::event::KeyCode::Enter => {
                        if let Some(action) = state.import_file_browser.handle_event(FileBrowserEvent::Activate) {
                            match action {
                                FileBrowserAction::DirectoryEntered(_) => {
                                    // Directory changed, stay in modal
                                }
                                FileBrowserAction::FileSelected(path) => {
                                    // File selected - start import preview
                                    return Command::perform(
                                        async move { path },
                                        Msg::ImportFileSelected,
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                    crossterm::event::KeyCode::Backspace => {
                        let _ = state.import_file_browser.handle_event(FileBrowserEvent::GoUp);
                    }
                    _ => {
                        state.import_file_browser.handle_navigation_key(key);
                    }
                }
                Command::None
            }

            Msg::ImportSetViewportHeight(height) => {
                let item_count = state.import_file_browser.entries().len();
                let list_state = state.import_file_browser.list_state_mut();
                list_state.set_viewport_height(height);
                list_state.update_scroll(height, item_count);
                Command::None
            }

            Msg::ImportFileSelected(path) => {
                // Load file and check for conflicts
                if let Resource::Success(resolved) = &state.resolved {
                    let entity_idx = state.current_entity_idx;
                    if let Some(entity) = resolved.entities.get(entity_idx) {
                        let entity_clone = entity.clone();
                        let path_str = path.to_string_lossy().to_string();

                        return Command::perform(
                            async move {
                                preview_import(entity_clone, entity_idx, path_str).await
                            },
                            Msg::ImportPreviewLoaded,
                        );
                    }
                }
                Command::None
            }

            Msg::ImportPreviewLoaded(result) => {
                match result {
                    Ok(pending) => {
                        state.pending_import = Some(pending);
                        state.active_modal = Some(super::state::PreviewModal::ImportConfirm {
                            path: state.pending_import.as_ref().unwrap().path.clone(),
                            conflicts: state.pending_import.as_ref().unwrap().conflicts.iter()
                                .map(|id| id.to_string())
                                .collect(),
                        });
                    }
                    Err(e) => {
                        log::error!("❌ Import preview failed: {}", e);
                        state.active_modal = None;
                    }
                }
                Command::None
            }

            Msg::ConfirmImport => {
                if let (Some(pending), Resource::Success(resolved)) = (&state.pending_import, &mut state.resolved) {
                    if let Some(entity) = resolved.entities.get_mut(pending.entity_idx) {
                        let entity_clone = entity.clone();
                        let path = pending.path.clone();

                        state.active_modal = None;
                        state.pending_import = None;

                        return Command::perform(
                            async move {
                                apply_import(entity_clone, path).await
                            },
                            |result| match result {
                                Ok(updated_entity) => Msg::ImportCompleted(Ok(updated_entity)),
                                Err(e) => Msg::ImportCompleted(Err(e)),
                            },
                        );
                    }
                }
                Command::None
            }

            Msg::CancelImport => {
                state.pending_import = None;
                state.active_modal = None;
                Command::None
            }

            Msg::ExportCompleted(result) => {
                match result {
                    Ok(path) => {
                        log::info!("✅ Exported to {}", path);
                        // TODO: Could show a success notification here
                    }
                    Err(e) => {
                        log::error!("❌ Export failed: {}", e);
                        // TODO: Could show an error modal here
                    }
                }
                Command::None
            }

            Msg::ImportCompleted(result) => {
                match result {
                    Ok(updated_entity) => {
                        // Replace the entity in the resolved transfer
                        if let Resource::Success(resolved) = &mut state.resolved {
                            if let Some(entity) = resolved.entities.get_mut(state.current_entity_idx) {
                                *entity = updated_entity;
                            }
                        }
                        log::info!("✅ Import completed successfully");
                    }
                    Err(e) => log::error!("❌ Import failed: {}", e),
                }
                state.active_modal = None;
                Command::None
            }

            // Refresh - re-fetch data and re-run transforms
            Msg::Refresh => {
                // Only refresh if we have a config and are not already loading
                let config = match &state.config {
                    Some(c) => c.clone(),
                    None => return Command::None,
                };

                if matches!(state.resolved, Resource::Loading) {
                    log::warn!("Already loading, ignoring refresh");
                    return Command::None;
                }

                log::info!("Starting refresh...");
                state.is_refreshing = true;

                // Clear accumulated data for new fetch
                state.source_data.clear();
                state.target_data.clear();

                // Build parallel fetch tasks (same as ConfigLoaded but uses existing config)
                let mut builder = Command::perform_parallel()
                    .with_title("Refreshing Records");

                let num_entities = config.entity_mappings.len();

                // Add source fetch tasks
                for mapping in &config.entity_mappings {
                    let entity = mapping.source_entity.clone();
                    let env = config.source_env.clone();

                    // Collect source fields from transforms + primary key
                    let mut source_fields: Vec<String> = mapping
                        .field_mappings
                        .iter()
                        .flat_map(|fm| fm.transform.source_fields())
                        .map(|s| s.to_string())
                        .collect();
                    source_fields.push(format!("{}id", entity));

                    // Add _fieldname_value for lookup fields (from source metadata)
                    if let Some(fields) = state.source_metadata.get(&mapping.source_entity) {
                        let lookup_fields: std::collections::HashSet<&str> = fields
                            .iter()
                            .filter(|f| f.related_entity.is_some())
                            .map(|f| f.logical_name.as_str())
                            .collect();

                        let lookup_value_fields: Vec<String> = source_fields
                            .iter()
                            .filter(|f| lookup_fields.contains(f.as_str()))
                            .map(|f| format!("_{}_value", f))
                            .collect();
                        source_fields.extend(lookup_value_fields);
                    }

                    source_fields.sort();
                    source_fields.dedup();

                    // Build expand tree for nested lookup traversals
                    let mut expand_tree = ExpandTree::new();
                    for fm in &mapping.field_mappings {
                        expand_tree.add_transform(&fm.transform);
                    }
                    let expands = expand_tree.build_expand_clauses();

                    builder = builder.add_task_with_progress(
                        format!("Source: {}", entity),
                        move |progress| fetch_entity_records(env, entity, true, source_fields, expands, Some(progress), true), // force refresh
                    );
                }

                // Add target fetch tasks
                for mapping in &config.entity_mappings {
                    let entity = mapping.target_entity.clone();
                    let env = config.target_env.clone();

                    let mut target_fields: Vec<String> = mapping
                        .field_mappings
                        .iter()
                        .map(|fm| fm.target_field.clone())
                        .collect();
                    target_fields.push(format!("{}id", entity));

                    // Replace lookup fields with _fieldname_value (from target metadata)
                    if let Some(fields) = state.target_metadata.get(&mapping.target_entity) {
                        let lookup_fields: std::collections::HashSet<&str> = fields
                            .iter()
                            .filter(|f| f.related_entity.is_some())
                            .map(|f| f.logical_name.as_str())
                            .collect();

                        // Replace lookup field names with _value versions
                        target_fields = target_fields
                            .into_iter()
                            .map(|f| {
                                if lookup_fields.contains(f.as_str()) {
                                    format!("_{}_value", f)
                                } else {
                                    f
                                }
                            })
                            .collect();
                    }

                    target_fields.sort();
                    target_fields.dedup();

                    let no_expands: Vec<String> = vec![];

                    builder = builder.add_task_with_progress(
                        format!("Target: {}", entity),
                        move |progress| fetch_entity_records(env, entity, false, target_fields, no_expands, Some(progress), true), // force refresh
                    );
                }

                // Add resolver source entity fetches (from target environment)
                let resolver_entities: Vec<String> = config
                    .resolvers
                    .iter()
                    .map(|r| r.source_entity.clone())
                    .filter(|e| !config.entity_mappings.iter().any(|m| &m.target_entity == e))
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();

                for resolver in &config.resolvers {
                    // Skip if already fetched as part of entity mappings
                    if config.entity_mappings.iter().any(|m| m.target_entity == resolver.source_entity) {
                        continue;
                    }

                    let entity = resolver.source_entity.clone();
                    let env = config.target_env.clone();
                    let match_field = resolver.match_field.clone();

                    let mut resolver_fields = vec![
                        format!("{}id", entity),
                        match_field,
                    ];
                    resolver_fields.sort();
                    resolver_fields.dedup();

                    let no_expands: Vec<String> = vec![];

                    builder = builder.add_task_with_progress(
                        format!("Resolver: {}", entity),
                        move |progress| fetch_entity_records(env, entity, false, resolver_fields, no_expands, Some(progress), true), // force refresh
                    );
                }

                state.pending_fetches = num_entities * 2 + resolver_entities.len();

                builder
                    .on_complete(AppId::TransferPreview)
                    .build(|_task_idx, result| {
                        let data = result
                            .downcast::<Result<(String, bool, Vec<serde_json::Value>), String>>()
                            .unwrap();
                        Msg::FetchResult(*data)
                    })
            }

            // Modal
            Msg::CloseModal => {
                state.active_modal = None;
                state.record_detail_state = None;
                Command::None
            }

            // Navigation
            Msg::Back => {
                Command::navigate_to(AppId::TransferMappingEditor)
            }

            // Send to Queue
            Msg::OpenSendToQueue => {
                // Check if there are errors - block if any exist
                if let Resource::Success(ref resolved) = state.resolved {
                    if resolved.error_count() > 0 {
                        log::warn!(
                            "Cannot send to queue: {} records have errors",
                            resolved.error_count()
                        );
                        // Could show an error modal here, but for now just log
                        return Command::None;
                    }

                    // Check if there's anything to send
                    let actionable_count = resolved.create_count() + resolved.update_count();
                    if actionable_count == 0 {
                        log::info!("Nothing to send to queue (no creates or updates)");
                        return Command::None;
                    }

                    state.active_modal = Some(super::state::PreviewModal::SendToQueue);
                }
                Command::None
            }

            Msg::ConfirmSendToQueue => {
                if let Resource::Success(ref resolved) = state.resolved {
                    state.active_modal = None;

                    // Build queue items synchronously
                    let queue_items = build_queue_items_from_resolved(resolved);

                    if queue_items.is_empty() {
                        log::info!("No operations to queue");
                        return Command::None;
                    }

                    let total_ops: usize = queue_items.iter().map(|item| item.operations.len()).sum();
                    log::info!("✅ Sending {} operations to queue", total_ops);

                    // Serialize and publish via Command
                    match serde_json::to_value(&queue_items) {
                        Ok(queue_items_json) => {
                            return Command::Batch(vec![
                                Command::Publish {
                                    topic: "queue:add_items".to_string(),
                                    data: queue_items_json,
                                },
                                Command::navigate_to(AppId::OperationQueue),
                            ]);
                        }
                        Err(e) => {
                            log::error!("Failed to serialize queue items: {}", e);
                        }
                    }
                }
                Command::None
            }
        }
    }

    fn view(state: &mut State) -> LayeredView<Msg> {
        let theme = &crate::global_runtime_config().theme;
        view::render(state, theme)
    }

    fn subscriptions(state: &State) -> Vec<Subscription<Msg>> {
        view::subscriptions(state)
    }

    fn title() -> &'static str {
        "Transfer Preview"
    }

    fn status(state: &State) -> Option<Line<'static>> {
        let theme = &crate::global_runtime_config().theme;

        match &state.resolved {
            Resource::NotAsked => None,
            Resource::Loading => Some(Line::from(vec![
                Span::styled("Loading...", Style::default().fg(theme.text_secondary)),
            ])),
            Resource::Failure(err) => Some(Line::from(vec![
                Span::styled("Error: ", Style::default().fg(theme.accent_error)),
                Span::styled(err.clone(), Style::default().fg(theme.text_primary)),
            ])),
            Resource::Success(resolved) => {
                if resolved.entities.is_empty() {
                    return Some(Line::from("No entities"));
                }

                let entity = &resolved.entities[state.current_entity_idx];
                let filtered_count = entity.records.iter()
                    .filter(|r| state.filter.matches(r.action))
                    .filter(|r| {
                        let query = state.search_field.value().to_lowercase();
                        if query.is_empty() {
                            return true;
                        }
                        // Search in source_id and field values
                        if r.source_id.to_string().to_lowercase().contains(&query) {
                            return true;
                        }
                        r.fields.values().any(|v| {
                            format!("{:?}", v).to_lowercase().contains(&query)
                        })
                    })
                    .count();

                Some(Line::from(vec![
                    Span::styled(entity.entity_name.clone(), Style::default().fg(theme.accent_primary)),
                    Span::styled(
                        format!(" ({}/{})", state.current_entity_idx + 1, resolved.entities.len()),
                        Style::default().fg(theme.text_secondary),
                    ),
                    Span::raw(" | "),
                    Span::styled(format!("{}", entity.create_count()), Style::default().fg(theme.accent_success)),
                    Span::styled(" create".to_string(), Style::default().fg(theme.text_secondary)),
                    Span::raw(" "),
                    Span::styled(format!("{}", entity.update_count()), Style::default().fg(theme.accent_secondary)),
                    Span::styled(" update".to_string(), Style::default().fg(theme.text_secondary)),
                    Span::raw(" "),
                    Span::styled(format!("{}", entity.nochange_count()), Style::default().fg(theme.text_tertiary)),
                    Span::styled(" unchanged".to_string(), Style::default().fg(theme.text_secondary)),
                    Span::raw(" "),
                    Span::styled(format!("{}", entity.target_only_count()), Style::default().fg(theme.accent_primary)),
                    Span::styled(" target-only".to_string(), Style::default().fg(theme.text_secondary)),
                    Span::raw(" "),
                    Span::styled(format!("{}", entity.skip_count()), Style::default().fg(theme.accent_warning)),
                    Span::styled(" skip".to_string(), Style::default().fg(theme.text_secondary)),
                    Span::raw(" "),
                    Span::styled(format!("{}", entity.error_count()), Style::default().fg(theme.accent_error)),
                    Span::styled(" error".to_string(), Style::default().fg(theme.text_secondary)),
                    Span::raw(" | "),
                    Span::styled(state.filter.display_name().to_string(), Style::default().fg(theme.accent_primary)),
                    Span::styled(format!(" ({})", filtered_count), Style::default().fg(theme.text_secondary)),
                    // Show selection count if multi-selection is active
                    if state.list_state.has_multi_selection() {
                        Span::styled(
                            format!(" | {} selected", state.list_state.multi_select_count()),
                            Style::default().fg(theme.accent_secondary),
                        )
                    } else {
                        Span::raw("")
                    },
                ]))
            }
        }
    }

    fn on_resume(state: &mut State) -> Command<Msg> {
        // If we have data but haven't transformed yet, run the transform now
        if matches!(state.resolved, Resource::Loading)
            && state.config.is_some()
            && !state.source_data.is_empty()
        {
            log::info!("TransferPreviewApp resuming with data ready - triggering transform");
            // Use a message to run transform so UI can update first
            Command::perform(async { () }, |_| Msg::RunTransform)
        } else {
            Command::None
        }
    }
}

// =============================================================================
// Async helper functions
// =============================================================================

/// Load transfer config from database
async fn load_config(config_name: String) -> Result<TransferConfig, String> {
    let pool = &crate::global_config().pool;
    get_transfer_config(pool, &config_name)
        .await
        .map_err(|e| format!("Failed to load config: {}", e))?
        .ok_or_else(|| format!("Config '{}' not found", config_name))
}

/// Fetch all records for an entity from an environment
/// Returns (entity_name, is_source, records)
///
/// If `force_refresh` is false, checks SQLite cache first (1 hour TTL).
/// Always saves fetched data to cache after API call.
async fn fetch_entity_records(
    env_name: String,
    entity_name: String,
    is_source: bool,
    fields: Vec<String>,  // Fields to select (for performance)
    expands: Vec<String>, // Expand clauses for lookup traversals
    progress: Option<crate::tui::command::ProgressSender>,
    force_refresh: bool,  // If true, bypass cache and fetch fresh
) -> Result<(String, bool, Vec<serde_json::Value>), String> {
    use crate::api::pluralization::pluralize_entity_name;
    use crate::api::query::QueryBuilder;

    let config = crate::global_config();

    // Check cache first (1 hour TTL) unless force_refresh
    if !force_refresh {
        if let Some(ref tx) = progress {
            let _ = tx.send("Checking cache...".to_string());
        }

        match config.get_entity_data_cache(&env_name, &entity_name, 1).await {
            Ok(Some(cached_data)) => {
                log::info!(
                    "✅ Using cached data for {} from {} ({} records)",
                    entity_name,
                    env_name,
                    cached_data.len()
                );
                if let Some(ref tx) = progress {
                    let _ = tx.send(format!("{} (cached)", cached_data.len()));
                }
                return Ok((entity_name, is_source, cached_data));
            }
            Ok(None) => {
                log::info!("[{}] No valid cache, fetching from API", entity_name);
            }
            Err(e) => {
                log::warn!("[{}] Cache check failed, fetching from API: {}", entity_name, e);
            }
        }
    } else {
        log::info!("[{}] Force refresh - bypassing cache", entity_name);
    }

    let manager = crate::client_manager();
    let client = manager
        .get_client(&env_name)
        .await
        .map_err(|e| format!("Failed to get client for {}: {}", env_name, e))?;

    let entity_set = pluralize_entity_name(&entity_name);

    // First: get real count via FetchXML aggregate (OData $count caps at 5000)
    let count_fetchxml = format!(
        r#"<fetch aggregate="true"><entity name="{}"><attribute name="{}id" aggregate="count" alias="total"/></entity></fetch>"#,
        entity_name, entity_name
    );

    let total_count: Option<u64> = match client.execute_fetchxml(&entity_name, &count_fetchxml).await {
        Ok(result) => {
            result.get("value")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|obj| obj.get("total"))
                .and_then(|t| t.as_u64())
        }
        Err(e) => {
            log::warn!("[{}] Count query failed, progress will show records only: {}", entity_name, e);
            None
        }
    };
    log::info!("[{}] Total count: {:?}", entity_name, total_count);

    // Report initial progress
    if let Some(ref tx) = progress {
        let msg = match total_count {
            Some(total) => format!("0/{}", total),
            None => "Starting...".to_string(),
        };
        let _ = tx.send(msg);
    }

    // Fetch data using @odata.nextLink pagination (Dynamics doesn't support $skip)
    let mut all_records = Vec::new();
    let mut page = 0;

    log::info!("[{}] 🚀 Starting data fetch...", entity_name);
    let fetch_start = std::time::Instant::now();

    // Use smaller page size for more responsive progress updates
    const PAGE_SIZE: u32 = 500;

    // Build initial query with $top for smaller chunks
    // Fetch all records (active + inactive) for complete transfer coverage
    let mut builder = QueryBuilder::new(&entity_set);
    if !fields.is_empty() {
        let field_refs: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();
        builder = builder.select(&field_refs);
    }
    // Add $expand for lookup traversals
    if !expands.is_empty() {
        let expand_refs: Vec<&str> = expands.iter().map(|s| s.as_str()).collect();
        builder = builder.expand(&expand_refs);
        log::info!("[{}] Query will expand: {:?}", entity_name, expands);
    }
    // Use smaller page size for more responsive progress
    builder = builder.top(PAGE_SIZE);
    let query = builder.build();
    log::info!("[{}] Executing query: {:?}", entity_name, query);

    let mut result = client
        .execute_query(&query)
        .await
        .map_err(|e| format!("Query failed for {}: {}", entity_name, e))?;

    log::info!("[{}] Initial response: has_data={}, record_count={}, has_more={}",
        entity_name,
        result.data.is_some(),
        result.data.as_ref().map(|d| d.value.len()).unwrap_or(0),
        result.has_more()
    );

    loop {
        page += 1;
        let page_start = std::time::Instant::now();

        let page_records = result.data
            .as_ref()
            .map(|d| d.value.len())
            .unwrap_or(0);

        log::info!("[{}] ✅ Page {} fetched in {}ms ({} records)",
            entity_name, page, page_start.elapsed().as_millis(), page_records);

        if let Some(ref data) = result.data {
            all_records.extend(data.value.clone());
        }

        // Report progress with ETA
        let progress_msg = match total_count {
            Some(total) => {
                let fetched = all_records.len() as u64;
                let elapsed = fetch_start.elapsed();

                // Calculate ETA based on records per second
                let eta_str = if fetched > 0 && fetched < total {
                    let records_per_sec = fetched as f64 / elapsed.as_secs_f64();
                    if records_per_sec > 0.0 {
                        let remaining = total - fetched;
                        let eta_secs = (remaining as f64 / records_per_sec) as u64;
                        if eta_secs >= 60 {
                            format!(" (~{}m {}s left)", eta_secs / 60, eta_secs % 60)
                        } else {
                            format!(" (~{}s left)", eta_secs)
                        }
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                format!("{}/{}{}", fetched, total, eta_str)
            }
            None => format!("{} records", all_records.len()),
        };
        if let Some(ref tx) = progress {
            let _ = tx.send(progress_msg);
        }

        // Check if there's more data via nextLink
        if !result.has_more() {
            break;
        }

        // Fetch next page
        let next_start = std::time::Instant::now();
        result = result
            .next_page(&client, Some(PAGE_SIZE))
            .await
            .map_err(|e| format!("Pagination failed: {}", e))?
            .ok_or_else(|| "nextLink returned no data".to_string())?;

        log::debug!("[{}] Next page request took {}ms", entity_name, next_start.elapsed().as_millis());
    }

    let total_time = fetch_start.elapsed();
    log::info!(
        "✅ Fetched {} records for {} from {} in {}ms",
        all_records.len(),
        entity_name,
        env_name,
        total_time.as_millis()
    );

    // Save to cache for future use
    if let Err(e) = config.set_entity_data_cache(&env_name, &entity_name, &all_records).await {
        log::warn!("[{}] Failed to cache data: {}", entity_name, e);
    } else {
        log::info!("[{}] Cached {} records", entity_name, all_records.len());
    }

    Ok((entity_name, is_source, all_records))
}

/// Export a ResolvedEntity to an Excel file
async fn export_entity_to_excel(
    entity: crate::transfer::ResolvedEntity,
    path: String,
) -> Result<String, String> {
    use crate::transfer::excel::resolved::write_resolved_excel;

    // The write function is synchronous, so we wrap it in spawn_blocking
    let result = tokio::task::spawn_blocking(move || {
        write_resolved_excel(&entity, &path)
            .map(|_| path)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?;

    result.map_err(|e| format!("Export failed: {}", e))
}

/// Preview an import by reading the Excel file and detecting conflicts
async fn preview_import(
    entity: crate::transfer::ResolvedEntity,
    entity_idx: usize,
    path: String,
) -> Result<super::state::PendingImport, String> {
    use crate::transfer::excel::resolved::read_resolved_excel;

    let path_clone = path.clone();

    // Read the file and detect edits (synchronous, use spawn_blocking)
    let result = tokio::task::spawn_blocking(move || {
        // Clone entity to read edits without modifying
        let mut temp_entity = entity.clone();
        let edits = read_resolved_excel(&path_clone, &mut temp_entity)
            .map_err(|e| format!("Failed to read Excel: {}", e))?;

        // Detect conflicts: records that are dirty locally AND changed in Excel
        let conflicts: Vec<uuid::Uuid> = edits.changed_records.keys()
            .filter(|id| entity.is_dirty(**id))
            .copied()
            .collect();

        Ok(super::state::PendingImport {
            path: path_clone,
            entity_idx,
            edit_count: edits.changed_records.len(),
            conflicts,
        })
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?;

    result
}

/// Apply an import by reading the Excel file and updating the entity
async fn apply_import(
    mut entity: crate::transfer::ResolvedEntity,
    path: String,
) -> Result<crate::transfer::ResolvedEntity, String> {
    use crate::transfer::excel::resolved::read_resolved_excel;

    // Apply edits (synchronous, use spawn_blocking)
    let result = tokio::task::spawn_blocking(move || {
        read_resolved_excel(&path, &mut entity)
            .map_err(|e| format!("Failed to apply import: {}", e))?;
        Ok(entity)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?;

    result
}

/// Build queue items from resolved transfer
fn build_queue_items_from_resolved(resolved: &ResolvedTransfer) -> Vec<crate::tui::apps::queue::models::QueueItem> {
    use crate::transfer::queue::{build_queue_items, QueueBuildOptions};

    let options = QueueBuildOptions::default();
    build_queue_items(resolved, &options)
}

/// Fetch source entity field metadata for lookup detection
/// Returns (entity_name, field_metadata)
/// Uses SQLite cache with 1 hour TTL
async fn fetch_source_metadata(
    env_name: String,
    entity_name: String,
) -> Result<(String, Vec<crate::api::metadata::FieldMetadata>), String> {
    log::debug!("[{}] fetch_source_metadata START for env={}", entity_name, env_name);

    let config = crate::global_config();

    // Check cache first (1 hour TTL)
    match config.get_entity_metadata_cache(&env_name, &entity_name, 1).await {
        Ok(Some(cached)) => {
            log::info!(
                "[{}] Using cached source metadata: {} fields",
                entity_name,
                cached.fields.len()
            );
            return Ok((entity_name, cached.fields));
        }
        Ok(None) => {
            log::info!("[{}] No valid source cache, fetching from API", entity_name);
        }
        Err(e) => {
            log::warn!("[{}] Cache check failed, fetching from API: {}", entity_name, e);
        }
    }

    // Fetch from API
    let manager = crate::client_manager();
    let client = manager
        .get_client(&env_name)
        .await
        .map_err(|e| format!("Failed to get client for {}: {}", env_name, e))?;

    // Fetch field metadata
    let fields = client
        .fetch_entity_fields_alt(&entity_name)
        .await
        .map_err(|e| format!("Failed to fetch field metadata for {}: {}", entity_name, e))?;

    log::info!(
        "[{}] Fetched {} source fields",
        entity_name,
        fields.len()
    );

    // Save to cache
    let metadata = crate::api::metadata::EntityMetadata {
        fields: fields.clone(),
        ..Default::default()
    };
    if let Err(e) = config.set_entity_metadata_cache(&env_name, &entity_name, &metadata).await {
        log::warn!("[{}] Failed to cache source metadata: {}", entity_name, e);
    }

    Ok((entity_name, fields))
}

/// Fetch target entity field metadata for lookup field detection
/// Returns (entity_name, field_metadata, entity_set_name)
/// Uses SQLite cache with 1 hour TTL
async fn fetch_target_metadata(
    env_name: String,
    entity_name: String,
) -> Result<(String, Vec<crate::api::metadata::FieldMetadata>, String), String> {
    log::debug!("[{}] fetch_target_metadata START for env={}", entity_name, env_name);

    let config = crate::global_config();

    // Check cache first (1 hour TTL) - only use if entity_set_name is present
    match config.get_entity_metadata_cache(&env_name, &entity_name, 1).await {
        Ok(Some(cached)) if cached.entity_set_name.is_some() => {
            let entity_set = cached.entity_set_name.unwrap();
            log::info!(
                "[{}] Using cached target metadata: {} fields, entity_set={}",
                entity_name,
                cached.fields.len(),
                entity_set
            );
            return Ok((entity_name, cached.fields, entity_set));
        }
        Ok(_) => {
            log::info!("[{}] No valid target cache (or missing entity_set_name), fetching from API", entity_name);
        }
        Err(e) => {
            log::warn!("[{}] Target cache check failed, fetching from API: {}", entity_name, e);
        }
    }

    // Fetch from API
    let manager = crate::client_manager();
    let client = manager
        .get_client(&env_name)
        .await
        .map_err(|e| format!("Failed to get client for {}: {}", env_name, e))?;

    // Fetch field metadata
    let fields = client
        .fetch_entity_fields_alt(&entity_name)
        .await
        .map_err(|e| format!("Failed to fetch target field metadata for {}: {}", entity_name, e))?;

    // Fetch entity metadata info (includes entity_set_name)
    let entity_info = client
        .fetch_entity_metadata_info(&entity_name)
        .await
        .map_err(|e| format!("Failed to fetch entity info for {}: {}", entity_name, e))?;

    log::info!(
        "[{}] Fetched {} target fields, entity_set={}",
        entity_name,
        fields.len(),
        entity_info.entity_set_name
    );

    // Save to cache with entity_set_name
    let metadata = crate::api::metadata::EntityMetadata {
        fields: fields.clone(),
        entity_set_name: Some(entity_info.entity_set_name.clone()),
        ..Default::default()
    };
    if let Err(e) = config.set_entity_metadata_cache(&env_name, &entity_name, &metadata).await {
        log::warn!("[{}] Failed to cache target metadata: {}", entity_name, e);
    }

    Ok((entity_name, fields, entity_info.entity_set_name))
}

/// Fetch entity metadata (fields + entity set name) for lookup binding
/// Returns (entity_name, field_metadata, entity_set_name)
/// Uses SQLite cache with 1 hour TTL
async fn fetch_entity_metadata(
    env_name: String,
    entity_name: String,
) -> Result<(String, Vec<crate::api::metadata::FieldMetadata>, String), String> {
    log::debug!("[{}] fetch_entity_metadata START for env={}", entity_name, env_name);

    let config = crate::global_config();
    log::debug!("[{}] Got global_config", entity_name);

    // Check cache first (1 hour TTL)
    log::debug!("[{}] Checking cache...", entity_name);
    match config.get_entity_metadata_cache(&env_name, &entity_name, 1).await {
        Ok(Some(cached)) if cached.entity_set_name.is_some() => {
            let entity_set = cached.entity_set_name.unwrap();
            log::info!(
                "[{}] Using cached metadata: {} fields, entity_set={} (from cache)",
                entity_name,
                cached.fields.len(),
                entity_set
            );
            return Ok((entity_name, cached.fields, entity_set));
        }
        Ok(_) => {
            log::info!("[{}] No valid cache, fetching from API", entity_name);
        }
        Err(e) => {
            log::warn!("[{}] Cache check failed, fetching from API: {}", entity_name, e);
        }
    }

    // Fetch from API
    let manager = crate::client_manager();
    let client = manager
        .get_client(&env_name)
        .await
        .map_err(|e| format!("Failed to get client for {}: {}", env_name, e))?;

    // Fetch field metadata (includes schema_name for @odata.bind)
    let fields = client
        .fetch_entity_fields_alt(&entity_name)
        .await
        .map_err(|e| format!("Failed to fetch field metadata for {}: {}", entity_name, e))?;

    // Fetch entity metadata info (includes entity_set_name)
    let entity_info = client
        .fetch_entity_metadata_info(&entity_name)
        .await
        .map_err(|e| format!("Failed to fetch entity info for {}: {}", entity_name, e))?;

    log::info!(
        "[{}] Fetched {} fields, entity_set={}",
        entity_name,
        fields.len(),
        entity_info.entity_set_name
    );

    // Save to cache
    let metadata = crate::api::metadata::EntityMetadata {
        fields: fields.clone(),
        entity_set_name: Some(entity_info.entity_set_name.clone()),
        ..Default::default()
    };
    if let Err(e) = config.set_entity_metadata_cache(&env_name, &entity_name, &metadata).await {
        log::warn!("[{}] Failed to cache metadata: {}", entity_name, e);
    }

    Ok((entity_name, fields, entity_info.entity_set_name))
}

/// Merge dirty records from old resolved state into new resolved state.
/// Dirty records preserve their user-edited action and field values.
fn merge_dirty_records(new_resolved: &mut ResolvedTransfer, old_resolved: &ResolvedTransfer) {
    for (new_entity, old_entity) in new_resolved.entities.iter_mut().zip(old_resolved.entities.iter()) {
        // Skip if entity names don't match (shouldn't happen, but be safe)
        if new_entity.entity_name != old_entity.entity_name {
            log::warn!(
                "Entity mismatch during merge: {} vs {}",
                new_entity.entity_name,
                old_entity.entity_name
            );
            continue;
        }

        let mut dirty_count = 0;

        // For each dirty record in the old entity, find and update in new entity
        for old_record in &old_entity.records {
            if !old_entity.is_dirty(old_record.source_id) {
                continue;
            }

            // Find matching record in new entity by source_id
            if let Some(new_record) = new_entity.records.iter_mut().find(|r| r.source_id == old_record.source_id) {
                // Preserve user edits: copy action and field values from old record
                new_record.action = old_record.action;
                new_record.error = old_record.error.clone();

                // Copy all field values from old record
                for (field_name, field_value) in &old_record.fields {
                    new_record.fields.insert(field_name.clone(), field_value.clone());
                }

                // Mark as dirty in new entity
                new_entity.mark_dirty(old_record.source_id);
                dirty_count += 1;
            } else {
                // Record was in old but not in new - it might have been deleted from source
                log::info!(
                    "[{}] Dirty record {} not found in refreshed data (may have been deleted)",
                    new_entity.entity_name,
                    old_record.source_id
                );
            }
        }

        if dirty_count > 0 {
            log::info!(
                "[{}] Preserved {} dirty records during refresh",
                new_entity.entity_name,
                dirty_count
            );
        }
    }
}
