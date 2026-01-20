use crate::tui::element::LayoutConstraint::*;
use crate::tui::widgets::list::{ListItem, ListState};
use crate::tui::{App, AppId, Command, Element, FocusId, LayeredView, Subscription, Theme};
use crate::{col, row, spacer, use_constraints};
use crossterm::event::KeyCode;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use std::collections::HashMap;

use super::diff::diff_associations;
use super::models::{DeadlineMode, InspectionParams, TransformedDeadline};
use super::operation_builder::{
    build_associate_operations, build_create_junction_operations, build_delete_junction_operations,
    build_disassociate_operations,
};
use crate::api::operations::Operations;
use crate::tui::apps::queue::{QueueItem, QueueMetadata};

pub struct DeadlinesInspectionApp;

/// Wrapper for TransformedDeadline to implement ListItem trait
#[derive(Clone)]
struct RecordListItem {
    record: TransformedDeadline,
    entity_type: String, // For determining field prefix
}

impl ListItem for RecordListItem {
    type Msg = Msg;

    fn to_element(
        &self,
        is_selected: bool,
        _is_multi_selected: bool,
        _is_hovered: bool,
    ) -> Element<Msg> {
        let theme = &crate::global_runtime_config().theme;
        let (fg_color, bg_style) = if is_selected {
            (
                theme.accent_primary,
                Some(Style::default().bg(theme.bg_surface)),
            )
        } else {
            (theme.text_primary, None)
        };

        // Extract name from direct fields
        let name_field = if self.entity_type == "cgk_deadline" {
            "cgk_deadlinename"
        } else {
            "nrq_deadlinename"
        };
        let name = self
            .record
            .direct_fields
            .get(name_field)
            .map(|s| s.as_str())
            .unwrap_or("<No Name>");

        // Truncate name if too long
        let display_name = if name.len() > 30 {
            format!("{}...", &name[..27])
        } else {
            name.to_string()
        };

        // Mode indicator with color
        let (mode_label, mode_color) = match &self.record.mode {
            DeadlineMode::Create => ("NEW", theme.accent_success),
            DeadlineMode::Update => ("UPD", theme.accent_secondary),
            DeadlineMode::Unchanged => ("---", theme.text_tertiary),
            DeadlineMode::Error(_) => ("ERR", theme.accent_error),
        };

        // Warning indicator
        let warning_indicator = if self.record.has_warnings() {
            Span::styled("⚠ ", Style::default().fg(theme.accent_warning))
        } else {
            Span::styled("  ", Style::default())
        };

        let mut builder = Element::styled_text(Line::from(vec![
            Span::styled(
                format!("[{}] ", mode_label),
                Style::default().fg(mode_color),
            ),
            warning_indicator,
            Span::styled(
                format!("Row {}: ", self.record.source_row),
                Style::default().fg(theme.text_tertiary),
            ),
            Span::styled(display_name, Style::default().fg(fg_color)),
        ]));

        if let Some(bg) = bg_style {
            builder = builder.background(bg);
        }

        builder.build()
    }
}

/// Filter for which record modes to display
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeFilter {
    All,
    Create,
    Update,
    Unchanged,
    Errors,
    Actionable, // Create + Update (records that will be processed)
}

impl ModeFilter {
    fn next(self) -> Self {
        match self {
            ModeFilter::All => ModeFilter::Actionable,
            ModeFilter::Actionable => ModeFilter::Create,
            ModeFilter::Create => ModeFilter::Update,
            ModeFilter::Update => ModeFilter::Unchanged,
            ModeFilter::Unchanged => ModeFilter::Errors,
            ModeFilter::Errors => ModeFilter::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            ModeFilter::All => "All",
            ModeFilter::Create => "New Only",
            ModeFilter::Update => "Updates Only",
            ModeFilter::Unchanged => "Unchanged Only",
            ModeFilter::Errors => "Errors Only",
            ModeFilter::Actionable => "Actionable",
        }
    }

    fn matches(self, record: &TransformedDeadline) -> bool {
        match self {
            ModeFilter::All => true,
            ModeFilter::Create => record.is_create(),
            ModeFilter::Update => record.is_update(),
            ModeFilter::Unchanged => record.is_unchanged(),
            ModeFilter::Errors => record.is_error(),
            ModeFilter::Actionable => record.is_create() || record.is_update(),
        }
    }
}

#[derive(Clone)]
pub struct State {
    entity_type: String,
    transformed_records: Vec<TransformedDeadline>,
    list_state: ListState,
    selected_record_idx: usize,
    /// Current mode filter
    mode_filter: ModeFilter,
    /// Filtered record indices (indices into transformed_records)
    filtered_indices: Vec<usize>,
    /// Track queue items created from this inspection session (queue_item_id -> Vec<TransformedDeadline>)
    queued_items: HashMap<String, Vec<TransformedDeadline>>,
    /// Total number of deadlines queued in current batch
    total_deadlines_queued: usize,
    /// Accumulated associations from completed deadline creations (deadline_guid -> operations)
    pending_associations: HashMap<String, Vec<crate::api::operations::Operation>>,
}

impl State {
    fn new(entity_type: String, transformed_records: Vec<TransformedDeadline>) -> Self {
        let mode_filter = ModeFilter::All;

        // Build initial filtered indices
        let filtered_indices: Vec<usize> = transformed_records
            .iter()
            .enumerate()
            .filter(|(_, r)| mode_filter.matches(r))
            .map(|(i, _)| i)
            .collect();

        let mut list_state = ListState::default();
        // Auto-select first record if any exist
        if !filtered_indices.is_empty() {
            list_state.select_and_scroll(Some(0), filtered_indices.len());
        }

        Self {
            entity_type,
            transformed_records,
            list_state,
            selected_record_idx: 0,
            mode_filter,
            filtered_indices,
            queued_items: HashMap::new(),
            total_deadlines_queued: 0,
            pending_associations: HashMap::new(),
        }
    }

    /// Rebuild filtered indices based on current filter
    fn apply_filter(&mut self) {
        self.filtered_indices = self
            .transformed_records
            .iter()
            .enumerate()
            .filter(|(_, r)| self.mode_filter.matches(r))
            .map(|(i, _)| i)
            .collect();

        // Reset selection to first item or none
        if self.filtered_indices.is_empty() {
            self.selected_record_idx = 0;
            self.list_state.select_and_scroll(None, 0);
        } else {
            self.selected_record_idx = 0;
            self.list_state
                .select_and_scroll(Some(0), self.filtered_indices.len());
        }
    }

    /// Get the actual record index from the filtered index
    fn get_record_index(&self, filtered_idx: usize) -> Option<usize> {
        self.filtered_indices.get(filtered_idx).copied()
    }

    /// Get the currently selected record
    fn selected_record(&self) -> Option<&TransformedDeadline> {
        self.get_record_index(self.selected_record_idx)
            .and_then(|idx| self.transformed_records.get(idx))
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new(String::new(), Vec::new())
    }
}

#[derive(Clone)]
pub enum Msg {
    SelectRecord(usize),
    ListNavigate(KeyCode),
    SetViewportHeight(usize),
    Back,
    AddToQueueAndView,
    CycleFilter,
    EnvironmentLoaded(Result<String, String>),
    QueueItemCompleted(
        String,
        crate::tui::apps::queue::models::QueueResult,
        crate::tui::apps::queue::models::QueueMetadata,
    ),
}

