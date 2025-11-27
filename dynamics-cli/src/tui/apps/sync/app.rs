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
                        }
                    }
                    TextInputEvent::Submit => {}
                }
                Command::None
            }
            Msg::ClearFilter => {
                state.entity_select.filter_text.clear();
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
            Msg::DiffSwitchTab => {
                state.diff_review.active_tab = state.diff_review.active_tab.next();
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
                    KeyCode::Tab,
                    "Switch tab",
                    Msg::DiffSwitchTab,
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
                // Load entities for the selected origin environment
                // In a real implementation, this would fetch from the API
                state.entity_select.available_entities = crate::tui::Resource::Loading;
            }
            Command::None
        }
        SyncStep::EntitySelect => {
            if state.entity_select.can_proceed() {
                state.step = SyncStep::Analysis;
                // Start analysis
                state.analysis = Default::default();
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
