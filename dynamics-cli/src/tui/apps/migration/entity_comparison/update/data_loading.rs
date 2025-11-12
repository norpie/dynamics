use crate::tui::command::{AppId, Command};
use crate::tui::Resource;
use std::collections::HashMap;
use super::super::{Msg, FetchType, FetchedData, ExamplePair, fetch_with_cache, extract_relationships};
use super::super::app::State;
use super::super::matching_adapter::recompute_all_matches;

/// Collect unique field types from a list of fields, sorted for consistent cycling
fn collect_field_types(fields: &[crate::api::metadata::FieldMetadata]) -> Vec<crate::api::metadata::models::FieldType> {
    let mut types: Vec<_> = fields.iter()
        .map(|f| f.field_type.clone())
        .collect();

    // Deduplicate while preserving order
    types.sort_by(|a, b| format!("{:?}", a).cmp(&format!("{:?}", b)));
    types.dedup();

    types
}

pub fn handle_parallel_data_loaded(
    state: &mut State,
    _task_idx: usize,
    result: Result<FetchedData, String>
) -> Command<Msg> {
    match result {
        Ok(data) => {
            // Update the appropriate metadata based on the data variant
            match data {
                FetchedData::SourceFields(mut fields) => {
                    // Extract relationships if we have Lookup fields (fresh from API)
                    // If no Lookup fields, they came from cache and relationships need to be loaded separately
                    let has_lookup_fields = fields.iter().any(|f| matches!(&f.field_type, crate::api::metadata::FieldType::Lookup) || matches!(&f.field_type, crate::api::metadata::FieldType::Other(t) if t.starts_with("Relationship:")));

                    let relationships = if has_lookup_fields {
                        let rels = extract_relationships(&fields);
                        fields.retain(|f| {
                            !matches!(&f.field_type, crate::api::metadata::FieldType::Lookup)
                                && !matches!(&f.field_type, crate::api::metadata::FieldType::Other(t) if t.starts_with("Relationship:"))
                        });
                        rels
                    } else {
                        // From cache - load relationships from cache
                        // TODO: Support multi-entity mode - for now use first entity
                        let config = crate::global_config();
                        let source_env = state.source_env.clone();
                        let source_entity = state.source_entities.first().cloned().unwrap_or_default();
                        tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                config.get_entity_metadata_cache(&source_env, &source_entity, 12)
                                    .await
                                    .ok()
                                    .flatten()
                                    .map(|cached| cached.relationships)
                                    .unwrap_or_default()
                            })
                        })
                    };

                    // TODO: Support multi-entity mode - for now use first entity
                    let first_entity = state.source_entities.first().cloned().unwrap_or_default();
                    if let Some(Resource::Success(meta)) = state.source_metadata.get_mut(&first_entity) {
                        meta.fields = fields;
                        meta.relationships = relationships;
                    } else {
                        state.source_metadata.insert(first_entity, Resource::Success(crate::api::EntityMetadata {
                            fields,
                            relationships,
                            ..Default::default()
                        }));
                    }
                }
                FetchedData::SourceForms(forms) => {
                    // TODO: Support multi-entity mode - for now use first entity
                    let first_entity = state.source_entities.first().cloned().unwrap_or_default();
                    if let Some(Resource::Success(meta)) = state.source_metadata.get_mut(&first_entity) {
                        meta.forms = forms;
                    } else {
                        state.source_metadata.insert(first_entity, Resource::Success(crate::api::EntityMetadata {
                            forms,
                            ..Default::default()
                        }));
                    }
                }
                FetchedData::SourceViews(views) => {
                    // TODO: Support multi-entity mode - for now use first entity
                    let first_entity = state.source_entities.first().cloned().unwrap_or_default();
                    if let Some(Resource::Success(meta)) = state.source_metadata.get_mut(&first_entity) {
                        meta.views = views;
                    } else {
                        state.source_metadata.insert(first_entity, Resource::Success(crate::api::EntityMetadata {
                            views,
                            ..Default::default()
                        }));
                    }
                }
                FetchedData::TargetFields(mut fields) => {
                    // Extract relationships if we have Lookup fields (fresh from API)
                    // If no Lookup fields, they came from cache and relationships need to be loaded separately
                    let has_lookup_fields = fields.iter().any(|f| matches!(&f.field_type, crate::api::metadata::FieldType::Lookup) || matches!(&f.field_type, crate::api::metadata::FieldType::Other(t) if t.starts_with("Relationship:")));

                    let relationships = if has_lookup_fields {
                        let rels = extract_relationships(&fields);
                        fields.retain(|f| {
                            !matches!(&f.field_type, crate::api::metadata::FieldType::Lookup)
                                && !matches!(&f.field_type, crate::api::metadata::FieldType::Other(t) if t.starts_with("Relationship:"))
                        });
                        rels
                    } else {
                        // From cache - load relationships from cache
                        // TODO: Support multi-entity mode - for now use first entity
                        let config = crate::global_config();
                        let target_env = state.target_env.clone();
                        let target_entity = state.target_entities.first().cloned().unwrap_or_default();
                        tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                config.get_entity_metadata_cache(&target_env, &target_entity, 12)
                                    .await
                                    .ok()
                                    .flatten()
                                    .map(|cached| cached.relationships)
                                    .unwrap_or_default()
                            })
                        })
                    };

                    // TODO: Support multi-entity mode - for now use first entity
                    let first_entity = state.target_entities.first().cloned().unwrap_or_default();
                    if let Some(Resource::Success(meta)) = state.target_metadata.get_mut(&first_entity) {
                        meta.fields = fields;
                        meta.relationships = relationships;
                    } else {
                        state.target_metadata.insert(first_entity, Resource::Success(crate::api::EntityMetadata {
                            fields,
                            relationships,
                            ..Default::default()
                        }));
                    }
                }
                FetchedData::TargetForms(forms) => {
                    // TODO: Support multi-entity mode - for now use first entity
                    let first_entity = state.target_entities.first().cloned().unwrap_or_default();
                    if let Some(Resource::Success(meta)) = state.target_metadata.get_mut(&first_entity) {
                        meta.forms = forms;
                    } else {
                        state.target_metadata.insert(first_entity, Resource::Success(crate::api::EntityMetadata {
                            forms,
                            ..Default::default()
                        }));
                    }
                }
                FetchedData::TargetViews(views) => {
                    // TODO: Support multi-entity mode - for now use first entity
                    let first_entity = state.target_entities.first().cloned().unwrap_or_default();
                    if let Some(Resource::Success(meta)) = state.target_metadata.get_mut(&first_entity) {
                        meta.views = views;
                    } else {
                        state.target_metadata.insert(first_entity, Resource::Success(crate::api::EntityMetadata {
                            views,
                            ..Default::default()
                        }));
                    }
                }
                FetchedData::ExampleData(pair_id, source_data, target_data) => {
                    // Store example data in cache with composite keys (entity:record_id)
                    if let Some(pair) = state.examples.pairs.iter().find(|p| p.id == pair_id) {
                        log::debug!("Storing example data for pair {}: source_id={}, target_id={}",
                            pair_id, pair.source_record_id, pair.target_record_id);

                        log::debug!("Source data has {} top-level fields",
                            source_data.as_object().map(|o| o.len()).unwrap_or(0));
                        if let Some(obj) = source_data.as_object() {
                            log::debug!("Source field names: {:?}", obj.keys().collect::<Vec<_>>());
                        }

                        log::debug!("Target data has {} top-level fields",
                            target_data.as_object().map(|o| o.len()).unwrap_or(0));
                        if let Some(obj) = target_data.as_object() {
                            log::debug!("Target field names: {:?}", obj.keys().collect::<Vec<_>>());
                        }

                        // Use composite keys: entity:record_id
                        // TODO: Support multi-entity mode - for now use first entity
                        let first_source_entity = state.source_entities.first().map(|s| s.as_str()).unwrap_or("");
                        let first_target_entity = state.target_entities.first().map(|s| s.as_str()).unwrap_or("");
                        let source_key = format!("{}:{}", first_source_entity, pair.source_record_id);
                        let target_key = format!("{}:{}", first_target_entity, pair.target_record_id);

                        log::debug!("Storing with keys: source='{}', target='{}'", source_key, target_key);

                        state.examples.cache.insert(source_key, source_data);
                        state.examples.cache.insert(target_key, target_data);

                        log::debug!("Cache now has {} entries", state.examples.cache.len());
                    } else {
                        log::warn!("No pair found with ID {} to store example data", pair_id);
                    }
                }
            }

            // Write complete metadata to cache and focus tree when both fully loaded
            // Support both single-entity and multi-entity modes
            let all_source_loaded = state.source_entities.iter().all(|entity| {
                matches!(state.source_metadata.get(entity), Some(Resource::Success(meta)) if !meta.fields.is_empty() && !meta.forms.is_empty() && !meta.views.is_empty())
            });

            let all_target_loaded = state.target_entities.iter().all(|entity| {
                matches!(state.target_metadata.get(entity), Some(Resource::Success(meta)) if !meta.fields.is_empty() && !meta.forms.is_empty() && !meta.views.is_empty())
            });

            if all_source_loaded && all_target_loaded {
                // Determine if multi-entity or single-entity mode
                let is_multi_entity = state.source_entities.len() > 1 || state.target_entities.len() > 1;

                if is_multi_entity {
                    // Multi-entity mode: compute matches across all entity pairs
                    // Extract successful metadata maps
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

                    // Compute matches for all entity pairs
                    let (field_matches, relationship_matches, entity_matches, source_related_entities, target_related_entities) =
                        super::super::matching_adapter::recompute_all_matches_multi(
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

                    // Collect available field types for type filtering (merge all entities)
                    state.available_source_types = source_metadata_map.values()
                        .flat_map(|meta| collect_field_types(&meta.fields))
                        .collect();
                    state.available_target_types = target_metadata_map.values()
                        .flat_map(|meta| collect_field_types(&meta.fields))
                        .collect();

                    // Cache all metadata objects asynchronously
                    for (entity, metadata) in source_metadata_map {
                        let source_env = state.source_env.clone();
                        tokio::spawn(async move {
                            let config = crate::global_config();
                            if let Err(e) = config.set_entity_metadata_cache(&source_env, &entity, &metadata).await {
                                log::error!("Failed to cache source metadata for {}/{}: {}", source_env, entity, e);
                            }
                        });
                    }

                    for (entity, metadata) in target_metadata_map {
                        let target_env = state.target_env.clone();
                        tokio::spawn(async move {
                            let config = crate::global_config();
                            if let Err(e) = config.set_entity_metadata_cache(&target_env, &entity, &metadata).await {
                                log::error!("Failed to cache target metadata for {}/{}: {}", target_env, entity, e);
                            }
                        });
                    }

                    return Command::set_focus("source_tree".into());
                } else {
                    // Single-entity mode: backwards compatible
                    let first_source_entity = state.source_entities.first().cloned().unwrap_or_default();
                    let first_target_entity = state.target_entities.first().cloned().unwrap_or_default();

                    if let (Some(Resource::Success(source)), Some(Resource::Success(target))) =
                        (state.source_metadata.get(&first_source_entity), state.target_metadata.get(&first_target_entity))
                    {
                        // Compute all matches using the extracted function
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

                        // Collect available field types for type filtering
                        state.available_source_types = collect_field_types(&source.fields);
                        state.available_target_types = collect_field_types(&target.fields);

                        // Cache both metadata objects asynchronously
                        let source_env = state.source_env.clone();
                        let source_entity = first_source_entity.clone();
                        let source_meta = source.clone();
                        tokio::spawn(async move {
                            let config = crate::global_config();
                            if let Err(e) = config.set_entity_metadata_cache(&source_env, &source_entity, &source_meta).await {
                                log::error!("Failed to cache source metadata for {}/{}: {}", source_env, source_entity, e);
                            } else {
                                log::debug!("Cached source metadata for {}/{}", source_env, source_entity);
                            }
                        });

                        let target_env = state.target_env.clone();
                        let target_entity = first_target_entity.clone();
                        let target_meta = target.clone();
                        tokio::spawn(async move {
                            let config = crate::global_config();
                            if let Err(e) = config.set_entity_metadata_cache(&target_env, &target_entity, &target_meta).await {
                                log::error!("Failed to cache target metadata for {}/{}: {}", target_env, target_entity, e);
                            } else {
                                log::debug!("Cached target metadata for {}/{}", target_env, target_entity);
                            }
                        });

                        return Command::set_focus("source_tree".into());
                    }
                }
            }
        }
        Err(e) => {
            log::error!("Failed to load metadata: {}", e);
            // Navigate to error screen
            return Command::start_app(
                AppId::ErrorScreen,
                crate::tui::apps::screens::ErrorScreenParams {
                    message: format!("Failed to load entity metadata:\n\n{}", e),
                    target: Some(AppId::MigrationComparisonSelect),
                }
            );
        }
    }

    Command::None
}

