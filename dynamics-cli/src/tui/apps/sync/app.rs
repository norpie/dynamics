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
                // Detect junction candidates after selection change
                state.entity_select.detect_junction_candidates();
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
                // Detect junction candidates after selection change
                state.entity_select.detect_junction_candidates();
                Command::None
            }
            Msg::DeselectAllEntities => {
                state.entity_select.selected_entities.clear();
                // Clear junction candidates when deselecting all
                state.entity_select.junction_candidates.clear();
                state.entity_select.included_junctions.clear();
                Command::None
            }
            Msg::PresetSelectEvent(event) => {
                use crate::tui::apps::sync::steps::entity_select::PRESETS;
                use crate::tui::widgets::SelectEvent;
                use crossterm::event::KeyCode;

                match event {
                    SelectEvent::Navigate(KeyCode::Enter) | SelectEvent::Navigate(KeyCode::Char(' '))
                        if !state.entity_select.preset_selector.is_open() =>
                    {
                        // Open dropdown when closed
                        state.entity_select.preset_selector.open();
                    }
                    SelectEvent::Navigate(KeyCode::Enter) | SelectEvent::Navigate(KeyCode::Char(' '))
                        if state.entity_select.preset_selector.is_open() =>
                    {
                        // Select highlighted item when open
                        state.entity_select.preset_selector.select_highlighted();
                        let selected_index = state.entity_select.preset_selector.selected();

                        // Apply the preset
                        if let Some(preset) = PRESETS.get(selected_index) {
                            if !preset.entities.is_empty() {
                                state.entity_select.selected_entities.clear();
                                for entity_name in preset.entities {
                                    state.entity_select.selected_entities.insert(entity_name.to_string());
                                }
                                log::info!("Applied preset '{}' with {} entities", preset.name, preset.entities.len());
                                // Detect junction candidates after preset applied
                                state.entity_select.detect_junction_candidates();
                            }
                        }
                    }
                    SelectEvent::Navigate(KeyCode::Esc) => {
                        // Close dropdown
                        if state.entity_select.preset_selector.is_open() {
                            state.entity_select.preset_selector.close();
                        }
                    }
                    SelectEvent::Select(idx) => {
                        // Handle click selection
                        state.entity_select.preset_selector.select(idx);
                        if let Some(preset) = PRESETS.get(idx) {
                            if !preset.entities.is_empty() {
                                state.entity_select.selected_entities.clear();
                                for entity_name in preset.entities {
                                    state.entity_select.selected_entities.insert(entity_name.to_string());
                                }
                                log::info!("Applied preset '{}' with {} entities", preset.name, preset.entities.len());
                                // Detect junction candidates after preset applied
                                state.entity_select.detect_junction_candidates();
                            }
                        }
                    }
                    SelectEvent::Blur => {
                        state.entity_select.preset_selector.close();
                    }
                    _ => {
                        // Handle other navigation (Up/Down arrows)
                        state.entity_select.preset_selector.handle_event(event);
                    }
                }
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
            Msg::DataListNavigate(key) => {
                // Get record count for current entity
                let count = state
                    .sync_plan
                    .as_ref()
                    .and_then(|p| p.entity_plans.get(state.diff_review.selected_entity_idx))
                    .map(|e| e.data_preview.origin_records.len())
                    .unwrap_or(0);
                state.diff_review.data_list.handle_key(key, count, 15);
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

/// Run the full analysis process with parallel fetching
async fn run_analysis(
    origin_env: &str,
    target_env: &str,
    selected_entities: &[String],
) -> Result<super::types::SyncPlan, String> {
    use super::logic::{compare_schemas, DependencyGraph};
    use super::types::*;
    use super::{init_analysis_progress, set_analysis_phase, set_analysis_complete,
                set_entity_schema_status, set_entity_records_status, FetchStatus};
    use crate::api::metadata::FieldType;
    use std::collections::HashSet;
    use std::sync::Arc;
    use futures::future::join_all;

    // Initialize progress tracking
    let entities_for_progress: Vec<(String, Option<String>)> = selected_entities
        .iter()
        .map(|e| (e.clone(), None))
        .collect();
    init_analysis_progress(&entities_for_progress);

    set_analysis_phase("Connecting to environments...");

    let manager = crate::client_manager();

    // Get clients for both environments
    let origin_client = Arc::new(manager
        .get_client(origin_env)
        .await
        .map_err(|e| format!("Failed to get origin client: {}", e))?);

    let target_client = Arc::new(manager
        .get_client(target_env)
        .await
        .map_err(|e| format!("Failed to get target client: {}", e))?);

    let selected_set: HashSet<String> = selected_entities.iter().cloned().collect();

    // Phase 1: Fetch all schemas and EntitySetNames in parallel
    set_analysis_phase("Fetching schemas...");

    let schema_futures: Vec<_> = selected_entities.iter().map(|entity_name| {
        let origin_client = Arc::clone(&origin_client);
        let target_client = Arc::clone(&target_client);
        let entity_name = entity_name.clone();

        async move {
            set_entity_schema_status(&entity_name, FetchStatus::Fetching);

            // Fetch fields and EntitySetName in parallel
            let (origin_fields, target_fields, entity_set_name) = tokio::join!(
                origin_client.fetch_entity_fields_combined(&entity_name),
                target_client.fetch_entity_fields_combined(&entity_name),
                origin_client.fetch_entity_set_name(&entity_name)
            );

            let origin_fields = origin_fields
                .map_err(|e| format!("Failed to fetch origin fields for {}: {}", entity_name, e));

            let target_fields = target_fields.unwrap_or_default();

            let entity_set_name = entity_set_name
                .map_err(|e| format!("Failed to fetch EntitySetName for {}: {}", entity_name, e));

            match (origin_fields, entity_set_name) {
                (Ok(fields), Ok(set_name)) => {
                    set_entity_schema_status(&entity_name, FetchStatus::Done);
                    Ok((entity_name, fields, target_fields, set_name))
                }
                (Err(e), _) | (_, Err(e)) => {
                    set_entity_schema_status(&entity_name, FetchStatus::Failed(e.clone()));
                    Err(e)
                }
            }
        }
    }).collect();

    let schema_results: Vec<Result<(String, Vec<crate::api::metadata::FieldMetadata>, Vec<crate::api::metadata::FieldMetadata>, String), String>> =
        join_all(schema_futures).await;

    // Collect successful schema results
    let mut schema_data: std::collections::HashMap<String, (Vec<crate::api::metadata::FieldMetadata>, Vec<crate::api::metadata::FieldMetadata>)> =
        std::collections::HashMap::new();
    let mut entity_set_names: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for result in schema_results {
        match result {
            Ok((entity_name, origin_fields, target_fields, set_name)) => {
                schema_data.insert(entity_name.clone(), (origin_fields, target_fields));
                entity_set_names.insert(entity_name, set_name);
            }
            Err(e) => {
                log::error!("Schema fetch failed: {}", e);
            }
        }
    }

    // Phase 2: Fetch all records in parallel (only for entities with successful schema fetch)
    set_analysis_phase("Fetching records...");

    let entity_set_names = Arc::new(entity_set_names);

    let record_futures: Vec<_> = selected_entities.iter()
        .filter(|name| entity_set_names.contains_key(*name))
        .map(|entity_name| {
            let origin_client = Arc::clone(&origin_client);
            let target_client = Arc::clone(&target_client);
            let entity_set_names = Arc::clone(&entity_set_names);
            let entity_name = entity_name.clone();

            async move {
                let entity_set_name = entity_set_names.get(&entity_name)
                    .expect("entity_set_name must exist (filtered above)");

                set_entity_records_status(&entity_name, FetchStatus::Fetching, None);

                let origin_records = fetch_all_records(&origin_client, &entity_name, entity_set_name, true).await;
                let target_ids = fetch_record_ids(&target_client, &entity_name, entity_set_name).await;

                match (&origin_records, &target_ids) {
                    (Ok(records), Ok(ids)) => {
                        let count = records.len();
                        set_entity_records_status(&entity_name, FetchStatus::Done, Some(count));
                        Ok((entity_name, records.clone(), ids.clone()))
                    }
                    (Err(e), _) => {
                        set_entity_records_status(&entity_name, FetchStatus::Failed(e.to_string()), None);
                        Err(format!("Failed to fetch records for {}: {}", entity_name, e))
                    }
                    (_, Err(e)) => {
                        set_entity_records_status(&entity_name, FetchStatus::Failed(e.to_string()), None);
                        Err(format!("Failed to fetch target IDs for {}: {}", entity_name, e))
                    }
                }
            }
        }).collect();

    let record_results: Vec<Result<(String, Vec<serde_json::Value>, Vec<String>), String>> =
        join_all(record_futures).await;

    // Collect successful record results
    let mut record_data: std::collections::HashMap<String, (Vec<serde_json::Value>, Vec<String>)> =
        std::collections::HashMap::new();

    for result in record_results {
        match result {
            Ok((entity_name, origin_records, target_ids)) => {
                record_data.insert(entity_name, (origin_records, target_ids));
            }
            Err(e) => {
                log::error!("Record fetch failed: {}", e);
            }
        }
    }

    // Phase 3: Fetch primary name attributes in parallel
    set_analysis_phase("Fetching entity metadata...");

    let metadata_futures: Vec<_> = selected_entities.iter().map(|entity_name| {
        let origin_client = Arc::clone(&origin_client);
        let entity_name = entity_name.clone();

        async move {
            let primary_name = fetch_primary_name_attribute(&origin_client, &entity_name).await;
            (entity_name, primary_name)
        }
    }).collect();

    let metadata_results: Vec<(String, Option<String>)> = join_all(metadata_futures).await;

    let primary_names: std::collections::HashMap<String, Option<String>> =
        metadata_results.into_iter().collect();

    // Phase 4: Build entity plans and dependency graph
    set_analysis_phase("Building dependency graph...");

    let mut entities_with_fields: Vec<(String, Option<String>, Vec<crate::api::metadata::FieldMetadata>)> = Vec::new();
    let mut entity_plans = Vec::new();
    let mut total_delete_count = 0usize;
    let mut total_insert_count = 0usize;
    let mut has_schema_changes = false;

    for entity_name in selected_entities {
        let Some((origin_fields, target_fields)) = schema_data.get(entity_name) else {
            log::warn!("Skipping {} - no schema data", entity_name);
            continue;
        };

        let (origin_records, target_record_ids) = record_data
            .get(entity_name)
            .cloned()
            .unwrap_or_else(|| (vec![], vec![]));

        let primary_name_attribute = primary_names.get(entity_name).cloned().flatten();

        // Store for dependency graph
        entities_with_fields.push((entity_name.clone(), None, origin_fields.clone()));

        // Compare schemas
        let schema_diff = compare_schemas(entity_name, origin_fields, target_fields, None);

        if schema_diff.has_changes() {
            has_schema_changes = true;
        }

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
                primary_name_attribute,
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
    set_analysis_phase("Computing sync order...");
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

    set_analysis_complete();

    Ok(SyncPlan {
        origin_env: origin_env.to_string(),
        target_env: target_env.to_string(),
        entity_plans,
        detected_junctions: vec![],
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
    entity_set_name: &str,
    active_only: bool,
) -> anyhow::Result<Vec<serde_json::Value>> {
    use crate::api::query::QueryBuilder;

    let mut all_records = Vec::new();

    let mut builder = QueryBuilder::new(entity_set_name).top(5000);
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

    log::info!("Fetched {} records from {} ({})", all_records.len(), entity_name, entity_set_name);
    Ok(all_records)
}

/// Fetch record IDs for an entity (for deletion)
async fn fetch_record_ids(
    client: &crate::api::DynamicsClient,
    entity_name: &str,
    entity_set_name: &str,
) -> anyhow::Result<Vec<String>> {
    use crate::api::query::QueryBuilder;

    let pk_field = format!("{}id", entity_name);
    let mut all_ids = Vec::new();

    let query = QueryBuilder::new(entity_set_name)
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

    log::info!("Fetched {} record IDs from {} ({})", all_ids.len(), entity_name, entity_set_name);
    Ok(all_ids)
}

/// Fetch the primary name attribute for an entity from metadata
async fn fetch_primary_name_attribute(
    client: &crate::api::DynamicsClient,
    entity_name: &str,
) -> Option<String> {
    use crate::api::query::{QueryBuilder, Filter};

    // Query EntityDefinitions for this entity's PrimaryNameAttribute
    let query = QueryBuilder::new("EntityDefinitions")
        .filter(Filter::eq("LogicalName", entity_name))
        .select(&["PrimaryNameAttribute"])
        .build();

    match client.execute_query(&query).await {
        Ok(result) => {
            if let Some(ref data) = result.data {
                if let Some(record) = data.value.first() {
                    if let Some(attr) = record.get("PrimaryNameAttribute").and_then(|v| v.as_str()) {
                        log::debug!("Primary name attribute for {}: {}", entity_name, attr);
                        return Some(attr.to_string());
                    }
                }
            }
            log::debug!("No primary name attribute found for {}", entity_name);
            None
        }
        Err(e) => {
            log::warn!("Failed to fetch primary name attribute for {}: {}", entity_name, e);
            None
        }
    }
}
