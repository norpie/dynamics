//! Transfer Preview app - displays resolved records after transform

use std::collections::HashMap;

use crate::config::repository::transfer::get_transfer_config;
use crate::transfer::{ResolvedTransfer, TransferConfig, TransformEngine};
use crate::tui::resource::Resource;
use crate::tui::{App, AppId, Command, LayeredView, Subscription};

use super::state::{Msg, PreviewParams, State};
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
            // Data loading - Step 1: Config loaded, now fetch records
            Msg::ConfigLoaded(result) => {
                match result {
                    Ok(config) => {
                        // Build parallel fetch tasks for loading screen
                        let mut builder = Command::perform_parallel()
                            .with_title("Fetching Records");

                        let num_entities = config.entity_mappings.len();

                        // Add source fetch tasks
                        for mapping in &config.entity_mappings {
                            let entity = mapping.source_entity.clone();
                            let env = config.source_env.clone();
                            builder = builder.add_task(
                                format!("Source: {}", entity),
                                fetch_entity_records(env, entity.clone(), true),
                            );
                        }

                        // Add target fetch tasks
                        for mapping in &config.entity_mappings {
                            let entity = mapping.target_entity.clone();
                            let env = config.target_env.clone();
                            builder = builder.add_task(
                                format!("Target: {}", entity),
                                fetch_entity_records(env, entity.clone(), false),
                            );
                        }

                        // Track how many fetches we're waiting for (source + target for each entity)
                        state.pending_fetches = num_entities * 2;
                        state.config = Some(config);

                        builder
                            .on_complete(AppId::TransferPreview)
                            .build(|_task_idx, result| {
                                let data = result
                                    .downcast::<Result<(String, bool, Vec<serde_json::Value>), String>>()
                                    .unwrap();
                                Msg::FetchResult(*data)
                            })
                    }
                    Err(e) => {
                        state.resolved = Resource::Failure(e);
                        Command::None
                    }
                }
            }

            // Data loading - Step 2: Each fetch result comes in individually
            Msg::FetchResult(result) => {
                match result {
                    Ok((entity_name, is_source, records)) => {
                        if is_source {
                            state.source_data.insert(entity_name, records);
                        } else {
                            state.target_data.insert(entity_name, records);
                        }

                        state.pending_fetches = state.pending_fetches.saturating_sub(1);

                        // When all fetches complete, run the transform
                        if state.pending_fetches == 0 {
                            if let Some(config) = state.config.take() {
                                // Build primary keys map
                                let primary_keys: HashMap<String, String> = config
                                    .entity_mappings
                                    .iter()
                                    .map(|m| (m.source_entity.clone(), format!("{}id", m.source_entity)))
                                    .collect();

                                // Run transform
                                let resolved = TransformEngine::transform_all(
                                    &config,
                                    &state.source_data,
                                    &state.target_data,
                                    &primary_keys,
                                );

                                log::info!(
                                    "Transform complete: {} records ({} upsert, {} nochange, {} skip, {} error)",
                                    resolved.total_records(),
                                    resolved.upsert_count(),
                                    resolved.nochange_count(),
                                    resolved.skip_count(),
                                    resolved.error_count()
                                );

                                // Clear accumulated data
                                state.source_data.clear();
                                state.target_data.clear();

                                state.resolved = Resource::Success(resolved);
                            }
                        }
                    }
                    Err(e) => {
                        state.resolved = Resource::Failure(e);
                        state.pending_fetches = 0;
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
                // TODO (Chunk 4): Pass real item count and visible height
                let item_count = if let Resource::Success(resolved) = &state.resolved {
                    resolved.entities.get(state.current_entity_idx)
                        .map(|e| e.records.len())
                        .unwrap_or(0)
                } else {
                    0
                };
                let visible_height = 20; // Placeholder until table is implemented
                state.list_state.handle_event(event, item_count, visible_height);
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
                // TODO: Implement search handling
                let _ = event;
                Command::None
            }

            // Record actions
            Msg::ToggleSkip => {
                // TODO (Chunk 8): Implement skip toggle
                Command::None
            }

            Msg::ViewDetails => {
                if let Some(idx) = state.list_state.selected() {
                    state.active_modal = Some(super::state::PreviewModal::RecordDetails {
                        record_idx: idx,
                    });
                }
                Command::None
            }

            Msg::EditRecord => {
                if let Some(idx) = state.list_state.selected() {
                    state.active_modal = Some(super::state::PreviewModal::EditRecord {
                        record_idx: idx,
                    });
                }
                Command::None
            }

            Msg::SaveRecord => {
                // TODO (Chunk 7): Implement record save
                state.active_modal = None;
                Command::None
            }

            // Bulk actions
            Msg::OpenBulkActions => {
                state.active_modal = Some(super::state::PreviewModal::BulkActions);
                Command::None
            }

            Msg::ApplyBulkAction(_action) => {
                // TODO (Chunk 8): Implement bulk action
                state.active_modal = None;
                Command::None
            }

            // Excel
            Msg::ExportExcel => {
                // TODO (Chunk 9): Implement Excel export
                Command::None
            }

            Msg::ImportExcel => {
                // TODO (Chunk 10): Implement Excel import
                Command::None
            }

            Msg::ExportCompleted(result) => {
                match result {
                    Ok(path) => log::info!("Exported to {}", path),
                    Err(e) => log::error!("Export failed: {}", e),
                }
                Command::None
            }

            Msg::ImportCompleted(result) => {
                match result {
                    Ok(resolved) => {
                        state.resolved = Resource::Success(resolved);
                    }
                    Err(e) => log::error!("Import failed: {}", e),
                }
                state.active_modal = None;
                Command::None
            }

            // Refresh
            Msg::Refresh => {
                // TODO (Chunk 11): Re-run transform
                Command::None
            }

            // Modal
            Msg::CloseModal => {
                state.active_modal = None;
                Command::None
            }

            // Navigation
            Msg::Back => {
                Command::navigate_to(AppId::TransferMappingEditor)
            }

            Msg::GoToExecute => {
                // TODO (Chunk 12): Navigate to execute app
                log::info!("Would navigate to execute");
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
async fn fetch_entity_records(
    env_name: String,
    entity_name: String,
    is_source: bool,
) -> Result<(String, bool, Vec<serde_json::Value>), String> {
    use crate::api::pluralization::pluralize_entity_name;
    use crate::api::query::QueryBuilder;

    let manager = crate::client_manager();
    let client = manager
        .get_client(&env_name)
        .await
        .map_err(|e| format!("Failed to get client for {}: {}", env_name, e))?;

    let entity_set = pluralize_entity_name(&entity_name);
    let mut all_records = Vec::new();

    let query = QueryBuilder::new(&entity_set)
        .active_only()
        .top(5000)
        .build();

    let mut result = client
        .execute_query(&query)
        .await
        .map_err(|e| format!("Query failed for {}: {}", entity_name, e))?;

    if let Some(ref data) = result.data {
        all_records.extend(data.value.clone());
    }

    // Handle pagination
    while result.has_more() {
        if let Some(next) = result
            .next_page(&client)
            .await
            .map_err(|e| format!("Pagination failed: {}", e))?
        {
            if let Some(ref data) = next.data {
                all_records.extend(data.value.clone());
            }
            result = next;
        } else {
            break;
        }
    }

    log::info!(
        "Fetched {} records for {} from {}",
        all_records.len(),
        entity_name,
        env_name
    );

    Ok((entity_name, is_source, all_records))
}
