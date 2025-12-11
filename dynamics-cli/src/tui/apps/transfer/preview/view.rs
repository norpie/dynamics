//! View rendering for the Transfer Preview app

use crossterm::event::KeyCode;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::transfer::{RecordAction, ResolvedEntity, ResolvedRecord, ResolvedTransfer, Value};
use crate::tui::element::{ColumnBuilder, FocusId, RowBuilder};
use crate::tui::resource::Resource;
use crate::tui::widgets::{ListEvent, ListItem};
use crate::tui::{Alignment, Element, LayeredView, LayoutConstraint, Subscription, Theme};

use super::state::{Msg, PreviewModal, RecordFilter, State};

/// Render the preview app view
pub fn render(state: &mut State, theme: &Theme) -> LayeredView<Msg> {
    let content = match &state.resolved {
        Resource::NotAsked => {
            Element::text("No data loaded. Press Back to return to editor.")
        }
        Resource::Loading => {
            Element::text("Loading resolved records...")
        }
        Resource::Failure(err) => {
            Element::styled_text(Line::from(vec![
                Span::styled("Error: ", Style::default().fg(theme.accent_error)),
                Span::styled(err.clone(), Style::default().fg(theme.text_primary)),
            ]))
            .build()
        }
        Resource::Success(resolved) => {
            render_preview(state, resolved, theme)
        }
    };

    // Build header with env info
    let header = Element::styled_text(Line::from(vec![
        Span::styled("Source: ", Style::default().fg(theme.text_secondary)),
        Span::styled(state.source_env.clone(), Style::default().fg(theme.accent_primary)),
        Span::raw(" → "),
        Span::styled("Target: ", Style::default().fg(theme.text_secondary)),
        Span::styled(state.target_env.clone(), Style::default().fg(theme.accent_primary)),
    ]))
    .build();

    let main_content = ColumnBuilder::new()
        .add(header, LayoutConstraint::Length(1))
        .add(content, LayoutConstraint::Fill(1))
        .build();

    let main_view = Element::panel(main_content)
        .title(&format!("Preview: {}", state.config_name))
        .build();

    let mut view = LayeredView::new(main_view);

    // Render modals
    if let Some(modal) = &state.active_modal {
        let modal_element = match modal {
            PreviewModal::RecordDetails { record_idx } => {
                render_record_details_placeholder(*record_idx, theme)
            }
            PreviewModal::EditRecord { record_idx } => {
                render_edit_record_placeholder(*record_idx, theme)
            }
            PreviewModal::BulkActions => {
                render_bulk_actions_placeholder(theme)
            }
            PreviewModal::ExportExcel => {
                render_export_placeholder(theme)
            }
            PreviewModal::ImportExcel => {
                render_import_placeholder(theme)
            }
            PreviewModal::ImportConfirm { path, conflicts } => {
                render_import_confirm_placeholder(path, conflicts, theme)
            }
        };
        view = view.with_app_modal(modal_element, Alignment::Center);
    }

    view
}

