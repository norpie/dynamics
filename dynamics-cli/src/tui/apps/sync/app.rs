//! Entity Sync App - Main Application
//!
//! TUI wizard for syncing "settings" entities between Dynamics 365 environments.
//! Implements the App trait with Elm-style architecture.

use crossterm::event::KeyCode;
use ratatui::text::{Line, Span};

use crate::tui::{
    app::App,
    command::Command,
    element::FocusId,
    subscription::Subscription,
    LayeredView,
};

use super::msg::Msg;
use super::state::{AnalysisPhase, State};
use super::steps::{
    render_analysis, render_confirm, render_diff_review, render_entity_select,
    render_environment_select,
};
use super::types::SyncStep;

/// Entity Sync App - wizard for syncing entities between environments
pub struct EntitySyncApp;

impl App for EntitySyncApp {
    type State = State;
    type Msg = Msg;
    type InitParams = ();

    fn init(_params: ()) -> (State, Command<Msg>) {
        let state = State::default();

        // Load environments on init
        let cmd = Command::perform(
            async {
                let config = crate::global_config();

                // Get list of environment names
                let env_names = config
                    .list_environments()
                    .await
                    .map_err(|e| e.to_string())?;

                // Fetch full environment details for each
                let mut environments = Vec::new();
                for env_name in env_names {
                    if let Ok(Some(env)) = config.get_environment(&env_name).await {
                        environments.push(env);
                    }
                }

                Ok(environments)
            },
            Msg::EnvironmentsLoaded,
        );

        (state, cmd)
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            // === Navigation ===
            Msg::Back => handle_back(state),
            Msg::Next => handle_next(state),
            Msg::ConfirmBack => {
                // User confirmed going back with unsaved changes
                if let Some(prev) = state.step.prev() {
                    state.step = prev;
                }
                Command::None
            }
            Msg::CancelBack => Command::None,

            // === Step 1: Environment Selection ===
            Msg::EnvironmentsLoaded(result) => {
                match result {
                    Ok(envs) => {
                        state.env_select.environments =
                            crate::tui::Resource::Success(envs);
                    }
                    Err(e) => {
                        state.env_select.environments =
                            crate::tui::Resource::Failure(e);
                    }
                }
                Command::None
            }
            Msg::OriginListNavigate(key) => {
                let env_count = state
                    .env_select
                    .environments
                    .as_ref()
                    .map(|e| e.len())
                    .unwrap_or(0);
                state
                    .env_select
                    .origin_list
                    .handle_key(key, env_count, 20);
                Command::None
            }
            Msg::TargetListNavigate(key) => {
                let env_count = state
                    .env_select
                    .environments
                    .as_ref()
                    .map(|e| e.len())
                    .unwrap_or(0);
                state
                    .env_select
                    .target_list
                    .handle_key(key, env_count, 20);
                Command::None
            }
            Msg::OriginListSelect(idx) => {
                if let crate::tui::Resource::Success(envs) = &state.env_select.environments {
                    if let Some(env) = envs.get(idx) {
                        state.env_select.origin_env = Some(env.name.clone());
                    }
                }
                Command::None
            }
            Msg::TargetListSelect(idx) => {
                if let crate::tui::Resource::Success(envs) = &state.env_select.environments {
                    if let Some(env) = envs.get(idx) {
                        state.env_select.target_env = Some(env.name.clone());
                    }
                }
                Command::None
            }
            Msg::SwitchEnvFocus => {
                state.env_select.origin_focused = !state.env_select.origin_focused;
                let focus_id = if state.env_select.origin_focused {
                    "origin-env-list"
                } else {
                    "target-env-list"
                };
                Command::set_focus(FocusId::new(focus_id))
            }
            Msg::OriginListClicked(idx) => Self::update(state, Msg::OriginListSelect(idx)),
            Msg::TargetListClicked(idx) => Self::update(state, Msg::TargetListSelect(idx)),

            // === Step 2: Entity Selection ===
            Msg::EntitiesLoaded(result) => {
                match result {
                    Ok(entities) => {
                        state.entity_select.available_entities =
                            crate::tui::Resource::Success(entities);
                    }
                    Err(e) => {
                        state.entity_select.available_entities =
                            crate::tui::Resource::Failure(e);
                    }
                }
                Command::None
            }
            Msg::EntityListNavigate(key) => {
                let count = state.entity_select.filtered_entities().len();
                state.entity_select.entity_list.handle_key(key, count, 20);
                Command::None
            }
            Msg::EntityListToggle(idx) => {
                let filtered = state.entity_select.filtered_entities();
                if let Some(entity) = filtered.get(idx) {
                    let name = entity.logical_name.clone();
                    if state.entity_select.selected_entities.contains(&name) {
                        state.entity_select.selected_entities.remove(&name);
                    } else {
                        state.entity_select.selected_entities.insert(name);
                    }
                }
                Command::None
            }
            Msg::SelectAllEntities => {
                // Collect names first to avoid borrow issues
                let names: Vec<String> = state
                    .entity_select
                    .filtered_entities()
                    .iter()
                    .map(|e| e.logical_name.clone())
                    .collect();
                for name in names {
                    state.entity_select.selected_entities.insert(name);
                }
                Command::None
            }
            Msg::DeselectAllEntities => {
                state.entity_select.selected_entities.clear();
                Command::None
            }
            Msg::FilterInputEvent(event) => {
                use crate::tui::widgets::TextInputEvent;
                match event {
                    TextInputEvent::Changed(key) => {
                        if let Some(new_value) = state.entity_select.filter_input.handle_key(
                            key,
                            &state.entity_select.filter_text,
                            None,
                        ) {
                            state.entity_select.filter_text = new_value;
                            // Reset list state when filter changes to avoid out-of-bounds
                            state.entity_select.entity_list = Default::default();
                        }
                    }
                    TextInputEvent::Submit => {}
                }
                Command::None
            }
            Msg::ClearFilter => {
                state.entity_select.filter_text.clear();
                // Reset list state when filter clears
                state.entity_select.entity_list = Default::default();
                Command::None
            }
            Msg::JunctionCandidatesLoaded(candidates) => {
                state.entity_select.junction_candidates = candidates;
                Command::None
            }
            Msg::JunctionListNavigate(key) => {
                let count = state.entity_select.junction_candidates.len();
                state.entity_select.junction_list.handle_key(key, count, 10);
                Command::None
            }
            Msg::JunctionListToggle(idx) => {
                if let Some(junction) = state.entity_select.junction_candidates.get(idx) {
                    let name = junction.logical_name.clone();
                    if state.entity_select.included_junctions.contains(&name) {
                        state.entity_select.included_junctions.remove(&name);
                    } else {
                        state.entity_select.included_junctions.insert(name);
                    }
                }
                Command::None
            }
            Msg::ToggleJunctionPanel => {
                state.entity_select.show_junctions = !state.entity_select.show_junctions;
                Command::None
            }
            Msg::SwitchEntityFocus => {
                state.entity_select.entities_focused = !state.entity_select.entities_focused;
                let focus_id = if state.entity_select.entities_focused {
                    "entity-list"
                } else {
                    "junction-list"
                };
                Command::set_focus(FocusId::new(focus_id))
            }
            Msg::IncludeAllJunctions => {
                for junction in &state.entity_select.junction_candidates {
                    state
                        .entity_select
                        .included_junctions
                        .insert(junction.logical_name.clone());
                }
                Command::None
            }
            Msg::ExcludeAllJunctions => {
                state.entity_select.included_junctions.clear();
                Command::None
            }

            // === Step 3: Analysis ===
            Msg::StartAnalysis => {
                state.analysis.phase = AnalysisPhase::FetchingOriginSchema;
                state.analysis.progress = 0;
                state.analysis.status_message = "Starting analysis...".to_string();
                // In a real implementation, this would kick off async analysis
                Command::None
            }
            Msg::AnalysisPhaseChanged(phase) => {
                state.analysis.phase = phase;
                Command::None
            }
            Msg::AnalysisProgress(progress, message) => {
                state.analysis.progress = progress;
                state.analysis.status_message = message;
                Command::None
            }
            Msg::AnalysisComplete(plan) => {
                state.sync_plan = Some(*plan);
                state.analysis.phase = AnalysisPhase::Complete;
                state.analysis.progress = 100;
                Command::None
            }
            Msg::AnalysisFailed(error) => {
                state.error = Some(error);
                Command::None
            }
            Msg::CancelAnalysis => {
                // Go back to entity selection
                state.step = SyncStep::EntitySelect;
                state.analysis = Default::default();
                Command::None
            }

            // === Step 4: Diff Review ===
            Msg::DiffEntityListNavigate(key) => {
                let count = state
                    .sync_plan
                    .as_ref()
                    .map(|p| p.entity_plans.len())
                    .unwrap_or(0);
                state.diff_review.entity_list.handle_key(key, count, 20);
                Command::None
            }
            Msg::DiffEntityListSelect(idx) => {
                state.diff_review.selected_entity_idx = idx;
                // Reset field list when selecting a new entity
                state.diff_review.field_list = Default::default();
                Command::None
            }
            Msg::DiffFieldListNavigate(key) => {
                // Get field count for current entity
                let count = state
                    .sync_plan
                    .as_ref()
                    .and_then(|p| p.entity_plans.get(state.diff_review.selected_entity_idx))
                    .map(|e| {
                        e.schema_diff.fields_to_add.len()
                            + e.schema_diff.fields_type_mismatch.len()
                            + e.schema_diff.fields_target_only.len()
                            + e.schema_diff.fields_in_both.len()
                    })
                    .unwrap_or(0);
                state.diff_review.field_list.handle_key(key, count, 15);
                Command::None
            }
            Msg::DiffNextTab => {
                state.diff_review.active_tab = state.diff_review.active_tab.next();
                Command::None
            }
            Msg::DiffPrevTab => {
                state.diff_review.active_tab = state.diff_review.active_tab.prev();
                Command::None
            }
            Msg::DiffToggleSection(section) => {
                if state.diff_review.expanded_sections.contains(&section) {
                    state.diff_review.expanded_sections.remove(&section);
                } else {
                    state.diff_review.expanded_sections.insert(section);
                }
                Command::None
            }
            Msg::DiffSetViewportHeight(height) => {
                state.diff_review.field_list.set_viewport_height(height);
                Command::None
            }

            // === Step 5: Confirm ===
            Msg::ToggleConfirm => {
                state.confirm.confirmed = !state.confirm.confirmed;
                Command::None
            }
            Msg::Execute => {
                if !state.confirm.confirmed {
                    return Command::None;
                }
                state.confirm.executing = true;
                state.confirm.execution_progress = 0;
                state.confirm.execution_status = "Starting sync...".to_string();
                // In a real implementation, this would kick off the sync
                Command::None
            }
            Msg::ExportReport => {
                if let Some(ref plan) = state.sync_plan {
                    let report = super::logic::build_pre_execution_report(plan);
                    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                    let filename = format!("sync_report_{}.xlsx", timestamp);

                    return Command::perform(
                        async move {
                            super::logic::export_report_to_excel(&report, &filename)
                                .map(|_| filename)
                                .map_err(|e| e.to_string())
                        },
                        Msg::ReportExported,
                    );
                }
                Command::None
            }
            Msg::ReportExported(result) => {
                match result {
                    Ok(path) => {
                        state.confirm.export_path = Some(path.clone());
                        super::logic::try_open_file(&path);
                    }
                    Err(e) => {
                        state.error = Some(format!("Failed to export report: {}", e));
                    }
                }
                Command::None
            }
            Msg::ExecutionStarted => {
                state.confirm.executing = true;
                Command::None
            }
            Msg::ExecutionProgress(progress, status) => {
                state.confirm.execution_progress = progress;
                state.confirm.execution_status = status;
                Command::None
            }
            Msg::ExecutionComplete(result) => {
                state.confirm.executing = false;
                match result {
                    Ok(()) => {
                        state.confirm.execution_status = "Sync completed successfully!".to_string();
                    }
                    Err(e) => {
                        state.error = Some(format!("Sync failed: {}", e));
                    }
                }
                Command::None
            }

            // === General ===
            Msg::DismissError => {
                state.error = None;
                Command::None
            }
            Msg::Noop => Command::None,
        }
    }

    fn view(state: &mut State) -> LayeredView<Msg> {
        let theme = &crate::global_runtime_config().theme;

        let main_view = match state.step {
            SyncStep::EnvironmentSelect => render_environment_select(state, theme),
            SyncStep::EntitySelect => render_entity_select(state, theme),
            SyncStep::Analysis => render_analysis(state, theme),
            SyncStep::DiffReview => render_diff_review(state, theme),
            SyncStep::Confirm => render_confirm(state, theme),
        };

        LayeredView::new(main_view)
    }

    fn subscriptions(state: &State) -> Vec<Subscription<Msg>> {
        let mut subs = vec![];

        // Global navigation
        subs.push(Subscription::keyboard(KeyCode::Esc, "Back", Msg::Back));

        match state.step {
            SyncStep::EnvironmentSelect => {
                subs.push(Subscription::keyboard(
                    KeyCode::Tab,
                    "Switch list",
                    Msg::SwitchEnvFocus,
                ));
                if state.env_select.can_proceed() {
                    subs.push(Subscription::keyboard(KeyCode::Enter, "Next", Msg::Next));
                }
            }
            SyncStep::EntitySelect => {
                subs.push(Subscription::keyboard(
                    KeyCode::Char(' '),
                    "Toggle",
                    Msg::Noop, // Handled by list widget
                ));
                subs.push(Subscription::keyboard(
                    KeyCode::Char('a'),
                    "Select all",
                    Msg::SelectAllEntities,
                ));
                subs.push(Subscription::keyboard(
                    KeyCode::Char('n'),
                    "Select none",
                    Msg::DeselectAllEntities,
                ));
                subs.push(Subscription::keyboard(
                    KeyCode::Char('j'),
                    "Toggle junctions",
                    Msg::ToggleJunctionPanel,
                ));
                subs.push(Subscription::keyboard(
                    KeyCode::Tab,
                    "Switch panel",
                    Msg::SwitchEntityFocus,
                ));
                if state.entity_select.can_proceed() {
                    subs.push(Subscription::keyboard(KeyCode::Enter, "Analyze", Msg::Next));
                }
            }
            SyncStep::Analysis => {
                if state.analysis.phase == AnalysisPhase::Complete {
                    subs.push(Subscription::keyboard(
                        KeyCode::Enter,
                        "Review",
                        Msg::Next,
                    ));
                }
            }
            SyncStep::DiffReview => {
                subs.push(Subscription::keyboard(
                    KeyCode::Char(']'),
                    "Next tab",
                    Msg::DiffNextTab,
                ));
                subs.push(Subscription::keyboard(
                    KeyCode::Char('['),
                    "Prev tab",
                    Msg::DiffPrevTab,
                ));
                subs.push(Subscription::keyboard(KeyCode::Enter, "Confirm", Msg::Next));
            }
            SyncStep::Confirm => {
                if !state.confirm.executing {
                    subs.push(Subscription::keyboard(
                        KeyCode::Char(' '),
                        "Toggle confirm",
                        Msg::ToggleConfirm,
                    ));
                    subs.push(Subscription::keyboard(
                        KeyCode::Char('e'),
                        "Export",
                        Msg::ExportReport,
                    ));
                    if state.confirm.confirmed {
                        subs.push(Subscription::keyboard(
                            KeyCode::Enter,
                            "Execute",
                            Msg::Execute,
                        ));
                    }
                }
            }
        }

        subs
    }

    fn title() -> &'static str {
        "Entity Sync"
    }

    fn status(state: &State) -> Option<Line<'static>> {
        let step_label = state.step.label();
        let step_num = state.step.number();
        Some(Line::from(vec![Span::raw(format!(
            "Step {}/5: {}",
            step_num, step_label
        ))]))
    }
}

