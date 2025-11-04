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
                        let config = crate::global_config();
                        let source_env = state.source_env.clone();
                        let source_entity = state.source_entity.clone();
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

                    if let Resource::Success(ref mut meta) = state.source_metadata {
                        meta.fields = fields;
                        meta.relationships = relationships;
                    } else {
                        state.source_metadata = Resource::Success(crate::api::EntityMetadata {
                            fields,
                            relationships,
                            ..Default::default()
                        });
                    }
                }
                FetchedData::SourceForms(forms) => {
                    if let Resource::Success(ref mut meta) = state.source_metadata {
                        meta.forms = forms;
                    } else {
                        state.source_metadata = Resource::Success(crate::api::EntityMetadata {
                            forms,
                            ..Default::default()
                        });
                    }
                }
                FetchedData::SourceViews(views) => {
                    if let Resource::Success(ref mut meta) = state.source_metadata {
                        meta.views = views;
                    } else {
                        state.source_metadata = Resource::Success(crate::api::EntityMetadata {
                            views,
                            ..Default::default()
                        });
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
                        let config = crate::global_config();
                        let target_env = state.target_env.clone();
                        let target_entity = state.target_entity.clone();
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

                    if let Resource::Success(ref mut meta) = state.target_metadata {
                        meta.fields = fields;
                        meta.relationships = relationships;
                    } else {
                        state.target_metadata = Resource::Success(crate::api::EntityMetadata {
                            fields,
                            relationships,
                            ..Default::default()
                        });
                    }
                }
                FetchedData::TargetForms(forms) => {
                    if let Resource::Success(ref mut meta) = state.target_metadata {
                        meta.forms = forms;
                    } else {
                        state.target_metadata = Resource::Success(crate::api::EntityMetadata {
                            forms,
                            ..Default::default()
                        });
                    }
                }
                FetchedData::TargetViews(views) => {
                    if let Resource::Success(ref mut meta) = state.target_metadata {
                        meta.views = views;
                    } else {
                        state.target_metadata = Resource::Success(crate::api::EntityMetadata {
                            views,
                            ..Default::default()
                        });
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
                        let source_key = format!("{}:{}", state.source_entity, pair.source_record_id);
                        let target_key = format!("{}:{}", state.target_entity, pair.target_record_id);

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
            if let (Resource::Success(source), Resource::Success(target)) =
                (&state.source_metadata, &state.target_metadata)
            {
                if !source.fields.is_empty() && !target.fields.is_empty()
                    && !source.forms.is_empty() && !target.forms.is_empty()
                    && !source.views.is_empty() && !target.views.is_empty() {

                    // Compute all matches using the extracted function
                    let (field_matches, relationship_matches, entity_matches, source_entities, target_entities) =
                        recompute_all_matches(
                            source,
                            target,
                            &state.field_mappings,
                            &state.imported_mappings,
                            &state.prefix_mappings,
                            &state.examples,
                            &state.source_entity,
                            &state.target_entity,
                &state.negative_matches,
                        );

                    state.field_matches = field_matches;
                    state.relationship_matches = relationship_matches;
                    state.entity_matches = entity_matches;
                    state.source_entities = source_entities;
                    state.target_entities = target_entities;

                    // Collect available field types for type filtering
                    state.available_source_types = collect_field_types(&source.fields);
                    state.available_target_types = collect_field_types(&target.fields);

                    // Cache both metadata objects asynchronously
                    let source_env = state.source_env.clone();
                    let source_entity = state.source_entity.clone();
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
                    let target_entity = state.target_entity.clone();
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

    // Now load metadata + example data in one parallel batch
    let mut builder = Command::perform_parallel()
        // Source entity fetches
        .add_task(
            format!("Loading {} fields ({})", state.source_entity, state.source_env),
            {
                let env = state.source_env.clone();
                let entity = state.source_entity.clone();
                async move {
                    fetch_with_cache(&env, &entity, FetchType::SourceFields, true).await
                }
            }
        )
        .add_task(
            format!("Loading {} forms ({})", state.source_entity, state.source_env),
            {
                let env = state.source_env.clone();
                let entity = state.source_entity.clone();
                async move {
                    fetch_with_cache(&env, &entity, FetchType::SourceForms, true).await
                }
            }
        )
        .add_task(
            format!("Loading {} views ({})", state.source_entity, state.source_env),
            {
                let env = state.source_env.clone();
                let entity = state.source_entity.clone();
                async move {
                    fetch_with_cache(&env, &entity, FetchType::SourceViews, true).await
                }
            }
        )
        // Target entity fetches
        .add_task(
            format!("Loading {} fields ({})", state.target_entity, state.target_env),
            {
                let env = state.target_env.clone();
                let entity = state.target_entity.clone();
                async move {
                    fetch_with_cache(&env, &entity, FetchType::TargetFields, true).await
                }
            }
        )
        .add_task(
            format!("Loading {} forms ({})", state.target_entity, state.target_env),
            {
                let env = state.target_env.clone();
                let entity = state.target_entity.clone();
                async move {
                    fetch_with_cache(&env, &entity, FetchType::TargetForms, true).await
                }
            }
        )
        .add_task(
            format!("Loading {} views ({})", state.target_entity, state.target_env),
            {
                let env = state.target_env.clone();
                let entity = state.target_entity.clone();
                async move {
                    fetch_with_cache(&env, &entity, FetchType::TargetViews, true).await
                }
            }
        );

    // Add example data fetching tasks
    for pair in example_pairs {
        let pair_id = pair.id.clone();
        let source_env = state.source_env.clone();
        let source_entity = state.source_entity.clone();
        let source_record_id = pair.source_record_id.clone();
        let target_env = state.target_env.clone();
        let target_entity = state.target_entity.clone();
        let target_record_id = pair.target_record_id.clone();

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
    // Re-fetch metadata for both entities
    state.source_metadata = Resource::Loading;
    state.target_metadata = Resource::Loading;

    // Clear example cache to force re-fetch
    state.examples.cache.clear();

    let mut builder = Command::perform_parallel()
        // Source entity fetches - bypass cache for manual refresh
        .add_task(
            format!("Refreshing {} fields ({})", state.source_entity, state.source_env),
            {
                let env = state.source_env.clone();
                let entity = state.source_entity.clone();
                async move {
                    fetch_with_cache(&env, &entity, FetchType::SourceFields, false).await
                }
            }
        )
        .add_task(
            format!("Refreshing {} forms ({})", state.source_entity, state.source_env),
            {
                let env = state.source_env.clone();
                let entity = state.source_entity.clone();
                async move {
                    fetch_with_cache(&env, &entity, FetchType::SourceForms, false).await
                }
            }
        )
        .add_task(
            format!("Refreshing {} views ({})", state.source_entity, state.source_env),
            {
                let env = state.source_env.clone();
                let entity = state.source_entity.clone();
                async move {
                    fetch_with_cache(&env, &entity, FetchType::SourceViews, false).await
                }
            }
        )
        // Target entity fetches - bypass cache for manual refresh
        .add_task(
            format!("Refreshing {} fields ({})", state.target_entity, state.target_env),
            {
                let env = state.target_env.clone();
                let entity = state.target_entity.clone();
                async move {
                    fetch_with_cache(&env, &entity, FetchType::TargetFields, false).await
                }
            }
        )
        .add_task(
            format!("Refreshing {} forms ({})", state.target_entity, state.target_env),
            {
                let env = state.target_env.clone();
                let entity = state.target_entity.clone();
                async move {
                    fetch_with_cache(&env, &entity, FetchType::TargetForms, false).await
                }
            }
        )
        .add_task(
            format!("Refreshing {} views ({})", state.target_entity, state.target_env),
            {
                let env = state.target_env.clone();
                let entity = state.target_entity.clone();
                async move {
                    fetch_with_cache(&env, &entity, FetchType::TargetViews, false).await
                }
            }
        );

    // Add example data fetching tasks
    for pair in &state.examples.pairs {
        let pair_id = pair.id.clone();
        let source_env = state.source_env.clone();
        let source_entity = state.source_entity.clone();
        let source_record_id = pair.source_record_id.clone();
        let target_env = state.target_env.clone();
        let target_entity = state.target_entity.clone();
        let target_record_id = pair.target_record_id.clone();

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