/// Render the main preview content when data is loaded
fn render_preview(
    state: &State,
    resolved: &ResolvedTransfer,
    theme: &Theme,
) -> Element<Msg> {
    if resolved.entities.is_empty() {
        return Element::text("No entities to preview. Add entity mappings in the editor.");
    }

    let entity = &resolved.entities[state.current_entity_idx];

    // Summary line - use owned strings
    let entity_name = entity.entity_name.clone();
    let position = format!(" ({} of {}) | ", state.current_entity_idx + 1, resolved.entities.len());
    let total = format!("{} total", entity.records.len());
    let create = format!("{} create", entity.create_count());
    let update = format!("{} update", entity.update_count());
    let nochange = format!("{} nochange", entity.nochange_count());
    let skip = format!("{} skip", entity.skip_count());
    let error = format!("{} error", entity.error_count());

    let summary = Element::styled_text(Line::from(vec![
        Span::styled("Entity: ", Style::default().fg(theme.text_secondary)),
        Span::styled(entity_name, Style::default().fg(theme.accent_primary)),
        Span::raw(position),
        Span::styled(total, Style::default().fg(theme.text_primary)),
        Span::raw(" | "),
        Span::styled(create, Style::default().fg(theme.accent_success)),
        Span::raw(" | "),
        Span::styled(update, Style::default().fg(theme.accent_secondary)),
        Span::raw(" | "),
        Span::styled(nochange, Style::default().fg(theme.text_secondary)),
        Span::raw(" | "),
        Span::styled(skip, Style::default().fg(theme.accent_warning)),
        Span::raw(" | "),
        Span::styled(error, Style::default().fg(theme.accent_error)),
    ]))
    .build();

    // Filter info with count
    let filtered_count = get_filtered_records(entity, state.filter).len();
    let filter_info = Element::styled_text(Line::from(vec![
        Span::styled("Filter: ", Style::default().fg(theme.text_secondary)),
        Span::styled(state.filter.display_name(), Style::default().fg(theme.accent_primary)),
        Span::styled(
            format!(" ({} shown)", filtered_count),
            Style::default().fg(theme.text_secondary),
        ),
        Span::raw(" | Press [f] to cycle"),
    ]))
    .build();

    // Table header
    let header = render_table_header(entity, theme);

    // Table content (list of records)
    let table = render_record_table(state, entity, theme);

    ColumnBuilder::new()
        .add(summary, LayoutConstraint::Length(1))
        .add(filter_info, LayoutConstraint::Length(1))
        .add(Element::text(""), LayoutConstraint::Length(1))
        .add(header, LayoutConstraint::Length(1))
        .add(table, LayoutConstraint::Fill(1))
        .build()
}

/// Get records filtered by the current filter
fn get_filtered_records<'a>(entity: &'a ResolvedEntity, filter: RecordFilter) -> Vec<&'a ResolvedRecord> {
    entity
        .records
        .iter()
        .filter(|r| filter.matches(r.action))
        .collect()
}

/// Render table header row
fn render_table_header(entity: &ResolvedEntity, theme: &Theme) -> Element<Msg> {
    let header_style = Style::default()
        .fg(theme.text_secondary)
        .add_modifier(Modifier::BOLD);

    // Build header columns: Action | Source ID | field1 | field2 | ...
    let mut header_parts = vec![
        Span::styled(format!("{:<10}", "Action"), header_style),
        Span::raw(" │ "),
        Span::styled(format!("{:<12}", "Source ID"), header_style),
    ];

    // Add field columns (limit to first 5 fields to fit screen)
    let max_fields = 5;
    for field in entity.field_names.iter().take(max_fields) {
        header_parts.push(Span::raw(" │ "));
        header_parts.push(Span::styled(
            format!("{:<15}", truncate_str(field, 15)),
            header_style,
        ));
    }

    if entity.field_names.len() > max_fields {
        header_parts.push(Span::raw(" │ "));
        header_parts.push(Span::styled("...", header_style));
    }

    Element::styled_text(Line::from(header_parts))
        .background(Style::default().bg(theme.bg_surface))
        .build()
}

/// Render the record table as a list with virtual scrolling
fn render_record_table(state: &State, entity: &ResolvedEntity, theme: &Theme) -> Element<Msg> {
    let filtered_records = get_filtered_records(entity, state.filter);
    let total_count = filtered_records.len();

    if total_count == 0 {
        return Element::styled_text(Line::from(vec![
            Span::styled("No records match the current filter.", Style::default().fg(theme.text_secondary)),
        ]))
        .build();
    }

    // Virtual scrolling: only create items for visible range
    // No buffer needed - the scroll_off in ListState handles keeping selection visible
    let scroll_offset = state.list_state.scroll_offset();
    let viewport = state.viewport_height;

    let start_idx = scroll_offset;
    let end_idx = (scroll_offset + viewport).min(total_count);

    // Create list items ONLY for visible range
    let visible_items: Vec<RecordListItem> = filtered_records[start_idx..end_idx]
        .iter()
        .enumerate()
        .map(|(i, record)| {
            let global_idx = start_idx + i;
            RecordListItem {
                record: (*record).clone(),
                field_names: entity.field_names.clone(),
                is_dirty: entity.is_dirty(record.source_id),
                theme: theme.clone(),
                global_index: global_idx,
            }
        })
        .collect();

    // Selection index relative to our slice
    let adjusted_selected = state.list_state.selected().and_then(|sel| {
        if sel >= start_idx && sel < end_idx {
            Some(sel - start_idx)
        } else {
            None
        }
    });

    // Create a windowed list state - scroll_offset is 0 because we already sliced
    let mut windowed_state = crate::tui::widgets::ListState::new().with_scroll_off(0);
    windowed_state.select(adjusted_selected);

    Element::list(
        FocusId::new("record-table"),
        &visible_items,
        &windowed_state,
        theme,
    )
    .on_navigate(|key| Msg::ListEvent(ListEvent::Navigate(key)))
    .on_activate(|_idx| Msg::ViewDetails)
    .on_render(Msg::ViewportHeightChanged)
    // scroll_offset is 0 - we've already sliced to the exact visible range
    .build()
}