impl crate::tui::AppState for State {}

impl App for DeadlinesInspectionApp {
    type State = State;
    type Msg = Msg;
    type InitParams = InspectionParams;

    fn init(params: Self::InitParams) -> (State, Command<Msg>) {
        let state = State::new(params.entity_type, params.transformed_records);

        (state, Command::set_focus(FocusId::new("queue-button")))
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::SelectRecord(idx) => {
                if idx < state.filtered_indices.len() {
                    state.selected_record_idx = idx;
                    state
                        .list_state
                        .select_and_scroll(Some(idx), state.filtered_indices.len());
                }
                Command::None
            }
            Msg::ListNavigate(key) => {
                // ListState will use its stored viewport_height from on_render, fallback to 20
                state
                    .list_state
                    .handle_key(key, state.filtered_indices.len(), 20);

                // Sync selected_record_idx with list_state
                if let Some(selected) = state.list_state.selected() {
                    if selected != state.selected_record_idx
                        && selected < state.filtered_indices.len()
                    {
                        state.selected_record_idx = selected;
                    }
                }

                Command::None
            }
            Msg::SetViewportHeight(height) => {
                let item_count = state.filtered_indices.len();
                state.list_state.set_viewport_height(height);
                state.list_state.update_scroll(height, item_count);
                Command::None
            }
            Msg::CycleFilter => {
                state.mode_filter = state.mode_filter.next();
                state.apply_filter();
                Command::None
            }
            Msg::Back => Command::navigate_to(AppId::DeadlinesMapping),
            Msg::AddToQueueAndView => {
                // Get current environment first
                Command::perform(
                    async {
                        let manager = crate::client_manager();
                        manager
                            .get_current_environment_name()
                            .await
                            .map_err(|e| e.to_string())?
                            .ok_or_else(|| "No environment selected".to_string())
                    },
                    Msg::EnvironmentLoaded,
                )
            }
            Msg::EnvironmentLoaded(Ok(environment_name)) => {
                // Collect valid records by mode (skip Unchanged, Error, and records with warnings)
                let create_records: Vec<&TransformedDeadline> = state
                    .transformed_records
                    .iter()
                    .filter(|r| r.is_create() && !r.has_warnings())
                    .collect();

                let update_records: Vec<&TransformedDeadline> = state
                    .transformed_records
                    .iter()
                    .filter(|r| r.is_update() && !r.has_warnings())
                    .collect();

                let total_actionable = create_records.len() + update_records.len();

                if total_actionable == 0 {
                    log::warn!("No actionable records to queue (all unchanged/error/warnings)");
                    return Command::None;
                }

                log::info!(
                    "Queuing {} creates and {} updates",
                    create_records.len(),
                    update_records.len()
                );

                // Track totals for association batching later
                state.total_deadlines_queued = create_records.len();

                let mut all_queue_items = Vec::new();

                // Batch deadline creates into groups of 10
                if !create_records.is_empty() {
                    let create_items = batch_deadline_creates(
                        &create_records,
                        &state.entity_type,
                        &environment_name,
                        &mut state.queued_items,
                        10,
                    );
                    all_queue_items.extend(create_items);
                }

                // Batch deadline updates into groups of 10
                // Updates include: PATCH operation + association changes
                if !update_records.is_empty() {
                    let update_items = batch_deadline_updates(
                        &update_records,
                        &state.entity_type,
                        &environment_name,
                        10,
                    );
                    all_queue_items.extend(update_items);
                }

                // Serialize queue items to JSON for pub/sub
                let queue_items_json = match serde_json::to_value(&all_queue_items) {
                    Ok(json) => json,
                    Err(e) => {
                        log::error!("Failed to serialize queue items: {}", e);
                        return Command::None;
                    }
                };

                // Publish to queue and navigate
                Command::Batch(vec![
                    Command::Publish {
                        topic: "queue:add_items".to_string(),
                        data: queue_items_json,
                    },
                    Command::navigate_to(AppId::OperationQueue),
                ])
            }
            Msg::EnvironmentLoaded(Err(err)) => {
                log::error!("Failed to load environment: {}", err);
                Command::None
            }

            Msg::QueueItemCompleted(item_id, result, metadata) => {
                // Check if this was a deadline create batch from our session
                if let Some(records) = state.queued_items.get(&item_id) {
                    // Only process successful creates
                    if !result.success || result.operation_results.is_empty() {
                        return Command::None;
                    }

                    // Match each operation result to its corresponding record
                    let num_results = result.operation_results.len();
                    let num_records = records.len();

                    if num_results != num_records {
                        log::error!(
                            "Mismatch: {} operation results but {} records in batch",
                            num_results,
                            num_records
                        );
                        return Command::None;
                    }

                    log::info!("Processing {} deadline creates from batch", num_results);

                    // Extract GUIDs and build associations for each deadline in the batch
                    for (idx, op_result) in result.operation_results.iter().enumerate() {
                        let record = &records[idx];

                        let created_guid = match extract_entity_guid_from_result(op_result) {
                            Some(guid) => guid,
                            None => {
                                log::error!(
                                    "Failed to extract entity GUID from operation result {}",
                                    idx
                                );
                                continue;
                            }
                        };

                        log::debug!("Deadline #{} created with GUID: {}", idx + 1, created_guid);

                        // Generate AssociateRef operations for N:N relationships
                        let mut all_ops = build_association_operations(
                            &created_guid,
                            &state.entity_type,
                            &record.checkbox_relationships,
                        );

                        // Generate Create operations for custom junction entities (e.g., nrq_deadlinesupport)
                        let custom_junction_ops = build_custom_junction_operations(
                            &created_guid,
                            &state.entity_type,
                            &record.custom_junction_records,
                        );
                        all_ops.extend(custom_junction_ops);

                        if !all_ops.is_empty() {
                            // Accumulate associations for later batching
                            state
                                .pending_associations
                                .insert(created_guid.clone(), all_ops);
                        }
                    }

                    // Check if all deadlines have been processed
                    if state.pending_associations.len() >= state.total_deadlines_queued {
                        log::info!(
                            "All {} deadlines created, batching associations",
                            state.total_deadlines_queued
                        );

                        // Batch associations in groups of 10 max, never splitting a deadline's associations
                        let batched_queue_items = batch_associations(
                            &state.pending_associations,
                            &state.entity_type,
                            &metadata.environment_name,
                            10,
                        );

                        if batched_queue_items.is_empty() {
                            log::info!("No associations to create");
                            state.pending_associations.clear();
                            return Command::None;
                        }

                        log::info!(
                            "Created {} association queue items",
                            batched_queue_items.len()
                        );

                        // Serialize and queue all batches
                        let queue_items_json = match serde_json::to_value(&batched_queue_items) {
                            Ok(json) => json,
                            Err(e) => {
                                log::error!("Failed to serialize association queue items: {}", e);
                                return Command::None;
                            }
                        };

                        // Clear pending associations
                        state.pending_associations.clear();

                        return Command::Publish {
                            topic: "queue:add_items".to_string(),
                            data: queue_items_json,
                        };
                    }

                    Command::None
                } else {
                    Command::None
                }
            }
        }
    }

    fn view(state: &mut State) -> LayeredView<Msg> {
        use_constraints!();
        let theme = &crate::global_runtime_config().theme;

        // Convert filtered records to list items
        let list_items: Vec<RecordListItem> = state
            .filtered_indices
            .iter()
            .filter_map(|&idx| state.transformed_records.get(idx))
            .map(|r| RecordListItem {
                record: r.clone(),
                entity_type: state.entity_type.clone(),
            })
            .collect();

        // Left panel: record list
        let record_list = Element::list("record-list", &list_items, &state.list_state, theme)
            .on_select(Msg::SelectRecord)
            .on_activate(Msg::SelectRecord)
            .on_navigate(Msg::ListNavigate)
            .on_render(Msg::SetViewportHeight)
            .build();

        // Build list title with filter info
        let filter_label = state.mode_filter.label();
        let list_title = format!(
            "Records [{filter_label}] ({}/{})",
            state.filtered_indices.len(),
            state.transformed_records.len()
        );

        let left_panel = Element::panel(record_list).title(&list_title).build();

        // Right panel: details for selected record
        let detail_content = if let Some(record) = state.selected_record() {
            build_detail_panel(record, &state.entity_type)
        } else {
            col![
                Element::styled_text(Line::from(vec![Span::styled(
                    "No record selected",
                    Style::default().fg(theme.text_tertiary)
                )]))
                .build()
            ]
        };

        let detail_title = if let Some(record) = state.selected_record() {
            let name_field = if state.entity_type == "cgk_deadline" {
                "cgk_deadlinename"
            } else {
                "nrq_deadlinename"
            };
            let name = record
                .direct_fields
                .get(name_field)
                .map(|s| s.as_str())
                .unwrap_or("<No Name>");
            format!("Row {} - {}", record.source_row, name)
        } else {
            "Record Details".to_string()
        };

        let right_panel = Element::panel(detail_content).title(&detail_title).build();

        // Main layout - two panels side by side with buttons at bottom
        let main_content = col![
            row![
                left_panel => Length(45),
                right_panel => Fill(1),
            ] => Fill(1),
            spacer!() => Length(1),
            row![
                Element::button("back-button", "Back")
                    .on_press(Msg::Back)
                    .build(),
                Element::button("filter-button", &format!("Filter: {} [f]", filter_label))
                    .on_press(Msg::CycleFilter)
                    .build(),
                spacer!(),
                Element::button("queue-button", "Add to Queue & View")
                    .on_press(Msg::AddToQueueAndView)
                    .build(),
            ] => Length(3),
        ];

        let outer_panel = Element::panel(main_content)
            .title("Deadlines - Inspection")
            .build();

        LayeredView::new(outer_panel)
    }

    fn subscriptions(_state: &State) -> Vec<Subscription<Msg>> {
        vec![
            Subscription::subscribe("queue:item_completed", |value| {
                // Extract id, result, metadata from the completion event
                let id = value.get("id")?.as_str()?.to_string();
                let result: crate::tui::apps::queue::models::QueueResult =
                    serde_json::from_value(value.get("result")?.clone()).ok()?;
                let metadata: crate::tui::apps::queue::models::QueueMetadata =
                    serde_json::from_value(value.get("metadata")?.clone()).ok()?;
                Some(Msg::QueueItemCompleted(id, result, metadata))
            }),
            // Keyboard shortcut for filter cycling
            Subscription::keyboard(KeyCode::Char('f'), "Cycle filter", Msg::CycleFilter),
        ]
    }

    fn title() -> &'static str {
        "Deadlines - Inspection"
    }

    fn status(state: &State) -> Option<Line<'static>> {
        let theme = &crate::global_runtime_config().theme;

        // Count records by mode
        let create_count = state
            .transformed_records
            .iter()
            .filter(|r| r.is_create())
            .count();
        let update_count = state
            .transformed_records
            .iter()
            .filter(|r| r.is_update())
            .count();
        let unchanged_count = state
            .transformed_records
            .iter()
            .filter(|r| r.is_unchanged())
            .count();
        let error_count = state
            .transformed_records
            .iter()
            .filter(|r| r.is_error())
            .count();
        let records_with_warnings = state
            .transformed_records
            .iter()
            .filter(|r| r.has_warnings())
            .count();

        Some(Line::from(vec![
            Span::styled("New: ", Style::default().fg(theme.text_tertiary)),
            Span::styled(
                create_count.to_string(),
                Style::default().fg(theme.accent_success),
            ),
            Span::styled(" | Update: ", Style::default().fg(theme.text_tertiary)),
            Span::styled(
                update_count.to_string(),
                Style::default().fg(theme.accent_secondary),
            ),
            Span::styled(" | Unchanged: ", Style::default().fg(theme.text_tertiary)),
            Span::styled(
                unchanged_count.to_string(),
                Style::default().fg(theme.text_tertiary),
            ),
            Span::styled(" | Errors: ", Style::default().fg(theme.text_tertiary)),
            Span::styled(
                error_count.to_string(),
                Style::default().fg(if error_count > 0 {
                    theme.accent_error
                } else {
                    theme.text_tertiary
                }),
            ),
            Span::styled(" | Warnings: ", Style::default().fg(theme.text_tertiary)),
            Span::styled(
                records_with_warnings.to_string(),
                Style::default().fg(if records_with_warnings > 0 {
                    theme.accent_warning
                } else {
                    theme.accent_success
                }),
            ),
        ]))
    }
}