/// Handle back navigation
fn handle_back(state: &mut State) -> Command<Msg> {
    match state.step {
        SyncStep::EnvironmentSelect => {
            // Exit the app
            Command::quit_self()
        }
        _ => {
            if let Some(prev) = state.step.prev() {
                state.step = prev;
            }
            Command::None
        }
    }
}

/// Handle forward navigation
fn handle_next(state: &mut State) -> Command<Msg> {
    match state.step {
        SyncStep::EnvironmentSelect => {
            if state.env_select.can_proceed() {
                state.step = SyncStep::EntitySelect;
                state.entity_select.available_entities = crate::tui::Resource::Loading;

                // Load entities from the origin environment
                let origin_env = state.env_select.origin_env.clone().unwrap();
                return Command::perform(
                    async move {
                        load_entities_for_env(&origin_env).await
                    },
                    Msg::EntitiesLoaded,
                );
            }
            Command::None
        }
        SyncStep::EntitySelect => {
            if state.entity_select.can_proceed() {
                state.step = SyncStep::Analysis;
                state.analysis = Default::default();
                state.analysis.phase = AnalysisPhase::FetchingOriginSchema;

                // Collect parameters for analysis
                let origin_env = state.env_select.origin_env.clone().unwrap();
                let target_env = state.env_select.target_env.clone().unwrap();
                let selected_entities: Vec<String> = state
                    .entity_select
                    .entities_to_sync()
                    .into_iter()
                    .collect();

                // Start async analysis
                return Command::perform(
                    async move {
                        run_analysis(&origin_env, &target_env, &selected_entities).await
                    },
                    |result| match result {
                        Ok(plan) => Msg::AnalysisComplete(Box::new(plan)),
                        Err(e) => Msg::AnalysisFailed(e),
                    },
                );
            }
            Command::None
        }
        SyncStep::Analysis => {
            if state.analysis.phase == AnalysisPhase::Complete {
                state.step = SyncStep::DiffReview;
            }
            Command::None
        }
        SyncStep::DiffReview => {
            state.step = SyncStep::Confirm;
            Command::None
        }
        SyncStep::Confirm => {
            // Already at the last step
            Command::None
        }
    }
}