/// A list item representing a single record row
struct RecordListItem {
    record: ResolvedRecord,
    field_names: Vec<String>,
    is_dirty: bool,
    theme: Theme,
    global_index: usize, // Index in the full filtered list (for virtual scrolling)
}

impl ListItem for RecordListItem {
    type Msg = Msg;

    fn to_element(&self, is_selected: bool, _is_hovered: bool) -> Element<Self::Msg> {
        let base_style = self.get_row_style(is_selected);

        // Build the row: Action | Source ID (truncated) | field values
        let mut spans = vec![
            self.action_span(),
            Span::styled(" │ ", base_style),
            self.source_id_span(base_style),
        ];

        // Add field values (limit to first 5)
        let max_fields = 5;
        for field in self.field_names.iter().take(max_fields) {
            spans.push(Span::styled(" │ ", base_style));
            spans.push(self.field_value_span(field, base_style));
        }

        if self.field_names.len() > max_fields {
            spans.push(Span::styled(" │ ...", base_style));
        }

        // For error records, append the error message
        if self.record.action == RecordAction::Error {
            if let Some(ref err) = self.record.error {
                spans.push(Span::styled(
                    format!(" ⚠ {}", truncate_str(err, 40)),
                    Style::default().fg(self.theme.accent_error),
                ));
            }
        }

        Element::styled_text(Line::from(spans))
            .background(if is_selected {
                Style::default().bg(self.theme.bg_surface)
            } else {
                Style::default()
            })
            .build()
    }
}

impl RecordListItem {
    /// Get the base style for this row based on action type
    fn get_row_style(&self, is_selected: bool) -> Style {
        let bg = if is_selected {
            self.theme.bg_surface
        } else {
            self.theme.bg_base
        };

        match self.record.action {
            RecordAction::Create => Style::default().fg(self.theme.text_primary).bg(bg),
            RecordAction::Update => Style::default().fg(self.theme.text_primary).bg(bg),
            RecordAction::NoChange => Style::default().fg(self.theme.text_tertiary).bg(bg),
            RecordAction::Skip => Style::default()
                .fg(self.theme.accent_warning)
                .add_modifier(Modifier::DIM)
                .bg(bg),
            RecordAction::Error => Style::default().fg(self.theme.accent_error).bg(bg),
        }
    }

    /// Render the action column with appropriate color
    fn action_span(&self) -> Span<'static> {
        let (text, color) = match self.record.action {
            RecordAction::Create => ("create    ", self.theme.accent_success),
            RecordAction::Update => ("update    ", self.theme.accent_secondary),
            RecordAction::NoChange => ("nochange  ", self.theme.text_tertiary),
            RecordAction::Skip => ("skip      ", self.theme.accent_warning),
            RecordAction::Error => ("error     ", self.theme.accent_error),
        };
        Span::styled(text.to_string(), Style::default().fg(color))
    }

    /// Render the source ID column (truncated UUID)
    fn source_id_span(&self, base_style: Style) -> Span<'static> {
        let short_id = format!("{:.12}", self.record.source_id.to_string());
        Span::styled(short_id, base_style)
    }

    /// Render a field value column
    fn field_value_span(&self, field: &str, base_style: Style) -> Span<'static> {
        let value_str = self
            .record
            .fields
            .get(field)
            .map(|v| format_value(v))
            .unwrap_or_else(|| "(null)".to_string());

        Span::styled(
            format!("{:<15}", truncate_str(&value_str, 15)),
            base_style,
        )
    }
}

