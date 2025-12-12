use crossterm::event::KeyCode;
use std::collections::HashMap;
use crate::tui::{App, AppId, Command, Element, Subscription, Theme, LayeredView, FocusId};
use crate::tui::element::LayoutConstraint::*;
use crate::tui::widgets::list::{ListItem, ListState};
use crate::{col, row, spacer, use_constraints};
use ratatui::text::{Line, Span};
use ratatui::style::{Style, Stylize};

use super::models::{InspectionParams, TransformedDeadline};
use crate::tui::apps::queue::{QueueItem, QueueMetadata};
use crate::api::operations::Operations;

pub struct DeadlinesInspectionApp;

/// Wrapper for TransformedDeadline to implement ListItem trait
#[derive(Clone)]
struct RecordListItem {
    record: TransformedDeadline,
    entity_type: String, // For determining field prefix
}

impl ListItem for RecordListItem {
    type Msg = Msg;

    fn to_element(&self, is_selected: bool, _is_multi_selected: bool, _is_hovered: bool) -> Element<Msg> {
        let theme = &crate::global_runtime_config().theme;
        let (fg_color, bg_style) = if is_selected {
            (theme.accent_primary, Some(Style::default().bg(theme.bg_surface)))
        } else {
            (theme.text_primary, None)
        };

        // Extract name from direct fields
        let name_field = if self.entity_type == "cgk_deadline" { "cgk_deadlinename" } else { "nrq_name" };
        let name = self.record.direct_fields.get(name_field)
            .map(|s| s.as_str())
            .unwrap_or("<No Name>");

        // Truncate name if too long
        let display_name = if name.len() > 35 {
            format!("{}...", &name[..32])
        } else {
            name.to_string()
        };

        // Warning indicator
        let warning_indicator = if self.record.has_warnings() {
            Span::styled("⚠ ", Style::default().fg(theme.accent_warning))
        } else {
            Span::styled("  ", Style::default())
        };

        let mut builder = Element::styled_text(Line::from(vec![
            warning_indicator,
            Span::styled(format!("Row {}: ", self.record.source_row), Style::default().fg(theme.text_tertiary)),
            Span::styled(display_name, Style::default().fg(fg_color)),
        ]));

        if let Some(bg) = bg_style {
            builder = builder.background(bg);
        }

        builder.build()
    }
}

#[derive(Clone)]
pub struct State {
    entity_type: String,
    transformed_records: Vec<TransformedDeadline>,
    list_state: ListState,
    selected_record_idx: usize,
    /// Track queue items created from this inspection session (queue_item_id -> Vec<TransformedDeadline>)
    queued_items: HashMap<String, Vec<TransformedDeadline>>,
    /// Total number of deadlines queued in current batch
    total_deadlines_queued: usize,
    /// Accumulated associations from completed deadline creations (deadline_guid -> operations)
    pending_associations: HashMap<String, Vec<crate::api::operations::Operation>>,
}

impl State {
    fn new(entity_type: String, transformed_records: Vec<TransformedDeadline>) -> Self {
        let mut list_state = ListState::default();
        // Auto-select first record if any exist
        let item_count = transformed_records.len();
        if !transformed_records.is_empty() {
            list_state.select_and_scroll(Some(0), item_count);
        }

        Self {
            entity_type,
            transformed_records,
            list_state,
            selected_record_idx: 0,
            queued_items: HashMap::new(),
            total_deadlines_queued: 0,
            pending_associations: HashMap::new(),
        }
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
    EnvironmentLoaded(Result<String, String>),
    QueueItemCompleted(String, crate::tui::apps::queue::models::QueueResult, crate::tui::apps::queue::models::QueueMetadata),
}

impl crate::tui::AppState for State {}

impl App for DeadlinesInspectionApp {
    type State = State;
    type Msg = Msg;
    type InitParams = InspectionParams;