/// Run the full analysis process
async fn run_analysis(
    origin_env: &str,
    target_env: &str,
    selected_entities: &[String],
) -> Result<super::types::SyncPlan, String> {
    use super::logic::{compare_schemas, DependencyGraph};
    use super::types::*;
    use crate::api::metadata::FieldType;
    use std::collections::HashSet;

    let manager = crate::client_manager();

    // Get clients for both environments
    let origin_client = manager
        .get_client(origin_env)
        .await
        .map_err(|e| format!("Failed to get origin client: {}", e))?;

    let target_client = manager
        .get_client(target_env)
        .await
        .map_err(|e| format!("Failed to get target client: {}", e))?;

    let selected_set: HashSet<String> = selected_entities.iter().cloned().collect();
    let mut entities_with_fields: Vec<(String, Option<String>, Vec<crate::api::metadata::FieldMetadata>)> = Vec::new();
    let mut entity_plans = Vec::new();
    let mut total_delete_count = 0usize;
    let mut total_insert_count = 0usize;
    let mut has_schema_changes = false;

    // Process each selected entity
    for entity_name in selected_entities {
        log::info!("Analyzing entity: {}", entity_name);

        // Fetch field metadata from both environments
        let origin_fields = origin_client
            .fetch_entity_fields_combined(entity_name)
            .await
            .map_err(|e| format!("Failed to fetch origin fields for {}: {}", entity_name, e))?;

        let target_fields = target_client
            .fetch_entity_fields_combined(entity_name)
            .await
            .unwrap_or_default(); // Target might not have the entity

        // Store for dependency graph
        entities_with_fields.push((entity_name.clone(), None, origin_fields.clone()));

        // Compare schemas (pass None for raw origin data - not needed for this use case)
        let schema_diff = compare_schemas(entity_name, &origin_fields, &target_fields, None);

        if schema_diff.has_changes() {
            has_schema_changes = true;
        }

        // Fetch actual records from both environments
        // Origin: all active records (statecode eq 0) with all field data
        // Target: just record IDs (to delete)
        log::info!("Fetching records for {}", entity_name);

        let origin_records = match fetch_all_records(&origin_client, entity_name, true).await {
            Ok(records) => records,
            Err(e) => {
                log::error!("Failed to fetch origin records for {}: {}", entity_name, e);
                vec![]
            }
        };
        let target_record_ids = match fetch_record_ids(&target_client, entity_name).await {
            Ok(ids) => ids,
            Err(e) => {
                log::error!("Failed to fetch target record IDs for {}: {}", entity_name, e);
                vec![]
            }
        };

        let origin_count = origin_records.len();
        let target_count = target_record_ids.len();

        total_delete_count += target_count;
        total_insert_count += origin_count;

        // Extract lookup info from fields
        let lookups: Vec<LookupInfo> = origin_fields
            .iter()
            .filter(|f| matches!(f.field_type, FieldType::Lookup))
            .filter_map(|f| {
                f.related_entity.as_ref().map(|target| LookupInfo {
                    field_name: f.logical_name.clone(),
                    target_entity: target.clone(),
                    is_internal: selected_set.contains(target),
                })
            })
            .collect();

        // Build entity plan
        entity_plans.push(EntitySyncPlan {
            entity_info: SyncEntityInfo {
                logical_name: entity_name.clone(),
                display_name: None,
                category: DependencyCategory::Standalone,
                lookups: lookups.clone(),
                dependents: vec![],
                insert_priority: 0,
                delete_priority: 0,
            },
            schema_diff,
            data_preview: EntityDataPreview {
                entity_name: entity_name.clone(),
                origin_count,
                target_count,
                origin_records,
                target_record_ids,
            },
            nulled_lookups: lookups
                .iter()
                .filter(|l| !l.is_internal)
                .map(|l| NulledLookupInfo {
                    entity_name: entity_name.clone(),
                    field_name: l.field_name.clone(),
                    target_entity: l.target_entity.clone(),
                    affected_count: origin_count,
                })
                .collect(),
        });
    }

    // Build dependency graph
    let dep_graph = DependencyGraph::build(entities_with_fields);

    // Get sorted order and update priorities
    if let Ok(sorted) = dep_graph.topological_sort() {
        for (insert_priority, name) in sorted.iter().enumerate() {
            if let Some(plan) = entity_plans.iter_mut().find(|p| &p.entity_info.logical_name == name) {
                plan.entity_info.insert_priority = insert_priority as u32;
                plan.entity_info.delete_priority = (sorted.len() - 1 - insert_priority) as u32;
                plan.entity_info.category = dep_graph.categorize(name);

                // Get dependents
                if let Some(deps) = dep_graph.dependents.get(name) {
                    plan.entity_info.dependents = deps.iter().cloned().collect();
                }
            }
        }
    }

    // Sort by insert priority
    entity_plans.sort_by_key(|p| p.entity_info.insert_priority);

    Ok(SyncPlan {
        origin_env: origin_env.to_string(),
        target_env: target_env.to_string(),
        entity_plans,
        detected_junctions: vec![], // Skip junction detection for now
        has_schema_changes,
        total_delete_count,
        total_insert_count,
    })
}