/// Build the detail panel for a selected record
fn build_detail_panel(record: &TransformedDeadline, entity_type: &str) -> Element<Msg> {
    let theme = &crate::global_runtime_config().theme;
    use crate::tui::element::ColumnBuilder;

    let mut builder = ColumnBuilder::new();

    // Mode section at top (prominent display)
    let (mode_label, mode_color, mode_icon) = match &record.mode {
        DeadlineMode::Create => ("CREATE - New Record", theme.accent_success, "+"),
        DeadlineMode::Update => ("UPDATE - Existing Record", theme.accent_secondary, "~"),
        DeadlineMode::Unchanged => ("UNCHANGED - No Changes", theme.text_tertiary, "="),
        DeadlineMode::Error(msg) => ("ERROR", theme.accent_error, "!"),
    };

    builder = builder.add(
        Element::styled_text(Line::from(vec![
            Span::styled(
                format!("{} ", mode_icon),
                Style::default().fg(mode_color).bold(),
            ),
            Span::styled(mode_label, Style::default().fg(mode_color).bold()),
        ]))
        .build(),
        Length(1),
    );

    // Show error message if in Error mode
    if let DeadlineMode::Error(msg) = &record.mode {
        builder = builder.add(
            Element::styled_text(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(msg.clone(), Style::default().fg(theme.accent_error)),
            ]))
            .build(),
            Length(1),
        );
    }

    // Show existing GUID for Update records
    if let Some(ref guid) = record.existing_guid {
        builder = builder.add(
            Element::styled_text(Line::from(vec![
                Span::styled("  Existing ID: ", Style::default().fg(theme.text_tertiary)),
                Span::styled(guid.clone(), Style::default().fg(theme.accent_muted)),
            ]))
            .build(),
            Length(1),
        );
    }

    builder = builder.add(spacer!(), Length(1));

    // For Update records, show what will change
    if record.is_update() {
        // Show field changes
        if let Some(ref existing_fields) = record.existing_fields {
            let mut field_changes: Vec<(String, String, String)> = Vec::new(); // (field_name, old_value, new_value)

            // Check direct fields for changes
            for (field_name, new_value) in &record.direct_fields {
                let old_value = existing_fields
                    .get(field_name)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if new_value != &old_value {
                    field_changes.push((field_name.clone(), old_value, new_value.clone()));
                }
            }

            // Check picklist fields
            for (field_name, new_value) in &record.picklist_fields {
                let old_value = existing_fields
                    .get(field_name)
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32);

                if old_value != Some(*new_value) {
                    field_changes.push((
                        field_name.clone(),
                        old_value
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "null".to_string()),
                        new_value.to_string(),
                    ));
                }
            }

            // Check boolean fields
            for (field_name, new_value) in &record.boolean_fields {
                let old_value = existing_fields.get(field_name).and_then(|v| v.as_bool());

                if old_value != Some(*new_value) {
                    field_changes.push((
                        field_name.clone(),
                        old_value
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "null".to_string()),
                        new_value.to_string(),
                    ));
                }
            }

            if !field_changes.is_empty() {
                builder = builder.add(
                    Element::styled_text(Line::from(vec![Span::styled(
                        "📝 Field Changes",
                        Style::default().fg(theme.accent_info).bold(),
                    )]))
                    .build(),
                    Length(1),
                );

                for (field_name, old_val, new_val) in &field_changes {
                    let old_display = if old_val.is_empty() {
                        "(empty)"
                    } else {
                        old_val.as_str()
                    };
                    let new_display = if new_val.is_empty() {
                        "(empty)"
                    } else {
                        new_val.as_str()
                    };

                    builder = builder.add(
                        Element::styled_text(Line::from(vec![
                            Span::styled(
                                format!("  {}: ", field_name),
                                Style::default().fg(theme.text_tertiary),
                            ),
                            Span::styled(
                                old_display.to_string(),
                                Style::default().fg(theme.accent_error),
                            ),
                            Span::styled(" → ", Style::default().fg(theme.text_tertiary)),
                            Span::styled(
                                new_display.to_string(),
                                Style::default().fg(theme.accent_success),
                            ),
                        ]))
                        .build(),
                        Length(1),
                    );
                }
                builder = builder.add(spacer!(), Length(1));
            }
        }

        if let Some(ref existing_assoc) = record.existing_associations {
            let association_diff = diff_associations(record, existing_assoc, entity_type);

            if association_diff.has_changes() {
                builder = builder.add(
                    Element::styled_text(Line::from(vec![Span::styled(
                        "📋 Association Changes",
                        Style::default().fg(theme.accent_warning).bold(),
                    )]))
                    .build(),
                    Length(1),
                );

                // Helper to format names for display (sorted for deterministic order)
                let format_names = |ids: &std::collections::HashSet<String>,
                                    name_map: &std::collections::HashMap<String, String>|
                 -> String {
                    let mut names: Vec<_> = ids
                        .iter()
                        .map(|id| {
                            name_map
                                .get(id)
                                .cloned()
                                .unwrap_or_else(|| id[..8.min(id.len())].to_string())
                        })
                        .collect();
                    names.sort();
                    names.join(", ")
                };

                // Support changes
                if !association_diff.support_to_add.is_empty()
                    || !association_diff.support_to_remove.is_empty()
                {
                    builder = builder.add(
                        Element::styled_text(Line::from(vec![Span::styled(
                            "  Support:",
                            Style::default().fg(theme.text_secondary).bold(),
                        )]))
                        .build(),
                        Length(1),
                    );
                    if !association_diff.support_to_add.is_empty() {
                        // For NRQ, get names from custom_junction_records (sorted)
                        let mut add_names_vec: Vec<_> = association_diff
                            .support_to_add
                            .iter()
                            .map(|id| {
                                record
                                    .custom_junction_records
                                    .iter()
                                    .find(|r| &r.related_id == id)
                                    .map(|r| r.related_name.clone())
                                    .unwrap_or_else(|| id[..8.min(id.len())].to_string())
                            })
                            .collect();
                        add_names_vec.sort();
                        let add_names = add_names_vec.join(", ");
                        builder = builder.add(
                            Element::styled_text(Line::from(vec![
                                Span::styled("    + ", Style::default().fg(theme.accent_success)),
                                Span::styled(add_names, Style::default().fg(theme.accent_success)),
                            ]))
                            .build(),
                            Length(1),
                        );
                    }
                    if !association_diff.support_to_remove.is_empty() {
                        let remove_names = format_names(
                            &association_diff.support_to_remove,
                            &existing_assoc.support_names,
                        );
                        builder = builder.add(
                            Element::styled_text(Line::from(vec![
                                Span::styled("    - ", Style::default().fg(theme.accent_error)),
                                Span::styled(remove_names, Style::default().fg(theme.accent_error)),
                            ]))
                            .build(),
                            Length(1),
                        );
                    }
                }

                // Category changes
                if !association_diff.category_to_add.is_empty()
                    || !association_diff.category_to_remove.is_empty()
                {
                    builder = builder.add(
                        Element::styled_text(Line::from(vec![Span::styled(
                            "  Category:",
                            Style::default().fg(theme.text_secondary).bold(),
                        )]))
                        .build(),
                        Length(1),
                    );
                    if !association_diff.category_to_add.is_empty() {
                        let add_names =
                            format!("{} item(s)", association_diff.category_to_add.len());
                        builder = builder.add(
                            Element::styled_text(Line::from(vec![
                                Span::styled("    + ", Style::default().fg(theme.accent_success)),
                                Span::styled(add_names, Style::default().fg(theme.accent_success)),
                            ]))
                            .build(),
                            Length(1),
                        );
                    }
                    if !association_diff.category_to_remove.is_empty() {
                        let remove_names = format_names(
                            &association_diff.category_to_remove,
                            &existing_assoc.category_names,
                        );
                        builder = builder.add(
                            Element::styled_text(Line::from(vec![
                                Span::styled("    - ", Style::default().fg(theme.accent_error)),
                                Span::styled(remove_names, Style::default().fg(theme.accent_error)),
                            ]))
                            .build(),
                            Length(1),
                        );
                    }
                }

                // Length changes (CGK only)
                if !association_diff.length_to_add.is_empty()
                    || !association_diff.length_to_remove.is_empty()
                {
                    builder = builder.add(
                        Element::styled_text(Line::from(vec![Span::styled(
                            "  Length:",
                            Style::default().fg(theme.text_secondary).bold(),
                        )]))
                        .build(),
                        Length(1),
                    );
                    if !association_diff.length_to_add.is_empty() {
                        let add_names = format!("{} item(s)", association_diff.length_to_add.len());
                        builder = builder.add(
                            Element::styled_text(Line::from(vec![
                                Span::styled("    + ", Style::default().fg(theme.accent_success)),
                                Span::styled(add_names, Style::default().fg(theme.accent_success)),
                            ]))
                            .build(),
                            Length(1),
                        );
                    }
                    if !association_diff.length_to_remove.is_empty() {
                        let remove_names = format_names(
                            &association_diff.length_to_remove,
                            &existing_assoc.length_names,
                        );
                        builder = builder.add(
                            Element::styled_text(Line::from(vec![
                                Span::styled("    - ", Style::default().fg(theme.accent_error)),
                                Span::styled(remove_names, Style::default().fg(theme.accent_error)),
                            ]))
                            .build(),
                            Length(1),
                        );
                    }
                }

                // Flemishshare changes
                if !association_diff.flemishshare_to_add.is_empty()
                    || !association_diff.flemishshare_to_remove.is_empty()
                {
                    builder = builder.add(
                        Element::styled_text(Line::from(vec![Span::styled(
                            "  Flemish Share:",
                            Style::default().fg(theme.text_secondary).bold(),
                        )]))
                        .build(),
                        Length(1),
                    );
                    if !association_diff.flemishshare_to_add.is_empty() {
                        let add_names =
                            format!("{} item(s)", association_diff.flemishshare_to_add.len());
                        builder = builder.add(
                            Element::styled_text(Line::from(vec![
                                Span::styled("    + ", Style::default().fg(theme.accent_success)),
                                Span::styled(add_names, Style::default().fg(theme.accent_success)),
                            ]))
                            .build(),
                            Length(1),
                        );
                    }
                    if !association_diff.flemishshare_to_remove.is_empty() {
                        let remove_names = format_names(
                            &association_diff.flemishshare_to_remove,
                            &existing_assoc.flemishshare_names,
                        );
                        builder = builder.add(
                            Element::styled_text(Line::from(vec![
                                Span::styled("    - ", Style::default().fg(theme.accent_error)),
                                Span::styled(remove_names, Style::default().fg(theme.accent_error)),
                            ]))
                            .build(),
                            Length(1),
                        );
                    }
                }

                // Subcategory changes (NRQ only)
                if !association_diff.subcategory_to_add.is_empty()
                    || !association_diff.subcategory_to_remove.is_empty()
                {
                    builder = builder.add(
                        Element::styled_text(Line::from(vec![Span::styled(
                            "  Subcategory:",
                            Style::default().fg(theme.text_secondary).bold(),
                        )]))
                        .build(),
                        Length(1),
                    );
                    if !association_diff.subcategory_to_add.is_empty() {
                        let add_names =
                            format!("{} item(s)", association_diff.subcategory_to_add.len());
                        builder = builder.add(
                            Element::styled_text(Line::from(vec![
                                Span::styled("    + ", Style::default().fg(theme.accent_success)),
                                Span::styled(add_names, Style::default().fg(theme.accent_success)),
                            ]))
                            .build(),
                            Length(1),
                        );
                    }
                    if !association_diff.subcategory_to_remove.is_empty() {
                        let remove_names = format_names(
                            &association_diff.subcategory_to_remove,
                            &existing_assoc.subcategory_names,
                        );
                        builder = builder.add(
                            Element::styled_text(Line::from(vec![
                                Span::styled("    - ", Style::default().fg(theme.accent_error)),
                                Span::styled(remove_names, Style::default().fg(theme.accent_error)),
                            ]))
                            .build(),
                            Length(1),
                        );
                    }
                }

                builder = builder.add(spacer!(), Length(1));
            } else {
                builder = builder.add(
                    Element::styled_text(Line::from(vec![Span::styled(
                        "📋 No association changes",
                        Style::default().fg(theme.text_tertiary),
                    )]))
                    .build(),
                    Length(1),
                );
                builder = builder.add(spacer!(), Length(1));
            }
        }
    }

    // Direct fields section
    if !record.direct_fields.is_empty() {
        builder = builder.add(
            Element::styled_text(Line::from(vec![Span::styled(
                "📝 Direct Fields",
                Style::default().fg(theme.accent_secondary).bold(),
            )]))
            .build(),
            Length(1),
        );

        for (key, value) in &record.direct_fields {
            builder = builder.add(
                Element::styled_text(Line::from(vec![
                    Span::styled(
                        format!("  {}: ", key),
                        Style::default().fg(theme.text_tertiary),
                    ),
                    Span::styled(value.clone(), Style::default().fg(theme.text_primary)),
                ]))
                .build(),
                Length(1),
            );
        }
        builder = builder.add(spacer!(), Length(1));
    }

    // Picklist fields section
    if !record.picklist_fields.is_empty() {
        builder = builder.add(
            Element::styled_text(Line::from(vec![Span::styled(
                "🔢 Picklist Fields",
                Style::default().fg(theme.accent_secondary).bold(),
            )]))
            .build(),
            Length(1),
        );

        for (key, value) in &record.picklist_fields {
            // Map value back to label for display
            let display_value = match *value {
                // CGK values
                806150000 => "Automatische steun (806150000)",
                806150001 => "Selectieve steun (806150001)",
                // NRQ values
                875810000 => "Automatische steun (875810000)",
                875810001 => "Selectieve steun (875810001)",
                _ => "Unknown",
            };

            builder = builder.add(
                Element::styled_text(Line::from(vec![
                    Span::styled(
                        format!("  {}: ", key),
                        Style::default().fg(theme.text_tertiary),
                    ),
                    Span::styled(display_value, Style::default().fg(theme.accent_primary)),
                ]))
                .build(),
                Length(1),
            );
        }
        builder = builder.add(spacer!(), Length(1));
    }

    // Boolean fields section
    if !record.boolean_fields.is_empty() {
        builder = builder.add(
            Element::styled_text(Line::from(vec![Span::styled(
                "✓ Boolean Fields",
                Style::default().fg(theme.accent_secondary).bold(),
            )]))
            .build(),
            Length(1),
        );

        for (key, value) in &record.boolean_fields {
            let display_value = if *value { "true" } else { "false" };
            let color = if *value {
                theme.accent_success
            } else {
                theme.text_tertiary
            };

            builder = builder.add(
                Element::styled_text(Line::from(vec![
                    Span::styled(
                        format!("  {}: ", key),
                        Style::default().fg(theme.text_tertiary),
                    ),
                    Span::styled(display_value, Style::default().fg(color)),
                ]))
                .build(),
                Length(1),
            );
        }
        builder = builder.add(spacer!(), Length(1));
    }

    // Lookup fields section
    if !record.lookup_fields.is_empty() {
        builder = builder.add(
            Element::styled_text(Line::from(vec![Span::styled(
                "🔗 Lookup Fields (Resolved IDs)",
                Style::default().fg(theme.accent_secondary).bold(),
            )]))
            .build(),
            Length(1),
        );

        for (key, (id, target_entity)) in &record.lookup_fields {
            let truncated = if id.len() > 20 {
                format!("{}...", &id[..20])
            } else {
                id.clone()
            };

            builder = builder.add(
                Element::styled_text(Line::from(vec![
                    Span::styled(
                        format!("  {}: ", key),
                        Style::default().fg(theme.text_tertiary),
                    ),
                    Span::styled(truncated, Style::default().fg(theme.accent_success)),
                    Span::styled(
                        format!(" ({})", target_entity),
                        Style::default().fg(theme.border_primary),
                    ),
                ]))
                .build(),
                Length(1),
            );
        }
        builder = builder.add(spacer!(), Length(1));
    }

    // Dates section
    if record.deadline_date.is_some() || record.commission_date.is_some() {
        builder = builder.add(
            Element::styled_text(Line::from(vec![Span::styled(
                "📅 Dates",
                Style::default().fg(theme.accent_secondary).bold(),
            )]))
            .build(),
            Length(1),
        );

        if let Some(date) = record.deadline_date {
            let mut line = vec![
                Span::styled(
                    "  Deadline Date: ",
                    Style::default().fg(theme.text_tertiary),
                ),
                Span::styled(
                    date.format("%Y-%m-%d").to_string(),
                    Style::default().fg(theme.text_primary),
                ),
            ];

            if let Some(time) = record.deadline_time {
                line.push(Span::styled(
                    " at ",
                    Style::default().fg(theme.text_tertiary),
                ));
                line.push(Span::styled(
                    time.format("%H:%M:%S").to_string(),
                    Style::default().fg(theme.text_primary),
                ));
            }

            builder = builder.add(Element::styled_text(Line::from(line)).build(), Length(1));
        }

        if let Some(date) = record.commission_date {
            builder = builder.add(
                Element::styled_text(Line::from(vec![
                    Span::styled(
                        "  Commission Date: ",
                        Style::default().fg(theme.text_tertiary),
                    ),
                    Span::styled(
                        date.format("%Y-%m-%d").to_string(),
                        Style::default().fg(theme.text_primary),
                    ),
                ]))
                .build(),
                Length(1),
            );
        }
        builder = builder.add(spacer!(), Length(1));
    }

    // Checkbox relationships section
    if !record.checkbox_relationships.is_empty() {
        builder = builder.add(
            Element::styled_text(Line::from(vec![Span::styled(
                "☑️  Checkbox Relationships (N:N)",
                Style::default().fg(theme.accent_secondary).bold(),
            )]))
            .build(),
            Length(1),
        );

        for (relationship, ids) in &record.checkbox_relationships {
            builder = builder.add(
                Element::styled_text(Line::from(vec![
                    Span::styled(
                        format!("  {}: ", relationship),
                        Style::default().fg(theme.text_tertiary),
                    ),
                    Span::styled(
                        format!("{} items", ids.len()),
                        Style::default().fg(theme.accent_muted),
                    ),
                ]))
                .build(),
                Length(1),
            );

            // Show first few IDs
            for (idx, id) in ids.iter().take(3).enumerate() {
                let truncated = if id.len() > 16 {
                    format!("{}...", &id[..16])
                } else {
                    id.clone()
                };

                builder = builder.add(
                    Element::styled_text(Line::from(vec![
                        Span::styled(
                            format!("    {}: ", idx + 1),
                            Style::default().fg(theme.border_secondary),
                        ),
                        Span::styled(truncated, Style::default().fg(theme.accent_success)),
                    ]))
                    .build(),
                    Length(1),
                );
            }

            if ids.len() > 3 {
                builder = builder.add(
                    Element::styled_text(Line::from(vec![Span::styled(
                        format!("    ... and {} more", ids.len() - 3),
                        Style::default().fg(theme.border_secondary).italic(),
                    )]))
                    .build(),
                    Length(1),
                );
            }
        }
        builder = builder.add(spacer!(), Length(1));
    }

    // Custom junction records section (e.g., nrq_deadlinesupport)
    if !record.custom_junction_records.is_empty() {
        builder = builder.add(
            Element::styled_text(Line::from(vec![Span::styled(
                "🔗 Custom Junction Records",
                Style::default().fg(theme.accent_secondary).bold(),
            )]))
            .build(),
            Length(1),
        );

        // Group by junction entity
        let mut by_entity: std::collections::HashMap<
            &str,
            Vec<&super::models::CustomJunctionRecord>,
        > = std::collections::HashMap::new();
        for rec in &record.custom_junction_records {
            by_entity.entry(&rec.junction_entity).or_default().push(rec);
        }

        for (junction_entity, records) in &by_entity {
            builder = builder.add(
                Element::styled_text(Line::from(vec![
                    Span::styled(
                        format!("  {}: ", junction_entity),
                        Style::default().fg(theme.text_tertiary),
                    ),
                    Span::styled(
                        format!("{} items", records.len()),
                        Style::default().fg(theme.accent_muted),
                    ),
                ]))
                .build(),
                Length(1),
            );

            // Show first few related IDs
            for (idx, rec) in records.iter().take(3).enumerate() {
                let truncated = if rec.related_id.len() > 16 {
                    format!("{}...", &rec.related_id[..16])
                } else {
                    rec.related_id.clone()
                };

                builder = builder.add(
                    Element::styled_text(Line::from(vec![
                        Span::styled(
                            format!("    {}: ", idx + 1),
                            Style::default().fg(theme.border_secondary),
                        ),
                        Span::styled(
                            format!("{} → ", rec.related_entity),
                            Style::default().fg(theme.text_tertiary),
                        ),
                        Span::styled(truncated, Style::default().fg(theme.accent_success)),
                    ]))
                    .build(),
                    Length(1),
                );
            }

            if records.len() > 3 {
                builder = builder.add(
                    Element::styled_text(Line::from(vec![Span::styled(
                        format!("    ... and {} more", records.len() - 3),
                        Style::default().fg(theme.border_secondary).italic(),
                    )]))
                    .build(),
                    Length(1),
                );
            }
        }
        builder = builder.add(spacer!(), Length(1));
    }

    // Warnings section
    builder = builder.add(
        Element::styled_text(Line::from(vec![Span::styled(
            if record.has_warnings() {
                "⚠️  Warnings"
            } else {
                "✅ Status"
            },
            Style::default()
                .fg(if record.has_warnings() {
                    theme.accent_warning
                } else {
                    theme.accent_success
                })
                .bold(),
        )]))
        .build(),
        Length(1),
    );

    if !record.warnings.is_empty() {
        for warning in &record.warnings {
            builder = builder.add(
                Element::styled_text(Line::from(vec![
                    Span::styled("  • ", Style::default().fg(theme.accent_warning)),
                    Span::styled(warning.clone(), Style::default().fg(theme.accent_error)),
                ]))
                .build(),
                Length(1),
            );
        }
    } else {
        builder = builder.add(
            Element::styled_text(Line::from(vec![Span::styled(
                "  No warnings - record is ready for upload",
                Style::default().fg(theme.accent_success),
            )]))
            .build(),
            Length(1),
        );
    }

    builder.build()
}