    fn init(params: Self::InitParams) -> (State, Command<Msg>) {
        let state = State::new(
            params.entity_type,
            params.transformed_records,
        );

        (state, Command::set_focus(FocusId::new("queue-button")))
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::SelectRecord(idx) => {
                if idx < state.transformed_records.len() {
                    state.selected_record_idx = idx;
                    let item_count = state.transformed_records.len();
                    state.list_state.select_and_scroll(Some(idx), item_count);
                }
                Command::None
            }
            Msg::ListNavigate(key) => {
                // ListState will use its stored viewport_height from on_render, fallback to 20
                state.list_state.handle_key(key, state.transformed_records.len(), 20);

                // Sync selected_record_idx with list_state
                if let Some(selected) = state.list_state.selected() {
                    if selected != state.selected_record_idx && selected < state.transformed_records.len() {
                        state.selected_record_idx = selected;
                    }
                }

                Command::None
            }
            Msg::SetViewportHeight(height) => {
                let item_count = state.transformed_records.len();
                state.list_state.set_viewport_height(height);
                state.list_state.update_scroll(height, item_count);
                Command::None
            }
            Msg::Back => Command::navigate_to(AppId::DeadlinesMapping),
            Msg::AddToQueueAndView => {
                // Get current environment first
                Command::perform(
                    async {
                        let manager = crate::client_manager();
                        manager.get_current_environment_name().await
                            .map_err(|e| e.to_string())?
                            .ok_or_else(|| "No environment selected".to_string())
                    },
                    Msg::EnvironmentLoaded
                )
            }
            Msg::EnvironmentLoaded(Ok(environment_name)) => {
                // Collect all valid records
                let mut valid_records: Vec<&TransformedDeadline> = state.transformed_records.iter()
                    .filter(|record| !record.has_warnings())
                    .collect();

                if valid_records.is_empty() {
                    log::warn!("No valid records to queue");
                    return Command::None;
                }

                // Track total for association batching later
                state.total_deadlines_queued = valid_records.len();

                // Batch deadline creates into groups of 50
                let queue_items = batch_deadline_creates(
                    &valid_records,
                    &state.entity_type,
                    &environment_name,
                    &mut state.queued_items,
                    50
                );

                // Serialize queue items to JSON for pub/sub
                let queue_items_json = match serde_json::to_value(&queue_items) {
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
                        log::error!("Mismatch: {} operation results but {} records in batch", num_results, num_records);
                        return Command::None;
                    }

                    log::info!("Processing {} deadline creates from batch", num_results);

                    // Extract GUIDs and build associations for each deadline in the batch
                    for (idx, op_result) in result.operation_results.iter().enumerate() {
                        let record = &records[idx];

                        let created_guid = match extract_entity_guid_from_result(op_result) {
                            Some(guid) => guid,
                            None => {
                                log::error!("Failed to extract entity GUID from operation result {}", idx);
                                continue;
                            }
                        };

                        log::debug!("Deadline #{} created with GUID: {}", idx + 1, created_guid);

                        // Generate AssociateRef operations for N:N relationships
                        let mut all_ops = build_association_operations(
                            &created_guid,
                            &state.entity_type,
                            &record.checkbox_relationships
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
                            state.pending_associations.insert(created_guid.clone(), all_ops);
                        }
                    }

                    // Check if all deadlines have been processed
                    if state.pending_associations.len() >= state.total_deadlines_queued {
                        log::info!("All {} deadlines created, batching associations", state.total_deadlines_queued);

                        // Batch associations in groups of 50 max, never splitting a deadline's associations
                        let batched_queue_items = batch_associations(
                            &state.pending_associations,
                            &state.entity_type,
                            &metadata.environment_name,
                            50
                        );

                        if batched_queue_items.is_empty() {
                            log::info!("No associations to create");
                            state.pending_associations.clear();
                            return Command::None;
                        }

                        log::info!("Created {} association queue items", batched_queue_items.len());

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

        // Convert records to list items
        let list_items: Vec<RecordListItem> = state.transformed_records.iter()
            .map(|r| RecordListItem {
                record: r.clone(),
                entity_type: state.entity_type.clone(),
            })
            .collect();

        // Left panel: record list
        let record_list = Element::list(
            "record-list",
            &list_items,
            &state.list_state,
            theme
        )
        .on_select(Msg::SelectRecord)
        .on_activate(Msg::SelectRecord)
        .on_navigate(Msg::ListNavigate)
        .on_render(Msg::SetViewportHeight)
        .build();

        let records_with_warnings = state.transformed_records.iter()
            .filter(|r| r.has_warnings())
            .count();

        let list_title = format!("Records ({} total, {} warnings)",
            state.transformed_records.len(),
            records_with_warnings
        );

        let left_panel = Element::panel(record_list)
            .title(&list_title)
            .build();

        // Right panel: details for selected record
        let detail_content = if let Some(record) = state.transformed_records.get(state.selected_record_idx) {
            build_detail_panel(record, &state.entity_type)
        } else {
            col![
                Element::styled_text(Line::from(vec![
                    Span::styled("No record selected", Style::default().fg(theme.text_tertiary))
                ])).build()
            ]
        };

        let detail_title = if let Some(record) = state.transformed_records.get(state.selected_record_idx) {
            let name_field = if state.entity_type == "cgk_deadline" { "cgk_deadlinename" } else { "nrq_name" };
            let name = record.direct_fields.get(name_field)
                .map(|s| s.as_str())
                .unwrap_or("<No Name>");
            format!("Row {} - {}", record.source_row, name)
        } else {
            "Record Details".to_string()
        };

        let right_panel = Element::panel(detail_content)
            .title(&detail_title)
            .build();

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
                let result: crate::tui::apps::queue::models::QueueResult = serde_json::from_value(value.get("result")?.clone()).ok()?;
                let metadata: crate::tui::apps::queue::models::QueueMetadata = serde_json::from_value(value.get("metadata")?.clone()).ok()?;
                Some(Msg::QueueItemCompleted(id, result, metadata))
            }),
        ]
    }

