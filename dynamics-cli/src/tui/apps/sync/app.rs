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
                // Get origin record count for current entity
                let count = state
                    .sync_plan
                    .as_ref()
                    .and_then(|p| p.entity_plans.get(state.diff_review.selected_entity_idx))
                    .map(|e| e.data_preview.origin_records.len())
                    .unwrap_or(0);
                state.diff_review.data_list.handle_key(key, count, 15);
                Command::None
            }
            Msg::TargetDataListNavigate(key) => {
                // Get target record count for current entity
                let count = state
                    .sync_plan
                    .as_ref()
                    .and_then(|p| p.entity_plans.get(state.diff_review.selected_entity_idx))
                    .map(|e| e.data_preview.target_records.len())
                    .unwrap_or(0);
                state.diff_review.target_data_list.handle_key(key, count, 15);
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
                if !state.confirm.can_execute() {
                    return Command::None;
                }

                let Some(ref plan) = state.sync_plan else {
                    return Command::None;
                };

                let target_env = state.env_select.target_env.clone().unwrap_or_default();

                // Build queue items
                let queue_items = super::logic::build_sync_queue_items(plan, &target_env);

                // Store batch IDs for tracking
                let (delete_ids, schema_ids, insert_ids, junction_ids) = queue_items.item_ids();
                state.confirm.delete_batch_ids = delete_ids;
                state.confirm.schema_batch_ids = schema_ids;
                state.confirm.insert_batch_ids = insert_ids;
                state.confirm.junction_batch_ids = junction_ids;

                // Set initial state
                state.confirm.executing = true;
                state.confirm.total_operations = queue_items.total_operations();
                state.confirm.completed_operations = 0;

                // Determine initial phase
                if !state.confirm.delete_batch_ids.is_empty() {
                    state.confirm.phase = super::state::ExecutionPhase::Deleting;
                    state.confirm.total_batches = state.confirm.delete_batch_ids.len();
                } else if !state.confirm.schema_batch_ids.is_empty() {
                    state.confirm.phase = super::state::ExecutionPhase::AddingFields;
                    state.confirm.total_batches = state.confirm.schema_batch_ids.len();
                } else if !state.confirm.insert_batch_ids.is_empty() {
                    state.confirm.phase = super::state::ExecutionPhase::Inserting;
                    state.confirm.total_batches = state.confirm.insert_batch_ids.len();
                } else if !state.confirm.junction_batch_ids.is_empty() {
                    state.confirm.phase = super::state::ExecutionPhase::InsertingJunctions;
                    state.confirm.total_batches = state.confirm.junction_batch_ids.len();
                } else {
                    // Nothing to do
                    state.confirm.phase = super::state::ExecutionPhase::Complete;
                    state.confirm.executing = false;
                    return Command::None;
                }
                state.confirm.current_batch = 1;

                // Publish all items to the queue
                let all_items = queue_items.all_items();
                let queue_items_json = serde_json::to_value(&all_items).unwrap_or_default();

                Command::Publish {
                    topic: "queue:add_items".to_string(),
                    data: queue_items_json,
                }
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
            Msg::QueueItemCompleted { id, result, metadata: _ } => {
                // Find which phase this batch belongs to and update progress
                let confirm = &mut state.confirm;

                if result.success {
                    // Count completed operations
                    confirm.completed_operations += result.operation_results.len();

                    // Check which phase completed
                    if let Some(pos) = confirm.delete_batch_ids.iter().position(|bid| bid == &id) {
                        confirm.delete_batch_ids.remove(pos);
                        confirm.current_batch = confirm.total_batches - confirm.delete_batch_ids.len();

                        if confirm.delete_batch_ids.is_empty() {
                            // Move to next phase
                            if !confirm.schema_batch_ids.is_empty() {
                                confirm.phase = super::state::ExecutionPhase::AddingFields;
                                confirm.total_batches = confirm.schema_batch_ids.len();
                                confirm.current_batch = 1;
                            } else if !confirm.insert_batch_ids.is_empty() {
                                confirm.phase = super::state::ExecutionPhase::Inserting;
                                confirm.total_batches = confirm.insert_batch_ids.len();
                                confirm.current_batch = 1;
                            } else if !confirm.junction_batch_ids.is_empty() {
                                confirm.phase = super::state::ExecutionPhase::InsertingJunctions;
                                confirm.total_batches = confirm.junction_batch_ids.len();
                                confirm.current_batch = 1;
                            } else {
                                confirm.phase = super::state::ExecutionPhase::Complete;
                                confirm.executing = false;
                            }
                        }
                    } else if let Some(pos) = confirm.schema_batch_ids.iter().position(|bid| bid == &id) {
                        confirm.schema_batch_ids.remove(pos);
                        confirm.current_batch = confirm.total_batches - confirm.schema_batch_ids.len();

                        if confirm.schema_batch_ids.is_empty() {
                            // Move to next phase
                            if !confirm.insert_batch_ids.is_empty() {
                                confirm.phase = super::state::ExecutionPhase::Inserting;
                                confirm.total_batches = confirm.insert_batch_ids.len();
                                confirm.current_batch = 1;
                            } else if !confirm.junction_batch_ids.is_empty() {
                                confirm.phase = super::state::ExecutionPhase::InsertingJunctions;
                                confirm.total_batches = confirm.junction_batch_ids.len();
                                confirm.current_batch = 1;
                            } else {
                                confirm.phase = super::state::ExecutionPhase::Complete;
                                confirm.executing = false;
                            }
                        }
                    } else if let Some(pos) = confirm.insert_batch_ids.iter().position(|bid| bid == &id) {
                        confirm.insert_batch_ids.remove(pos);
                        confirm.current_batch = confirm.total_batches - confirm.insert_batch_ids.len();

                        if confirm.insert_batch_ids.is_empty() {
                            // Move to next phase
                            if !confirm.junction_batch_ids.is_empty() {
                                confirm.phase = super::state::ExecutionPhase::InsertingJunctions;
                                confirm.total_batches = confirm.junction_batch_ids.len();
                                confirm.current_batch = 1;
                            } else {
                                confirm.phase = super::state::ExecutionPhase::Complete;
                                confirm.executing = false;
                            }
                        }
                    } else if let Some(pos) = confirm.junction_batch_ids.iter().position(|bid| bid == &id) {
                        confirm.junction_batch_ids.remove(pos);
                        confirm.current_batch = confirm.total_batches - confirm.junction_batch_ids.len();

                        if confirm.junction_batch_ids.is_empty() {
                            confirm.phase = super::state::ExecutionPhase::Complete;
                            confirm.executing = false;
                        }
                    }
                } else {
                    // Batch failed
                    confirm.phase = super::state::ExecutionPhase::Failed;
                    confirm.executing = false;
                    confirm.failed = Some(super::state::FailedOperation {
                        phase: confirm.phase,
                        batch_id: id,
                        error: result.error.unwrap_or_else(|| "Unknown error".to_string()),
                    });
                }

                Command::None
            }
            Msg::ExecutionPhaseChanged(phase) => {
                state.confirm.phase = phase;
                Command::None
            }
            Msg::ExecutionComplete(result) => {
                state.confirm.executing = false;
                match result {
                    Ok(()) => {
                        state.confirm.phase = super::state::ExecutionPhase::Complete;
                    }
                    Err(e) => {
                        state.confirm.phase = super::state::ExecutionPhase::Failed;
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

                // Subscribe to queue completion events during execution
                if state.confirm.executing {
                    subs.push(Subscription::subscribe("queue:item_completed", |value| {
                        let id = value.get("id")?.as_str()?.to_string();
                        let result: crate::tui::apps::queue::models::QueueResult =
                            serde_json::from_value(value.get("result")?.clone()).ok()?;
                        let metadata: crate::tui::apps::queue::models::QueueMetadata =
                            serde_json::from_value(value.get("metadata")?.clone()).ok()?;
                        Some(Msg::QueueItemCompleted { id, result, metadata })
                    }));
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
                set_entity_schema_status, set_entity_records_status, set_entity_refs_status,
                set_entity_nn_status, FetchStatus};
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

            // Fetch fields, entity metadata, and raw attribute metadata in parallel
            let (origin_fields, target_fields, entity_metadata, origin_attrs_raw) = tokio::join!(
                origin_client.fetch_entity_fields_combined(&entity_name),
                target_client.fetch_entity_fields_combined(&entity_name),
                origin_client.fetch_entity_metadata_info(&entity_name),
                origin_client.fetch_entity_attributes_raw(&entity_name)
            );

            let origin_fields = origin_fields
                .map_err(|e| format!("Failed to fetch origin fields for {}: {}", entity_name, e));

            let target_fields = target_fields.unwrap_or_default();

            let entity_metadata = entity_metadata
                .map_err(|e| format!("Failed to fetch entity metadata for {}: {}", entity_name, e));

            // Raw attributes are optional - log warning if failed but don't fail the whole entity
            let origin_attrs_raw = match origin_attrs_raw {
                Ok(attrs) => Some(attrs),
                Err(e) => {
                    log::warn!("Failed to fetch raw attributes for {}: {} - schema sync will skip this entity", entity_name, e);
                    None
                }
            };

            match (origin_fields, entity_metadata) {
                (Ok(fields), Ok(metadata)) => {
                    set_entity_schema_status(&entity_name, FetchStatus::Done);
                    Ok((entity_name, fields, target_fields, metadata, origin_attrs_raw))
                }
                (Err(e), _) | (_, Err(e)) => {
                    set_entity_schema_status(&entity_name, FetchStatus::Failed(e.clone()));
                    Err(e)
                }
            }
        }
    }).collect();

    type SchemaResult = Result<(String, Vec<crate::api::metadata::FieldMetadata>, Vec<crate::api::metadata::FieldMetadata>, crate::api::EntityMetadataInfo, Option<std::collections::HashMap<String, serde_json::Value>>), String>;
    let schema_results: Vec<SchemaResult> = join_all(schema_futures).await;

    // Collect successful schema results
    // schema_data now includes raw attribute metadata for CreateAttribute operations
    type RawAttrsMap = Option<std::collections::HashMap<String, serde_json::Value>>;
    let mut schema_data: std::collections::HashMap<String, (Vec<crate::api::metadata::FieldMetadata>, Vec<crate::api::metadata::FieldMetadata>, RawAttrsMap)> =
        std::collections::HashMap::new();
    let mut entity_metadata_map: std::collections::HashMap<String, crate::api::EntityMetadataInfo> =
        std::collections::HashMap::new();

    for result in schema_results {
        match result {
            Ok((entity_name, origin_fields, target_fields, metadata, origin_attrs_raw)) => {
                schema_data.insert(entity_name.clone(), (origin_fields, target_fields, origin_attrs_raw));
                entity_metadata_map.insert(entity_name, metadata);
            }
            Err(e) => {
                log::error!("Schema fetch failed: {}", e);
            }
        }
    }

    // Phase 2: Fetch all records in parallel (only for entities with successful schema fetch)
    set_analysis_phase("Fetching records...");

    let entity_metadata_map = Arc::new(entity_metadata_map);

    let record_futures: Vec<_> = selected_entities.iter()
        .filter(|name| entity_metadata_map.contains_key(*name))
        .map(|entity_name| {
            let origin_client = Arc::clone(&origin_client);
            let target_client = Arc::clone(&target_client);
            let entity_metadata_map = Arc::clone(&entity_metadata_map);
            let entity_name = entity_name.clone();

            async move {
                let metadata = entity_metadata_map.get(&entity_name)
                    .expect("entity metadata must exist (filtered above)");

                set_entity_records_status(&entity_name, FetchStatus::Fetching, None);

                // Don't use active_only filter for junction/intersect entities (they don't have statecode)
                let active_only = !metadata.is_intersect;
                let origin_records = fetch_all_records(&origin_client, &entity_name, &metadata.entity_set_name, active_only).await;
                let target_records = fetch_target_records(
                    &target_client,
                    &entity_name,
                    &metadata.entity_set_name,
                    metadata.primary_name_attribute.as_deref(),
                ).await;

                match (&origin_records, &target_records) {
                    (Ok(records), Ok(targets)) => {
                        let count = records.len();
                        set_entity_records_status(&entity_name, FetchStatus::Done, Some(count));
                        Ok((entity_name, records.clone(), targets.clone()))
                    }
                    (Err(e), _) => {
                        set_entity_records_status(&entity_name, FetchStatus::Failed(e.to_string()), None);
                        Err(format!("Failed to fetch records for {}: {}", entity_name, e))
                    }
                    (_, Err(e)) => {
                        set_entity_records_status(&entity_name, FetchStatus::Failed(e.to_string()), None);
                        Err(format!("Failed to fetch target records for {}: {}", entity_name, e))
                    }
                }
            }
        }).collect();

    let record_results: Vec<Result<(String, Vec<serde_json::Value>, Vec<super::types::TargetRecord>), String>> =
        join_all(record_futures).await;

    // Collect successful record results
    let mut record_data: std::collections::HashMap<String, (Vec<serde_json::Value>, Vec<super::types::TargetRecord>)> =
        std::collections::HashMap::new();

    for result in record_results {
        match result {
            Ok((entity_name, origin_records, target_records)) => {
                record_data.insert(entity_name, (origin_records, target_records));
            }
            Err(e) => {
                log::error!("Record fetch failed: {}", e);
            }
        }
    }

    // Phase 3: Fetch incoming references in parallel
    set_analysis_phase("Fetching incoming references...");

    let refs_futures: Vec<_> = selected_entities.iter().map(|entity_name| {
        let origin_client = Arc::clone(&origin_client);
        let entity_name = entity_name.clone();

        async move {
            set_entity_refs_status(&entity_name, FetchStatus::Fetching, None);

            match origin_client.fetch_incoming_references(&entity_name).await {
                Ok(refs) => {
                    let count = refs.len();
                    set_entity_refs_status(&entity_name, FetchStatus::Done, Some(count));
                    Ok((entity_name, refs))
                }
                Err(e) => {
                    set_entity_refs_status(&entity_name, FetchStatus::Failed(e.to_string()), None);
                    Err(format!("Failed to fetch refs for {}: {}", entity_name, e))
                }
            }
        }
    }).collect();

    let refs_results: Vec<Result<(String, Vec<crate::api::IncomingReference>), String>> =
        join_all(refs_futures).await;

    // Collect successful refs results
    let mut refs_data: std::collections::HashMap<String, Vec<crate::api::IncomingReference>> =
        std::collections::HashMap::new();

    for result in refs_results {
        match result {
            Ok((entity_name, refs)) => {
                refs_data.insert(entity_name, refs);
            }
            Err(e) => {
                log::error!("Refs fetch failed: {}", e);
            }
        }
    }

    // Phase 4: Build entity plans and dependency graph
    set_analysis_phase("Building dependency graph...");

    let mut entities_with_fields: Vec<(String, Option<String>, Vec<crate::api::metadata::FieldMetadata>)> = Vec::new();
    let mut entity_plans = Vec::new();
    let mut total_delete_count = 0usize;
    let mut total_insert_count = 0usize;
    let mut has_schema_changes = false;

    for entity_name in selected_entities {
        let Some((origin_fields, target_fields, origin_attrs_raw)) = schema_data.get(entity_name) else {
            log::warn!("Skipping {} - no schema data", entity_name);
            continue;
        };

        let (origin_records, target_records) = record_data
            .get(entity_name)
            .cloned()
            .unwrap_or_else(|| (vec![], vec![]));

        // Get entity metadata (is_intersect, primary_name_attribute)
        let metadata = entity_metadata_map.get(entity_name);
        let is_intersect = metadata.map(|m| m.is_intersect).unwrap_or(false);
        let primary_name_attribute = metadata.and_then(|m| m.primary_name_attribute.clone());

        // Store for dependency graph
        entities_with_fields.push((entity_name.clone(), None, origin_fields.clone()));

        // Compare schemas - pass raw attribute metadata for CreateAttribute operations
        let schema_diff = compare_schemas(entity_name, origin_fields, target_fields, origin_attrs_raw.as_ref());

        if schema_diff.has_changes() {
            has_schema_changes = true;
        }

        let origin_count = origin_records.len();
        let target_count = target_records.len();

        total_delete_count += target_count;
        total_insert_count += origin_count;

        // Extract lookup info from fields
        let mut lookups: Vec<LookupInfo> = origin_fields
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

        // For junction/intersect entities, parse Uniqueidentifier fields to find related entities
        // These don't use Lookup fields - they store GUIDs directly in Uniqueidentifier fields
        if is_intersect {
            let pk_field = format!("{}id", entity_name);
            for field in origin_fields.iter() {
                if matches!(field.field_type, FieldType::UniqueIdentifier) {
                    // Skip the primary key field
                    if field.logical_name == pk_field {
                        continue;
                    }
                    // Extract entity name from field name (e.g., "nrq_fundid" -> "nrq_fund")
                    if let Some(target_entity) = field.logical_name.strip_suffix("id") {
                        // Check if this entity exists in our selection
                        let is_internal = selected_set.contains(target_entity);
                        lookups.push(LookupInfo {
                            field_name: field.logical_name.clone(),
                            target_entity: target_entity.to_string(),
                            is_internal,
                        });
                        log::debug!("Junction {} has reference to {} (internal: {})",
                            entity_name, target_entity, is_internal);
                    }
                }
            }
        }

        // Determine category based on is_intersect flag and lookup count
        let category = if is_intersect {
            DependencyCategory::Junction
        } else {
            DependencyCategory::Standalone // Will be updated by dependency graph
        };

        // Convert incoming references to IncomingReferenceInfo with is_internal check
        let incoming_refs: Vec<IncomingReferenceInfo> = refs_data
            .get(entity_name)
            .map(|refs| {
                refs.iter()
                    .map(|r| IncomingReferenceInfo {
                        referencing_entity: r.referencing_entity.clone(),
                        referencing_attribute: r.referencing_attribute.clone(),
                        is_internal: selected_set.contains(&r.referencing_entity),
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Get entity_set_name from metadata
        let entity_set_name = metadata
            .map(|m| m.entity_set_name.clone())
            .unwrap_or_else(|| format!("{}s", entity_name)); // Fallback pluralization

        // Build entity plan
        entity_plans.push(EntitySyncPlan {
            entity_info: SyncEntityInfo {
                logical_name: entity_name.clone(),
                display_name: None,
                entity_set_name,
                primary_name_attribute,
                category,
                lookups: lookups.clone(),
                incoming_references: incoming_refs,
                dependents: vec![],
                insert_priority: 0,
                delete_priority: 0,
                nn_relationship: None, // Will be populated in Phase 4 for junction entities
            },
            schema_diff,
            data_preview: EntityDataPreview {
                entity_name: entity_name.clone(),
                origin_count,
                target_count,
                origin_records,
                target_records,
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

    // Build dependency graph from fields
    let mut dep_graph = DependencyGraph::build(entities_with_fields);

    // Add junction entity dependencies manually (they use Uniqueidentifier fields, not Lookups)
    for plan in &entity_plans {
        if plan.entity_info.category == DependencyCategory::Junction {
            let entity_name = &plan.entity_info.logical_name;
            for lookup in &plan.entity_info.lookups {
                if lookup.is_internal {
                    // Add dependency: junction depends on target entity
                    dep_graph.dependencies
                        .entry(entity_name.clone())
                        .or_default()
                        .insert(lookup.target_entity.clone());

                    // Add reverse: target entity has this junction as dependent
                    dep_graph.dependents
                        .entry(lookup.target_entity.clone())
                        .or_default()
                        .insert(entity_name.clone());

                    log::debug!("Added junction dependency: {} -> {}", entity_name, lookup.target_entity);
                }
            }
        }
    }

    // Get sorted order and update priorities
    set_analysis_phase("Computing sync order...");
    if let Ok(sorted) = dep_graph.topological_sort() {
        for (insert_priority, name) in sorted.iter().enumerate() {
            if let Some(plan) = entity_plans.iter_mut().find(|p| &p.entity_info.logical_name == name) {
                plan.entity_info.insert_priority = insert_priority as u32;
                plan.entity_info.delete_priority = (sorted.len() - 1 - insert_priority) as u32;

                // Preserve Junction category from is_intersect flag, otherwise use graph categorization
                if plan.entity_info.category != DependencyCategory::Junction {
                    plan.entity_info.category = dep_graph.categorize(name);
                }

                // Get dependents
                if let Some(deps) = dep_graph.dependents.get(name) {
                    plan.entity_info.dependents = deps.iter().cloned().collect();
                }
            }
        }
    }

    // Sort by insert priority
    entity_plans.sort_by_key(|p| p.entity_info.insert_priority);

    // Phase 4: Fetch N:N relationship metadata for junction entities
    set_analysis_phase("Fetching N:N relationship metadata...");

    // Collect junction entities that need N:N metadata
    let junction_entities: Vec<String> = entity_plans
        .iter()
        .filter(|p| p.entity_info.category == DependencyCategory::Junction)
        .map(|p| p.entity_info.logical_name.clone())
        .collect();

    if !junction_entities.is_empty() {
        // For each junction entity, we need to find the N:N relationship metadata
        // by querying one of the parent entities' ManyToManyRelationships
        for junction_name in &junction_entities {
            set_entity_nn_status(junction_name, FetchStatus::Fetching);

            // Find the junction entity plan to get its lookups (which tell us the parent entities)
            let junction_plan = entity_plans
                .iter()
                .find(|p| &p.entity_info.logical_name == junction_name);

            if let Some(plan) = junction_plan {
                // Get the first internal lookup target as the parent entity to query
                let parent_entity = plan.entity_info.lookups
                    .iter()
                    .filter(|l| l.is_internal)
                    .map(|l| l.target_entity.clone())
                    .next();

                if let Some(parent) = parent_entity {
                    // Query ManyToManyRelationships from the parent entity
                    match origin_client.fetch_many_to_many_relationships(&parent).await {
                        Ok(relationships) => {
                            // Find the relationship that matches this junction entity
                            let matching_rel = relationships
                                .iter()
                                .find(|r| r.intersect_entity_name == *junction_name);

                            if let Some(rel) = matching_rel {
                                // Find the entity_set_names for parent and target
                                let parent_entity_set = entity_plans
                                    .iter()
                                    .find(|p| p.entity_info.logical_name == rel.entity1_logical_name)
                                    .map(|p| p.entity_info.entity_set_name.clone())
                                    .unwrap_or_else(|| format!("{}s", rel.entity1_logical_name));

                                let target_entity_set = entity_plans
                                    .iter()
                                    .find(|p| p.entity_info.logical_name == rel.entity2_logical_name)
                                    .map(|p| p.entity_info.entity_set_name.clone())
                                    .unwrap_or_else(|| format!("{}s", rel.entity2_logical_name));

                                let nn_info = NNRelationshipInfo {
                                    navigation_property: rel.entity1_navigation_property.clone(),
                                    parent_entity: rel.entity1_logical_name.clone(),
                                    parent_entity_set,
                                    parent_fk_field: rel.entity1_intersect_attribute.clone(),
                                    target_entity: rel.entity2_logical_name.clone(),
                                    target_entity_set,
                                    target_fk_field: rel.entity2_intersect_attribute.clone(),
                                };

                                // Update the entity plan with N:N info
                                if let Some(plan) = entity_plans
                                    .iter_mut()
                                    .find(|p| &p.entity_info.logical_name == junction_name)
                                {
                                    plan.entity_info.nn_relationship = Some(nn_info);
                                }

                                set_entity_nn_status(junction_name, FetchStatus::Done);
                                log::info!("Found N:N relationship for {}: nav_prop={}", junction_name, rel.entity1_navigation_property);
                            } else {
                                set_entity_nn_status(junction_name, FetchStatus::Failed(
                                    format!("No matching N:N relationship found for {}", junction_name)
                                ));
                                log::warn!("No N:N relationship found for junction entity {}", junction_name);
                            }
                        }
                        Err(e) => {
                            set_entity_nn_status(junction_name, FetchStatus::Failed(e.to_string()));
                            log::error!("Failed to fetch N:N relationships for {}: {}", parent, e);
                        }
                    }
                } else {
                    set_entity_nn_status(junction_name, FetchStatus::Failed(
                        "No internal lookup found".to_string()
                    ));
                    log::warn!("Junction entity {} has no internal lookups", junction_name);
                }
            }
        }
    }

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

/// Fetch target records (ID + name) for an entity (for deletion preview)
async fn fetch_target_records(
    client: &crate::api::DynamicsClient,
    entity_name: &str,
    entity_set_name: &str,
    primary_name_attribute: Option<&str>,
) -> anyhow::Result<Vec<super::types::TargetRecord>> {
    use crate::api::query::QueryBuilder;

    let pk_field = format!("{}id", entity_name);
    let mut all_records = Vec::new();

    // Select ID and optionally the name field
    let select_fields: Vec<&str> = if let Some(name_attr) = primary_name_attribute {
        vec![&pk_field, name_attr]
    } else {
        vec![&pk_field]
    };

    let query = QueryBuilder::new(entity_set_name)
        .select(&select_fields)
        .top(5000)
        .build();

    let mut result = client.execute_query(&query).await?;

    let extract_record = |record: &serde_json::Value| -> Option<super::types::TargetRecord> {
        let id = record.get(&pk_field).and_then(|v| v.as_str())?.to_string();
        let name = primary_name_attribute
            .and_then(|attr| record.get(attr))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Some(super::types::TargetRecord { id, name })
    };

    if let Some(ref data) = result.data {
        for record in &data.value {
            if let Some(tr) = extract_record(record) {
                all_records.push(tr);
            }
        }
    }

    while result.has_more() {
        if let Some(next) = result.next_page(client).await? {
            if let Some(ref data) = next.data {
                for record in &data.value {
                    if let Some(tr) = extract_record(record) {
                        all_records.push(tr);
                    }
                }
            }
            result = next;
        } else {
            break;
        }
    }

    log::info!("Fetched {} target records from {} ({})", all_records.len(), entity_name, entity_set_name);
    Ok(all_records)
}