/// Extract entity GUID from OperationResult headers or body
fn extract_entity_guid_from_result(
    result: &crate::api::operations::OperationResult,
) -> Option<String> {
    // Try headers first (OData-EntityId or Location)
    for (key, value) in &result.headers {
        if key.eq_ignore_ascii_case("odata-entityid") || key.eq_ignore_ascii_case("location") {
            // Format: /entityset(guid) or https://host/api/data/v9.2/entityset(guid)
            // Extract GUID using regex
            if let Some(start) = value.rfind('(') {
                if let Some(end) = value.rfind(')') {
                    if end > start {
                        return Some(value[start + 1..end].to_string());
                    }
                }
            }
        }
    }

    // Try response body (when Prefer: return=representation is used)
    if let Some(ref data) = result.data {
        // Look for common ID field names
        if let Some(id_value) = data
            .get("cgk_deadlineid")
            .or_else(|| data.get("nrq_deadlineid"))
            .or_else(|| data.get("id"))
        {
            if let Some(guid_str) = id_value.as_str() {
                return Some(guid_str.to_string());
            }
        }
    }

    None
}

/// Batch deadline creates into queue items with max_per_batch operations each
fn batch_deadline_creates(
    records: &[&TransformedDeadline],
    entity_type: &str,
    environment_name: &str,
    queued_items: &mut HashMap<String, Vec<TransformedDeadline>>,
    max_per_batch: usize,
) -> Vec<QueueItem> {
    use crate::api::operations::Operations;

    let mut queue_items = Vec::new();
    let mut current_batch_ops = Vec::new();
    let mut current_batch_records = Vec::new();
    let mut batch_num = 1;

    for record in records {
        // Add to current batch
        let operations_vec = record.to_operations(entity_type);
        current_batch_ops.push(operations_vec[0].clone()); // Each deadline is 1 Create operation
        current_batch_records.push((*record).clone());

        // If we hit the batch limit, create a queue item
        if current_batch_ops.len() >= max_per_batch {
            let operations = Operations::from_operations(current_batch_ops.clone());
            let metadata = QueueMetadata {
                source: "Deadlines Excel".to_string(),
                entity_type: entity_type.to_string(),
                description: format!(
                    "Deadline batch {} ({} deadlines)",
                    batch_num,
                    current_batch_ops.len()
                ),
                row_number: None,
                environment_name: environment_name.to_string(),
            };
            let priority = 64; // High priority for deadline creates
            let queue_item = QueueItem::new(operations, metadata, priority);

            // Track all records in this batch for association creation later
            queued_items.insert(queue_item.id.clone(), current_batch_records.clone());

            queue_items.push(queue_item);

            // Start new batch
            current_batch_ops.clear();
            current_batch_records.clear();
            batch_num += 1;
        }
    }

    // Create queue item for remaining deadlines
    if !current_batch_ops.is_empty() {
        let operations = Operations::from_operations(current_batch_ops.clone());
        let metadata = QueueMetadata {
            source: "Deadlines Excel".to_string(),
            entity_type: entity_type.to_string(),
            description: format!(
                "Deadline batch {} ({} deadlines)",
                batch_num,
                current_batch_ops.len()
            ),
            row_number: None,
            environment_name: environment_name.to_string(),
        };
        let priority = 64; // High priority for deadline creates
        let queue_item = QueueItem::new(operations, metadata, priority);

        // Track all records in this batch for association creation later
        queued_items.insert(queue_item.id.clone(), current_batch_records.clone());

        queue_items.push(queue_item);
    }

    queue_items
}