    fn title() -> &'static str {
        "Deadlines - Inspection"
    }

    fn status(state: &State) -> Option<Line<'static>> {
        let theme = &crate::global_runtime_config().theme;
        let records_with_warnings = state.transformed_records.iter()
            .filter(|r| r.has_warnings())
            .count();

        Some(Line::from(vec![
            Span::styled("Records: ", Style::default().fg(theme.text_tertiary)),
            Span::styled(
                state.transformed_records.len().to_string(),
                Style::default().fg(theme.accent_primary),
            ),
            Span::styled(" | Warnings: ", Style::default().fg(theme.text_tertiary)),
            Span::styled(
                records_with_warnings.to_string(),
                Style::default().fg(if records_with_warnings > 0 { theme.accent_warning } else { theme.accent_success }),
            ),
        ]))
    }
}

/// Build the detail panel for a selected record
fn build_detail_panel(record: &TransformedDeadline, entity_type: &str) -> Element<Msg> {
    let theme = &crate::global_runtime_config().theme;
    use crate::tui::element::ColumnBuilder;

    let mut builder = ColumnBuilder::new();

    // Direct fields section
    if !record.direct_fields.is_empty() {
        builder = builder.add(Element::styled_text(Line::from(vec![
            Span::styled("📝 Direct Fields", Style::default().fg(theme.accent_secondary).bold())
        ])).build(), Length(1));

        for (key, value) in &record.direct_fields {
            builder = builder.add(Element::styled_text(Line::from(vec![
                Span::styled(format!("  {}: ", key), Style::default().fg(theme.text_tertiary)),
                Span::styled(value.clone(), Style::default().fg(theme.text_primary)),
            ])).build(), Length(1));
        }
        builder = builder.add(spacer!(), Length(1));
    }

    // Picklist fields section
    if !record.picklist_fields.is_empty() {
        builder = builder.add(Element::styled_text(Line::from(vec![
            Span::styled("🔢 Picklist Fields", Style::default().fg(theme.accent_secondary).bold())
        ])).build(), Length(1));

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

            builder = builder.add(Element::styled_text(Line::from(vec![
                Span::styled(format!("  {}: ", key), Style::default().fg(theme.text_tertiary)),
                Span::styled(display_value, Style::default().fg(theme.accent_primary)),
            ])).build(), Length(1));
        }
        builder = builder.add(spacer!(), Length(1));
    }

    // Lookup fields section
    if !record.lookup_fields.is_empty() {
        builder = builder.add(Element::styled_text(Line::from(vec![
            Span::styled("🔗 Lookup Fields (Resolved IDs)", Style::default().fg(theme.accent_secondary).bold())
        ])).build(), Length(1));

        for (key, (id, target_entity)) in &record.lookup_fields {
            let truncated = if id.len() > 20 {
                format!("{}...", &id[..20])
            } else {
                id.clone()
            };

            builder = builder.add(Element::styled_text(Line::from(vec![
                Span::styled(format!("  {}: ", key), Style::default().fg(theme.text_tertiary)),
                Span::styled(truncated, Style::default().fg(theme.accent_success)),
                Span::styled(format!(" ({})", target_entity), Style::default().fg(theme.border_primary)),
            ])).build(), Length(1));
        }
        builder = builder.add(spacer!(), Length(1));
    }

    // Dates section
    if record.deadline_date.is_some() || record.commission_date.is_some() {
        builder = builder.add(Element::styled_text(Line::from(vec![
            Span::styled("📅 Dates", Style::default().fg(theme.accent_secondary).bold())
        ])).build(), Length(1));

        if let Some(date) = record.deadline_date {
            let mut line = vec![
                Span::styled("  Deadline Date: ", Style::default().fg(theme.text_tertiary)),
                Span::styled(date.format("%Y-%m-%d").to_string(), Style::default().fg(theme.text_primary)),
            ];

            if let Some(time) = record.deadline_time {
                line.push(Span::styled(" at ", Style::default().fg(theme.text_tertiary)));
                line.push(Span::styled(time.format("%H:%M:%S").to_string(), Style::default().fg(theme.text_primary)));
            }

            builder = builder.add(Element::styled_text(Line::from(line)).build(), Length(1));
        }

        if let Some(date) = record.commission_date {
            builder = builder.add(Element::styled_text(Line::from(vec![
                Span::styled("  Commission Date: ", Style::default().fg(theme.text_tertiary)),
                Span::styled(date.format("%Y-%m-%d").to_string(), Style::default().fg(theme.text_primary)),
            ])).build(), Length(1));
        }
        builder = builder.add(spacer!(), Length(1));
    }

    // Checkbox relationships section
    if !record.checkbox_relationships.is_empty() {
        builder = builder.add(Element::styled_text(Line::from(vec![
            Span::styled("☑️  Checkbox Relationships (N:N)", Style::default().fg(theme.accent_secondary).bold())
        ])).build(), Length(1));

        for (relationship, ids) in &record.checkbox_relationships {
            builder = builder.add(Element::styled_text(Line::from(vec![
                Span::styled(format!("  {}: ", relationship), Style::default().fg(theme.text_tertiary)),
                Span::styled(format!("{} items", ids.len()), Style::default().fg(theme.accent_muted)),
            ])).build(), Length(1));

            // Show first few IDs
            for (idx, id) in ids.iter().take(3).enumerate() {
                let truncated = if id.len() > 16 {
                    format!("{}...", &id[..16])
                } else {
                    id.clone()
                };

                builder = builder.add(Element::styled_text(Line::from(vec![
                    Span::styled(format!("    {}: ", idx + 1), Style::default().fg(theme.border_secondary)),
                    Span::styled(truncated, Style::default().fg(theme.accent_success)),
                ])).build(), Length(1));
            }

            if ids.len() > 3 {
                builder = builder.add(Element::styled_text(Line::from(vec![
                    Span::styled(format!("    ... and {} more", ids.len() - 3), Style::default().fg(theme.border_secondary).italic()),
                ])).build(), Length(1));
            }
        }
        builder = builder.add(spacer!(), Length(1));
    }

    // Custom junction records section (e.g., nrq_deadlinesupport)
    if !record.custom_junction_records.is_empty() {
        builder = builder.add(Element::styled_text(Line::from(vec![
            Span::styled("🔗 Custom Junction Records", Style::default().fg(theme.accent_secondary).bold())
        ])).build(), Length(1));

        // Group by junction entity
        let mut by_entity: std::collections::HashMap<&str, Vec<&super::models::CustomJunctionRecord>> = std::collections::HashMap::new();
        for rec in &record.custom_junction_records {
            by_entity.entry(&rec.junction_entity).or_default().push(rec);
        }

        for (junction_entity, records) in &by_entity {
            builder = builder.add(Element::styled_text(Line::from(vec![
                Span::styled(format!("  {}: ", junction_entity), Style::default().fg(theme.text_tertiary)),
                Span::styled(format!("{} items", records.len()), Style::default().fg(theme.accent_muted)),
            ])).build(), Length(1));

            // Show first few related IDs
            for (idx, rec) in records.iter().take(3).enumerate() {
                let truncated = if rec.related_id.len() > 16 {
                    format!("{}...", &rec.related_id[..16])
                } else {
                    rec.related_id.clone()
                };

                builder = builder.add(Element::styled_text(Line::from(vec![
                    Span::styled(format!("    {}: ", idx + 1), Style::default().fg(theme.border_secondary)),
                    Span::styled(format!("{} → ", rec.related_entity), Style::default().fg(theme.text_tertiary)),
                    Span::styled(truncated, Style::default().fg(theme.accent_success)),
                ])).build(), Length(1));
            }

            if records.len() > 3 {
                builder = builder.add(Element::styled_text(Line::from(vec![
                    Span::styled(format!("    ... and {} more", records.len() - 3), Style::default().fg(theme.border_secondary).italic()),
                ])).build(), Length(1));
            }
        }
        builder = builder.add(spacer!(), Length(1));
    }

    // Warnings section
    builder = builder.add(Element::styled_text(Line::from(vec![
        Span::styled(
            if record.has_warnings() { "⚠️  Warnings" } else { "✅ Status" },
            Style::default().fg(if record.has_warnings() { theme.accent_warning } else { theme.accent_success }).bold()
        )
    ])).build(), Length(1));

    if !record.warnings.is_empty() {
        for warning in &record.warnings {
            builder = builder.add(Element::styled_text(Line::from(vec![
                Span::styled("  • ", Style::default().fg(theme.accent_warning)),
                Span::styled(warning.clone(), Style::default().fg(theme.accent_error)),
            ])).build(), Length(1));
        }
    } else {
        builder = builder.add(Element::styled_text(Line::from(vec![
            Span::styled("  No warnings - record is ready for upload", Style::default().fg(theme.accent_success))
        ])).build(), Length(1));
    }

    builder.build()
}