/// Format a Value for display in the table
fn format_value(value: &Value) -> String {
    match value {
        Value::Null => "(null)".to_string(),
        Value::String(s) => s.clone(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => format!("{:.2}", f),
        Value::Bool(b) => b.to_string(),
        Value::DateTime(dt) => dt.format("%Y-%m-%d").to_string(),
        Value::Guid(g) => format!("{:.8}...", g),
        Value::OptionSet(n) => n.to_string(),
        Value::Dynamic(dv) => format!("{:?}", dv),
    }
}

/// Truncate a string to max length with ellipsis (UTF-8 safe)
fn truncate_str(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else {
        // Take max_len - 1 chars and append ellipsis
        let truncated: String = s.chars().take(max_len - 1).collect();
        format!("{}…", truncated)
    }
}

// Placeholder modal renderers - will be implemented in later chunks

fn render_record_details_placeholder(record_idx: usize, theme: &Theme) -> Element<Msg> {
    let content = Element::text(&format!(
        "Record Details (index: {})\n\nPress Esc to close.",
        record_idx
    ));

    Element::panel(content)
        .title("Record Details")
        .width(60)
        .height(20)
        .build()
}

fn render_edit_record_placeholder(record_idx: usize, theme: &Theme) -> Element<Msg> {
    let content = Element::text(&format!(
        "Edit Record (index: {})\n\nPress Esc to close.",
        record_idx
    ));

    Element::panel(content)
        .title("Edit Record")
        .width(60)
        .height(20)
        .build()
}

fn render_bulk_actions_placeholder(theme: &Theme) -> Element<Msg> {
    let content = Element::text("Bulk Actions\n\nPress Esc to close.");

    Element::panel(content)
        .title("Bulk Actions")
        .width(50)
        .height(15)
        .build()
}

fn render_export_placeholder(theme: &Theme) -> Element<Msg> {
    let content = Element::text("Export to Excel\n\nPress Esc to close.");

    Element::panel(content)
        .title("Export Excel")
        .width(60)
        .height(20)
        .build()
}

fn render_import_placeholder(theme: &Theme) -> Element<Msg> {
    let content = Element::text("Import from Excel\n\nPress Esc to close.");

    Element::panel(content)
        .title("Import Excel")
        .width(60)
        .height(20)
        .build()
}

fn render_import_confirm_placeholder(path: &str, conflicts: &[String], theme: &Theme) -> Element<Msg> {
    let content = Element::text(&format!(
        "Import Confirmation\n\nPath: {}\nConflicts: {}\n\nPress Esc to close.",
        path,
        conflicts.len()
    ));

    Element::panel(content)
        .title("Confirm Import")
        .width(60)
        .height(20)
        .build()
}

/// Build subscriptions for keyboard shortcuts
pub fn subscriptions(state: &State) -> Vec<Subscription<Msg>> {
    let mut subs = vec![];

    // Modal-specific subscriptions
    if state.active_modal.is_some() {
        subs.push(Subscription::keyboard(KeyCode::Esc, "Close", Msg::CloseModal));
        return subs;
    }

    // Main view subscriptions
    subs.push(Subscription::keyboard(KeyCode::Esc, "Back to editor", Msg::Back));

    // Entity navigation
    subs.push(Subscription::keyboard(KeyCode::Tab, "Next entity", Msg::NextEntity));
    subs.push(Subscription::shift_key(KeyCode::BackTab, "Previous entity", Msg::PrevEntity));

    // Filtering
    subs.push(Subscription::keyboard(KeyCode::Char('f'), "Cycle filter", Msg::CycleFilter));

    // Record actions
    subs.push(Subscription::keyboard(KeyCode::Enter, "View details", Msg::ViewDetails));
    subs.push(Subscription::keyboard(KeyCode::Char('e'), "Edit record", Msg::EditRecord));
    subs.push(Subscription::keyboard(KeyCode::Char('s'), "Toggle skip", Msg::ToggleSkip));

    // Bulk actions
    subs.push(Subscription::keyboard(KeyCode::Char('b'), "Bulk actions", Msg::OpenBulkActions));

    // Refresh
    subs.push(Subscription::keyboard(KeyCode::Char('r'), "Refresh", Msg::Refresh));

    // Excel
    subs.push(Subscription::keyboard(KeyCode::Char('x'), "Export Excel", Msg::ExportExcel));
    subs.push(Subscription::keyboard(KeyCode::Char('i'), "Import Excel", Msg::ImportExcel));

    // Navigation to execute
    subs.push(Subscription::keyboard(KeyCode::Right, "Go to execute", Msg::GoToExecute));

    subs
}