/// Load entities for a given environment (with caching)
async fn load_entities_for_env(env_name: &str) -> Result<Vec<super::state::EntityListItem>, String> {
    use super::state::EntityListItem;

    let config = crate::global_config();
    let manager = crate::client_manager();

    // Try cache first (24 hours)
    let entity_names = match config.get_entity_cache(env_name, 24).await {
        Ok(Some(cached)) => cached,
        _ => {
            // Cache miss - fetch from API
            let client = manager
                .get_client(env_name)
                .await
                .map_err(|e| format!("Failed to get client for {}: {}", env_name, e))?;

            let metadata_xml = client
                .fetch_metadata()
                .await
                .map_err(|e| format!("Failed to fetch metadata: {}", e))?;

            let entities = crate::api::metadata::parse_entity_list(&metadata_xml)
                .map_err(|e| format!("Failed to parse metadata: {}", e))?;

            // Cache for future use
            let _ = config.set_entity_cache(env_name, entities.clone()).await;

            entities
        }
    };

    // Convert to EntityListItem
    let entities: Vec<EntityListItem> = entity_names
        .into_iter()
        .map(|name| EntityListItem {
            logical_name: name,
            display_name: None,
            record_count: None,
        })
        .collect();

    Ok(entities)
}

/// Fetch all records for an entity (with pagination)
async fn fetch_all_records(
    client: &crate::api::DynamicsClient,
    entity_name: &str,
    active_only: bool,
) -> anyhow::Result<Vec<serde_json::Value>> {
    use crate::api::query::QueryBuilder;

    let mut all_records = Vec::new();

    // Entity set name is typically logical name + 's'
    let entity_set = format!("{}s", entity_name);
    let mut builder = QueryBuilder::new(&entity_set).top(5000);
    if active_only {
        builder = builder.active_only();
    }

    let mut result = client.execute_query(&builder.build()).await?;
    if let Some(ref data) = result.data {
        all_records.extend(data.value.clone());
    }

    while result.has_more() {
        if let Some(next) = result.next_page(client).await? {
            if let Some(ref data) = next.data {
                all_records.extend(data.value.clone());
            }
            result = next;
        } else {
            break;
        }
    }

    log::info!("Fetched {} records from {}", all_records.len(), entity_name);
    Ok(all_records)
}