/// Extract entity GUID from OperationResult headers or body
fn extract_entity_guid_from_result(result: &crate::api::operations::OperationResult) -> Option<String> {
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
        if let Some(id_value) = data.get("cgk_deadlineid")
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
                description: format!("Deadline batch {} ({} deadlines)", batch_num, current_batch_ops.len()),
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
            description: format!("Deadline batch {} ({} deadlines)", batch_num, current_batch_ops.len()),
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
    use crate::api::operations::Operation;
    use crate::api::pluralization::pluralize_entity_name;
    use super::operation_builder::{get_junction_entity_name, extract_related_entity_from_relationship};

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
            // Relative URI - batch builder will convert to absolute
            let target_ref = format!("/api/data/v9.2/{}({})", related_entity_set, related_id);

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
            json!(format!("/{}({})", main_entity_set, entity_guid))
        );

        // Related entity lookup (e.g., nrq_supportid@odata.bind)
        let related_bind_field = format!("{}@odata.bind", record.related_entity_field);
        payload.insert(
            related_bind_field,
            json!(format!("/{}({})", related_entity_set, record.related_id))
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
                description: format!("Association batch {} ({} operations)", batch_num, current_batch_count),
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
            description: format!("Association batch {} ({} operations)", batch_num, current_batch_count),
            row_number: None,
            environment_name: environment_name.to_string(),
        };
        let priority = 128; // Medium priority for associations
        queue_items.push(QueueItem::new(operations, metadata, priority));
    }

    queue_items
}