/// Build AssociateRef operations for N:N relationships
fn build_association_operations(
    entity_guid: &str,
    entity_type: &str,
    checkbox_relationships: &HashMap<String, Vec<String>>,
) -> Vec<crate::api::operations::Operation> {
    use super::operation_builder::{
        extract_related_entity_from_relationship, get_junction_entity_name,
    };
    use crate::api::operations::Operation;
    use crate::api::pluralization::pluralize_entity_name;

    let mut operations = Vec::new();
    let entity_set = pluralize_entity_name(entity_type);

    for (relationship_name, related_ids) in checkbox_relationships {
        if related_ids.is_empty() {
            continue;
        }

        let junction_entity = get_junction_entity_name(entity_type, relationship_name);
        let related_entity = extract_related_entity_from_relationship(relationship_name);
        let related_entity_set = pluralize_entity_name(&related_entity);

        for related_id in related_ids {
            // Relative URI for @odata.id binding
            let target_ref = format!("/{}({})", related_entity_set, related_id);

            operations.push(Operation::AssociateRef {
                entity: entity_set.clone(),
                entity_ref: entity_guid.to_string(),
                navigation_property: junction_entity.clone(),
                target_ref,
            });
        }
    }

    operations
}

/// Build Create operations for custom junction entities (e.g., nrq_deadlinesupport)
/// These are separate entities that link deadline to related records, with additional fields
fn build_custom_junction_operations(
    entity_guid: &str,
    entity_type: &str,
    custom_junction_records: &[super::models::CustomJunctionRecord],
) -> Vec<crate::api::operations::Operation> {
    use crate::api::operations::Operation;
    use crate::api::pluralization::pluralize_entity_name;
    use serde_json::json;

    let mut operations = Vec::new();
    let main_entity_set = pluralize_entity_name(entity_type);

    for record in custom_junction_records {
        let junction_entity_set = pluralize_entity_name(&record.junction_entity);
        let related_entity_set = pluralize_entity_name(&record.related_entity);

        // Build payload with lookups to main entity and related entity
        let mut payload = serde_json::Map::new();

        // Main entity lookup (e.g., nrq_deadlineid@odata.bind)
        let main_bind_field = format!("{}@odata.bind", record.main_entity_field);
        payload.insert(
            main_bind_field,
            json!(format!("/{}({})", main_entity_set, entity_guid)),
        );

        // Related entity lookup (e.g., nrq_supportid@odata.bind)
        let related_bind_field = format!("{}@odata.bind", record.related_entity_field);
        payload.insert(
            related_bind_field,
            json!(format!("/{}({})", related_entity_set, record.related_id)),
        );

        // Add entity-specific fields for nrq_deadlinesupport
        if record.junction_entity == "nrq_deadlinesupport" {
            // Name field = the matched support name from Dynamics
            payload.insert("nrq_name".to_string(), json!(record.related_name));
            // Constants
            payload.insert("nrq_enablehearing".to_string(), json!(false));
            payload.insert("nrq_enablereporter".to_string(), json!(true));
        }

        operations.push(Operation::Create {
            entity: junction_entity_set,
            data: serde_json::Value::Object(payload),
        });
    }

    operations
}