/// Fetch record IDs for an entity (for deletion)
async fn fetch_record_ids(
    client: &crate::api::DynamicsClient,
    entity_name: &str,
) -> anyhow::Result<Vec<String>> {
    use crate::api::query::QueryBuilder;

    let pk_field = format!("{}id", entity_name);
    let mut all_ids = Vec::new();

    // Entity set name is typically logical name + 's'
    let entity_set = format!("{}s", entity_name);
    let query = QueryBuilder::new(&entity_set)
        .select(&[&pk_field])
        .top(5000)
        .build();

    let mut result = client.execute_query(&query).await?;

    if let Some(ref data) = result.data {
        for record in &data.value {
            if let Some(id) = record.get(&pk_field).and_then(|v| v.as_str()) {
                all_ids.push(id.to_string());
            }
        }
    }

    while result.has_more() {
        if let Some(next) = result.next_page(client).await? {
            if let Some(ref data) = next.data {
                for record in &data.value {
                    if let Some(id) = record.get(&pk_field).and_then(|v| v.as_str()) {
                        all_ids.push(id.to_string());
                    }
                }
            }
            result = next;
        } else {
            break;
        }
    }

    log::info!("Fetched {} record IDs from {}", all_ids.len(), entity_name);
    Ok(all_ids)
}