pub fn handle_mappings_loaded(
    state: &mut State,
    field_mappings: HashMap<String, Vec<String>>,
    prefix_mappings: HashMap<String, Vec<String>>,
    imported_mappings: HashMap<String, Vec<String>>,
    import_source_file: Option<String>,
    example_pairs: Vec<ExamplePair>,
    ignored_items: std::collections::HashSet<String>,
    negative_matches: std::collections::HashSet<String>,
) -> Command<Msg> {
    // Update state with loaded mappings and examples
    state.field_mappings = field_mappings;
    state.prefix_mappings = prefix_mappings;
    state.imported_mappings = imported_mappings;
    state.import_source_file = import_source_file;
    state.examples.pairs = example_pairs.clone();
    state.ignored_items = ignored_items;
    state.negative_matches = negative_matches;

    // Set first pair as active if any exist
    if !state.examples.pairs.is_empty() {
        state.examples.active_pair_id = Some(state.examples.pairs[0].id.clone());
    }

    // Load metadata for ALL selected entities (multi-entity support)
    let mut builder = Command::perform_parallel();

    // Source entities: load fields, forms, views for each
    for source_entity in &state.source_entities {
        builder = builder
            .add_task(
                format!("Loading {} fields ({})", source_entity, state.source_env),
                {
                    let env = state.source_env.clone();
                    let entity = source_entity.clone();
                    async move {
                        fetch_with_cache(&env, &entity, FetchType::SourceFields, true).await
                    }
                }
            )
            .add_task(
                format!("Loading {} forms ({})", source_entity, state.source_env),
                {
                    let env = state.source_env.clone();
                    let entity = source_entity.clone();
                    async move {
                        fetch_with_cache(&env, &entity, FetchType::SourceForms, true).await
                    }
                }
            )
            .add_task(
                format!("Loading {} views ({})", source_entity, state.source_env),
                {
                    let env = state.source_env.clone();
                    let entity = source_entity.clone();
                    async move {
                        fetch_with_cache(&env, &entity, FetchType::SourceViews, true).await
                    }
                }
            );
    }

    // Target entities: load fields, forms, views for each
    for target_entity in &state.target_entities {
        builder = builder
            .add_task(
                format!("Loading {} fields ({})", target_entity, state.target_env),
                {
                    let env = state.target_env.clone();
                    let entity = target_entity.clone();
                    async move {
                        fetch_with_cache(&env, &entity, FetchType::TargetFields, true).await
                    }
                }
            )
            .add_task(
                format!("Loading {} forms ({})", target_entity, state.target_env),
                {
                    let env = state.target_env.clone();
                    let entity = target_entity.clone();
                    async move {
                        fetch_with_cache(&env, &entity, FetchType::TargetForms, true).await
                    }
                }
            )
            .add_task(
                format!("Loading {} views ({})", target_entity, state.target_env),
                {
                    let env = state.target_env.clone();
                    let entity = target_entity.clone();
                    async move {
                        fetch_with_cache(&env, &entity, FetchType::TargetViews, true).await
                    }
                }
            );
    }

    // Add example data fetching tasks
    // For multi-entity mode, we need to parse entity from pair IDs
    for pair in example_pairs {
        let pair_id = pair.id.clone();
        let source_env = state.source_env.clone();
        let target_env = state.target_env.clone();

        // Parse entity names from pair (format: "entity1:id1:entity2:id2")
        let (source_entity, source_record_id, target_entity, target_record_id) =
            if let Some((source_part, target_part)) = pair_id.split_once(':') {
                // Try to parse as "source_entity:source_id:target_entity:target_id"
                let parts: Vec<&str> = pair_id.split(':').collect();
                if parts.len() >= 4 {
                    (parts[0].to_string(), parts[1].to_string(), parts[2].to_string(), parts[3].to_string())
                } else {
                    // Fallback: use first entities
                    (
                        state.source_entities.first().cloned().unwrap_or_default(),
                        pair.source_record_id.clone(),
                        state.target_entities.first().cloned().unwrap_or_default(),
                        pair.target_record_id.clone(),
                    )
                }
            } else {
                // Fallback: use first entities
                (
                    state.source_entities.first().cloned().unwrap_or_default(),
                    pair.source_record_id.clone(),
                    state.target_entities.first().cloned().unwrap_or_default(),
                    pair.target_record_id.clone(),
                )
            };

        builder = builder.add_task(
            format!("Loading example: {}", pair.display_name()),
            async move {
                super::super::fetch_example_pair_data(
                    &source_env,
                    &source_entity,
                    &source_record_id,
                    &target_env,
                    &target_entity,
                    &target_record_id,
                ).await
                .map(|(source, target)| FetchedData::ExampleData(pair_id, source, target))
                .map_err(|e| e.to_string())
            }
        );
    }

    builder
        .with_title("Loading Entity Comparison")
        .on_complete(AppId::EntityComparison)
        .on_cancel(AppId::MigrationComparisonSelect)
        .cancellable(true)
        .build(|_task_idx, result| {
            let data = result.downcast::<Result<FetchedData, String>>().unwrap();
            Msg::ParallelDataLoaded(0, *data)
        })
}