/// Batch associations into queue items with max_per_batch operations each
/// Never splits a single deadline's associations across multiple batches
fn batch_associations(
    pending_associations: &HashMap<String, Vec<crate::api::operations::Operation>>,
    entity_type: &str,
    environment_name: &str,
    max_per_batch: usize,
) -> Vec<QueueItem> {
    use crate::api::operations::Operations;

    let mut queue_items = Vec::new();
    let mut current_batch = Vec::new();
    let mut current_batch_count = 0;
    let mut batch_num = 1;

    for (deadline_guid, ops) in pending_associations {
        let ops_count = ops.len();

        // If this deadline's associations would exceed the batch limit and we have operations already,
        // create a queue item for the current batch
        if !current_batch.is_empty() && current_batch_count + ops_count > max_per_batch {
            let operations = Operations::from_operations(current_batch.clone());
            let metadata = QueueMetadata {
                source: "Deadlines Excel (Associations)".to_string(),
                entity_type: entity_type.to_string(),
                description: format!(
                    "Association batch {} ({} operations)",
                    batch_num, current_batch_count
                ),
                row_number: None,
                environment_name: environment_name.to_string(),
            };
            let priority = 128; // Medium priority for associations
            queue_items.push(QueueItem::new(operations, metadata, priority));

            // Start new batch
            current_batch.clear();
            current_batch_count = 0;
            batch_num += 1;
        }

        // Add this deadline's associations to the current batch
        current_batch.extend(ops.clone());
        current_batch_count += ops_count;
    }

    // Create queue item for remaining operations
    if !current_batch.is_empty() {
        let operations = Operations::from_operations(current_batch);
        let metadata = QueueMetadata {
            source: "Deadlines Excel (Associations)".to_string(),
            entity_type: entity_type.to_string(),
            description: format!(
                "Association batch {} ({} operations)",
                batch_num, current_batch_count
            ),
            row_number: None,
            environment_name: environment_name.to_string(),
        };
        let priority = 128; // Medium priority for associations
        queue_items.push(QueueItem::new(operations, metadata, priority));
    }

    queue_items
}