pub fn handle_refresh(state: &mut State) -> Command<Msg> {
    // Multi-entity support: refresh ALL selected entities
    // Mark all source/target metadata as loading
    for source_entity in &state.source_entities {
        state.source_metadata.insert(source_entity.clone(), Resource::Loading);
    }
    for target_entity in &state.target_entities {
        state.target_metadata.insert(target_entity.clone(), Resource::Loading);
    }

    // Clear example cache to force re-fetch
    state.examples.cache.clear();

    let mut builder = Command::perform_parallel();

    // Source entities: refresh fields, forms, views for each (bypass cache)
    for source_entity in &state.source_entities {
        builder = builder
            .add_task(
                format!("Refreshing {} fields ({})", source_entity, state.source_env),
                {
                    let env = state.source_env.clone();
                    let entity = source_entity.clone();
                    async move {
                        fetch_with_cache(&env, &entity, FetchType::SourceFields, false).await
                    }
                }
            )
            .add_task(
                format!("Refreshing {} forms ({})", source_entity, state.source_env),
                {
                    let env = state.source_env.clone();
                    let entity = source_entity.clone();
                    async move {
                        fetch_with_cache(&env, &entity, FetchType::SourceForms, false).await
                    }
                }
            )
            .add_task(
                format!("Refreshing {} views ({})", source_entity, state.source_env),
                {
                    let env = state.source_env.clone();
                    let entity = source_entity.clone();
                    async move {
                        fetch_with_cache(&env, &entity, FetchType::SourceViews, false).await
                    }
                }
            );
    }

    // Target entities: refresh fields, forms, views for each (bypass cache)
    for target_entity in &state.target_entities {
        builder = builder
            .add_task(
                format!("Refreshing {} fields ({})", target_entity, state.target_env),
                {
                    let env = state.target_env.clone();
                    let entity = target_entity.clone();
                    async move {
                        fetch_with_cache(&env, &entity, FetchType::TargetFields, false).await
                    }
                }
            )
            .add_task(
                format!("Refreshing {} forms ({})", target_entity, state.target_env),
                {
                    let env = state.target_env.clone();
                    let entity = target_entity.clone();
                    async move {
                        fetch_with_cache(&env, &entity, FetchType::TargetForms, false).await
                    }
                }
            )
            .add_task(
                format!("Refreshing {} views ({})", target_entity, state.target_env),
                {
                    let env = state.target_env.clone();
                    let entity = target_entity.clone();
                    async move {
                        fetch_with_cache(&env, &entity, FetchType::TargetViews, false).await
                    }
                }
            );
    }

    // Add example data fetching tasks
    // Parse entity names from pair IDs for multi-entity support
    for pair in &state.examples.pairs {
        let pair_id = pair.id.clone();
        let source_env = state.source_env.clone();
        let target_env = state.target_env.clone();

        // Parse entity names from pair (format: "entity1:id1:entity2:id2")
        let (source_entity, source_record_id, target_entity, target_record_id) =
            if let Some((source_part, target_part)) = pair_id.split_once(':') {
                // Try to parse as "source_entity:source_id:target_entity:target_id"
                let parts: Vec<&str> = pair_id.split(':').collect();
                if parts.len() >= 4 {
                    (parts[0].to_string(), parts[1].to_string(), parts[2].to_string(), parts[3].to_string())
                } else {
                    // Fallback: use first entities
                    (
                        state.source_entities.first().cloned().unwrap_or_default(),
                        pair.source_record_id.clone(),
                        state.target_entities.first().cloned().unwrap_or_default(),
                        pair.target_record_id.clone(),
                    )
                }
            } else {
                // Fallback: use first entities
                (
                    state.source_entities.first().cloned().unwrap_or_default(),
                    pair.source_record_id.clone(),
                    state.target_entities.first().cloned().unwrap_or_default(),
                    pair.target_record_id.clone(),
                )
            };

        builder = builder.add_task(
            format!("Refreshing example: {}", pair.display_name()),
            async move {
                super::super::fetch_example_pair_data(
                    &source_env,
                    &source_entity,
                    &source_record_id,
                    &target_env,
                    &target_entity,
                    &target_record_id,
                ).await
                .map(|(source, target)| FetchedData::ExampleData(pair_id, source, target))
                .map_err(|e| e.to_string())
            }
        );
    }

    builder
        .with_title("Refreshing Entity Comparison")
        .on_complete(AppId::EntityComparison)
        .on_cancel(AppId::MigrationComparisonSelect)
        .cancellable(true)
        .build(|_task_idx, result| {
            let data = result.downcast::<Result<FetchedData, String>>().unwrap();
            Msg::ParallelDataLoaded(0, *data)
        })
}