/// Batch deadline updates into queue items
///
/// Each update record generates:
/// 1. PATCH operation for field changes
/// 2. Disassociate operations for removed N:N relationships
/// 3. Associate operations for added N:N relationships
/// 4. Delete operations for removed custom junctions (NRQ support)
/// 5. Create operations for added custom junctions (NRQ support)
fn batch_deadline_updates(
    records: &[&TransformedDeadline],
    entity_type: &str,
    environment_name: &str,
    max_per_batch: usize,
) -> Vec<QueueItem> {
    use crate::api::operations::Operations;

    let mut queue_items = Vec::new();
    let is_cgk = entity_type == "cgk_deadline";

    // Collect operations into three separate batches:
    // 1. Field updates (PATCH) - priority 64
    // 2. Deletes/Disassociates - priority 63
    // 3. Creates/Associates - priority 62
    let mut update_ops_batch = Vec::new();
    let mut delete_ops_batch = Vec::new();
    let mut create_ops_batch = Vec::new();
    let mut update_batch_num = 1;
    let mut delete_batch_num = 1;
    let mut create_batch_num = 1;

    for record in records {
        // Skip records without existing_guid (shouldn't happen for Update mode)
        let entity_guid = match &record.existing_guid {
            Some(guid) => guid.clone(),
            None => {
                log::error!("Update record missing existing_guid - skipping");
                continue;
            }
        };

        // 1. PATCH operation for field changes (separate batch)
        let update_ops = record.to_update_operations(entity_type);
        update_ops_batch.extend(update_ops);

        // Flush update batch if at limit
        if update_ops_batch.len() >= max_per_batch {
            let operations = Operations::from_operations(update_ops_batch.clone());
            let metadata = QueueMetadata {
                source: "Deadlines Excel (Updates)".to_string(),
                entity_type: entity_type.to_string(),
                description: format!(
                    "Field update batch {} ({} operations)",
                    update_batch_num,
                    update_ops_batch.len()
                ),
                row_number: None,
                environment_name: environment_name.to_string(),
            };
            queue_items.push(QueueItem::new(operations, metadata, 64));
            update_ops_batch.clear();
            update_batch_num += 1;
        }

        // Get existing associations for diffing
        let existing_associations = match &record.existing_associations {
            Some(assoc) => assoc,
            None => {
                log::warn!(
                    "Update record missing existing_associations - skipping association sync"
                );
                continue;
            }
        };

        // 2. Compute association diff
        let association_diff = diff_associations(record, existing_associations, entity_type);

        // 3. Disassociate operations for removed N:N (delete batch)
        let disassociate_ops =
            build_disassociate_operations(&entity_guid, entity_type, &association_diff);
        delete_ops_batch.extend(disassociate_ops);

        // 4. Associate operations for added N:N (create batch)
        let associate_ops =
            build_associate_operations(&entity_guid, entity_type, &association_diff);
        create_ops_batch.extend(associate_ops);

        // 5. For NRQ: Handle custom junction (nrq_deadlinesupport)
        if !is_cgk {
            // Delete removed support junctions (delete batch)
            let delete_junction_ops = build_delete_junction_operations(
                existing_associations,
                &association_diff.support_to_remove,
            );
            delete_ops_batch.extend(delete_junction_ops);

            // Create added support junctions (create batch)
            let create_junction_ops = build_create_junction_operations(
                &entity_guid,
                &association_diff.support_to_add,
                &record.custom_junction_records,
            );
            create_ops_batch.extend(create_junction_ops);
        }

        // Flush delete batch if at limit
        if delete_ops_batch.len() >= max_per_batch {
            let operations = Operations::from_operations(delete_ops_batch.clone());
            let metadata = QueueMetadata {
                source: "Deadlines Excel (Removals)".to_string(),
                entity_type: entity_type.to_string(),
                description: format!(
                    "Delete/Disassociate batch {} ({} operations)",
                    delete_batch_num,
                    delete_ops_batch.len()
                ),
                row_number: None,
                environment_name: environment_name.to_string(),
            };
            queue_items.push(QueueItem::new(operations, metadata, 63));
            delete_ops_batch.clear();
            delete_batch_num += 1;
        }

        // Flush create batch if at limit
        if create_ops_batch.len() >= max_per_batch {
            let operations = Operations::from_operations(create_ops_batch.clone());
            let metadata = QueueMetadata {
                source: "Deadlines Excel (Additions)".to_string(),
                entity_type: entity_type.to_string(),
                description: format!(
                    "Create/Associate batch {} ({} operations)",
                    create_batch_num,
                    create_ops_batch.len()
                ),
                row_number: None,
                environment_name: environment_name.to_string(),
            };
            queue_items.push(QueueItem::new(operations, metadata, 62));
            create_ops_batch.clear();
            create_batch_num += 1;
        }
    }

    // Flush remaining update operations
    if !update_ops_batch.is_empty() {
        let operations = Operations::from_operations(update_ops_batch.clone());
        let metadata = QueueMetadata {
            source: "Deadlines Excel (Updates)".to_string(),
            entity_type: entity_type.to_string(),
            description: format!(
                "Field update batch {} ({} operations)",
                update_batch_num,
                update_ops_batch.len()
            ),
            row_number: None,
            environment_name: environment_name.to_string(),
        };
        queue_items.push(QueueItem::new(operations, metadata, 64));
    }

    // Flush remaining delete operations
    if !delete_ops_batch.is_empty() {
        let operations = Operations::from_operations(delete_ops_batch.clone());
        let metadata = QueueMetadata {
            source: "Deadlines Excel (Removals)".to_string(),
            entity_type: entity_type.to_string(),
            description: format!(
                "Delete/Disassociate batch {} ({} operations)",
                delete_batch_num,
                delete_ops_batch.len()
            ),
            row_number: None,
            environment_name: environment_name.to_string(),
        };
        queue_items.push(QueueItem::new(operations, metadata, 63));
    }

    // Flush remaining create operations
    if !create_ops_batch.is_empty() {
        let operations = Operations::from_operations(create_ops_batch.clone());
        let metadata = QueueMetadata {
            source: "Deadlines Excel (Additions)".to_string(),
            entity_type: entity_type.to_string(),
            description: format!(
                "Create/Associate batch {} ({} operations)",
                create_batch_num,
                create_ops_batch.len()
            ),
            row_number: None,
            environment_name: environment_name.to_string(),
        };
        queue_items.push(QueueItem::new(operations, metadata, 62));
    }

    log::info!(
        "Created {} update queue items with {} total operations",
        queue_items.len(),
        records.len()
    );

    queue_items
}
