//! View rendering for the Transfer Preview app

use crossterm::event::KeyCode;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::transfer::{
    LookupBindingContext, OperationFilter, RecordAction, ResolvedEntity, ResolvedRecord,
    ResolvedTransfer, Value,
};
use crate::tui::element::{ColumnBuilder, FocusId, RowBuilder};
use crate::tui::resource::Resource;
use crate::tui::widgets::{ListEvent, ListItem, TextInputEvent};
use crate::tui::{Alignment, Element, LayeredView, LayoutConstraint, Subscription, Theme};

use super::modals;
use super::state::{BulkAction, BulkActionScope, Msg, PreviewModal, RecordFilter, State};

/// Render the preview app view
pub fn render(state: &mut State, theme: &Theme) -> LayeredView<Msg> {
    let content = match &state.resolved {
        Resource::NotAsked => Element::text("No data loaded. Press Back to return to editor."),
        Resource::Loading => Element::text("Loading resolved records..."),
        Resource::Failure(err) => Element::styled_text(Line::from(vec![
            Span::styled("Error: ", Style::default().fg(theme.accent_error)),
            Span::styled(err.clone(), Style::default().fg(theme.text_primary)),
        ]))
        .build(),
        Resource::Success(resolved) => render_preview(state, resolved, theme),
    };

    let mut view = LayeredView::new(content);

    // Render modals
    if let Some(modal) = &state.active_modal {
        let modal_element = match modal {
            PreviewModal::RecordDetails { record_idx }
            | PreviewModal::EditRecord { record_idx } => {
                // Both RecordDetails and EditRecord use the same modal with different modes
                if let (Some(detail_state), Resource::Success(resolved)) =
                    (&state.record_detail_state, &state.resolved)
                {
                    if let Some(entity) = resolved.entities.get(state.current_entity_idx) {
                        // Get the actual record from filtered list
                        let filtered: Vec<_> = entity
                            .records
                            .iter()
                            .filter(|r| state.filter.matches(r.action))
                            .filter(|r| {
                                let query = state.search_field.value().to_lowercase();
                                if query.is_empty() {
                                    return true;
                                }
                                if r.source_id.to_string().to_lowercase().contains(&query) {
                                    return true;
                                }
                                r.fields
                                    .values()
                                    .any(|v| format_value(v).to_lowercase().contains(&query))
                            })
                            .collect();

                        if let Some(record) = filtered.get(*record_idx) {
                            modals::record_details::render(
                                detail_state,
                                record,
                                entity.lookup_context.as_ref(),
                                theme,
                            )
                        } else {
                            render_record_details_placeholder(*record_idx, theme)
                        }
                    } else {
                        render_record_details_placeholder(*record_idx, theme)
                    }
                } else {
                    render_record_details_placeholder(*record_idx, theme)
                }
            }
            PreviewModal::BulkActions => modals::bulk_actions::render(state, theme),
            PreviewModal::ExportExcel => modals::export::render(state, theme),
            PreviewModal::ImportExcel => modals::import::render_file_browser(state, theme),
            PreviewModal::ImportConfirm { path, conflicts } => {
                modals::import::render_confirmation(state, path, conflicts, theme)
            }
            PreviewModal::SendToQueue => {
                if let Resource::Success(ref resolved) = state.resolved {
                    modals::send_to_queue::render(resolved, theme)
                } else {
                    Element::text("No data to send")
                }
            }
        };
        view = view.with_app_modal(modal_element, Alignment::Center);
    }

    view
}

/// Render the main preview content when data is loaded
fn render_preview(state: &State, resolved: &ResolvedTransfer, theme: &Theme) -> Element<Msg> {
    if resolved.entities.is_empty() {
        return Element::text("No entities to preview. Add entity mappings in the editor.");
    }

    let entity = &resolved.entities[state.current_entity_idx];

    // Search input panel
    let search_input = Element::text_input(
        FocusId::new("search-input"),
        state.search_field.value(),
        &state.search_field.state,
    )
    .on_event(|e| Msg::SearchChanged(e))
    .placeholder("Search records...")
    .build();

    let search_panel = Element::panel(search_input).title("Search").build();

    // Table header
    let header = render_table_header(state, entity, theme);

    // Table content (list of records)
    let table = render_record_table(state, entity, theme);

    // Table panel wrapping header + records
    let table_content = ColumnBuilder::new()
        .add(header, LayoutConstraint::Length(1))
        .add(table, LayoutConstraint::Fill(1))
        .build();

    let table_panel = Element::panel(table_content).title("Records").build();

    ColumnBuilder::new()
        .add(search_panel, LayoutConstraint::Length(3))
        .add(table_panel, LayoutConstraint::Fill(1))
        .build()
}

/// Get records filtered by the current filter and search query
fn get_filtered_records<'a>(
    entity: &'a ResolvedEntity,
    filter: RecordFilter,
    search_query: &str,
) -> Vec<&'a ResolvedRecord> {
    let query = search_query.to_lowercase();
    entity
        .records
        .iter()
        .filter(|r| filter.matches(r.action))
        .filter(|r| {
            if query.is_empty() {
                return true;
            }
            // Search in source_id
            if r.source_id.to_string().to_lowercase().contains(&query) {
                return true;
            }
            // Search in field values
            r.fields
                .values()
                .any(|v| format_value(v).to_lowercase().contains(&query))
        })
        .collect()
}

/// Render table header row
fn render_table_header(state: &State, entity: &ResolvedEntity, theme: &Theme) -> Element<Msg> {
    log::trace!(
        "render_table_header: column_widths={:?}, terminal_width={}, horizontal_scroll={}",
        state.column_widths,
        state.terminal_width,
        state.horizontal_scroll
    );

    let header_style = Style::default()
        .fg(theme.text_secondary)
        .add_modifier(Modifier::BOLD);

    // Build header columns: [scroll indicator] [checkbox] Action | Source ID | field1 | field2 | ... [scroll indicator]
    let mut header_parts = vec![];

    // Left scroll indicator
    if state.has_columns_left() {
        header_parts.push(Span::styled(
            "◀ ",
            Style::default().fg(theme.accent_secondary),
        ));
    } else {
        header_parts.push(Span::styled("  ", header_style));
    }

    header_parts.push(Span::styled("    ", header_style)); // Space for checkbox [✓] or [ ]
    header_parts.push(Span::styled(format!("{:<10}", "Action"), header_style));
    header_parts.push(Span::raw(" │ "));
    header_parts.push(Span::styled(format!("{:<36}", "Source ID"), header_style));

    // Get visible column range
    let visible_range = state.visible_column_range(entity.field_names.len());
    log::trace!(
        "render_table_header: visible_range={:?} for {} fields",
        visible_range,
        entity.field_names.len()
    );

    // Add field columns based on visible range and calculated widths
    for i in visible_range.clone() {
        let field = &entity.field_names[i];
        let width = state.column_widths.get(i).copied().unwrap_or(15);
        header_parts.push(Span::raw(" │ "));
        header_parts.push(Span::styled(
            format!("{:<width$}", truncate_str(field, width), width = width),
            header_style,
        ));
    }

    // Right scroll indicator
    if state.has_columns_right(entity.field_names.len()) {
        header_parts.push(Span::styled(
            " ▶",
            Style::default().fg(theme.accent_secondary),
        ));
    }

    Element::styled_text(Line::from(header_parts))
        .background(Style::default().bg(theme.bg_surface))
        .build()
}

/// Render the record table as a list with virtual scrolling
fn render_record_table(state: &State, entity: &ResolvedEntity, theme: &Theme) -> Element<Msg> {
    let filtered_records = get_filtered_records(entity, state.filter, state.search_field.value());
    let total_count = filtered_records.len();

    if total_count == 0 {
        return Element::styled_text(Line::from(vec![Span::styled(
            "No records match the current filter or search.",
            Style::default().fg(theme.text_secondary),
        )]))
        .build();
    }

    // Virtual scrolling: only create items for visible range
    // No buffer needed - the scroll_off in ListState handles keeping selection visible
    let scroll_offset = state.list_state.scroll_offset();
    let viewport = state.viewport_height;

    let start_idx = scroll_offset;
    let end_idx = (scroll_offset + viewport).min(total_count);

    // Get visible column range for horizontal scrolling
    let visible_range = state.visible_column_range(entity.field_names.len());
    let has_columns_left = state.has_columns_left();
    let has_columns_right = state.has_columns_right(entity.field_names.len());

    // Create list items ONLY for visible range
    let visible_items: Vec<RecordListItem> = filtered_records[start_idx..end_idx]
        .iter()
        .enumerate()
        .map(|(i, record)| {
            let global_idx = start_idx + i;
            RecordListItem {
                record: (*record).clone(),
                field_names: entity.field_names.clone(),
                column_widths: state.column_widths.clone(),
                visible_range: visible_range.clone(),
                has_columns_left,
                has_columns_right,
                is_dirty: entity.is_dirty(record.source_id),
                theme: theme.clone(),
                global_index: global_idx,
                lookup_context: entity.lookup_context.clone(),
                operation_filter: entity.operation_filter,
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
    // Map multi-selection indices from global to windowed space
    let windowed_multi = state
        .list_state
        .windowed_multi_selection(start_idx, end_idx - start_idx);
    let mut windowed_state = crate::tui::widgets::ListState::new().with_scroll_off(0);
    windowed_state.select(adjusted_selected);
    windowed_state.set_multi_selected(windowed_multi);

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
    column_widths: Vec<usize>,
    visible_range: std::ops::Range<usize>,
    has_columns_left: bool,
    has_columns_right: bool,
    is_dirty: bool,
    theme: Theme,
    global_index: usize, // Index in the full filtered list (for virtual scrolling)
    lookup_context: Option<LookupBindingContext>, // For showing lookup bind indicators
    operation_filter: OperationFilter, // For showing disabled operations with strikethrough
}

impl ListItem for RecordListItem {
    type Msg = Msg;

    fn to_element(
        &self,
        is_selected: bool,
        is_multi_selected: bool,
        _is_hovered: bool,
    ) -> Element<Self::Msg> {
        let base_style = self.get_row_style(is_selected);

        // Build the row: [scroll indicator] [checkbox] Action | Source ID (truncated) | field values [scroll indicator]
        let mut spans = vec![];

        // Left scroll indicator (matches header)
        if self.has_columns_left {
            spans.push(Span::styled(
                "◀ ",
                Style::default().fg(self.theme.accent_secondary),
            ));
        } else {
            spans.push(Span::styled("  ", base_style));
        }

        let checkbox = if is_multi_selected {
            Span::styled("[✓] ", Style::default().fg(self.theme.accent_primary))
        } else {
            Span::styled("[ ] ", Style::default().fg(self.theme.text_tertiary))
        };

        spans.push(checkbox);
        spans.push(self.action_span());
        spans.push(Span::styled(" │ ", base_style));
        spans.push(self.source_id_span(base_style));

        // Add field values based on visible range and calculated widths
        for i in self.visible_range.clone() {
            let field = &self.field_names[i];
            let width = self.column_widths.get(i).copied().unwrap_or(15);
            spans.push(Span::styled(" │ ", base_style));
            spans.push(self.field_value_span_with_width(field, width, base_style));
        }

        // Right scroll indicator (matches header)
        if self.has_columns_right {
            spans.push(Span::styled(
                " ▶",
                Style::default().fg(self.theme.accent_secondary),
            ));
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
    /// Check if this record's operation is disabled by the entity's operation filter
    fn is_operation_disabled(&self) -> bool {
        match self.record.action {
            RecordAction::Create => !self.operation_filter.creates,
            RecordAction::Update => !self.operation_filter.updates,
            RecordAction::TargetOnly => !self.operation_filter.has_orphan_action(),
            _ => false, // NoChange, Skip, Error are not affected by operation filter
        }
    }

    /// Get the base style for this row based on action type
    fn get_row_style(&self, is_selected: bool) -> Style {
        let bg = if is_selected {
            self.theme.bg_surface
        } else {
            self.theme.bg_base
        };

        // If operation is disabled by filter, use gray + strikethrough
        if self.is_operation_disabled() {
            return Style::default()
                .fg(self.theme.text_tertiary)
                .add_modifier(Modifier::CROSSED_OUT)
                .bg(bg);
        }

        match self.record.action {
            RecordAction::Create => Style::default().fg(self.theme.text_primary).bg(bg),
            RecordAction::Update => Style::default().fg(self.theme.text_primary).bg(bg),
            RecordAction::Delete => Style::default().fg(self.theme.accent_error).bg(bg),
            RecordAction::Deactivate => Style::default().fg(self.theme.accent_warning).bg(bg),
            RecordAction::NoChange => Style::default().fg(self.theme.text_tertiary).bg(bg),
            RecordAction::TargetOnly => Style::default().fg(self.theme.text_tertiary).bg(bg),
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
            RecordAction::Delete => ("delete    ", self.theme.accent_error),
            RecordAction::Deactivate => ("deactivate", self.theme.accent_warning),
            RecordAction::NoChange => ("nochange  ", self.theme.text_tertiary),
            RecordAction::TargetOnly => ("targetonly", self.theme.text_tertiary),
            RecordAction::Skip => ("skip      ", self.theme.accent_warning),
            RecordAction::Error => ("error     ", self.theme.accent_error),
        };

        // If operation is disabled by filter, use gray + strikethrough
        if self.is_operation_disabled() {
            return Span::styled(
                text.to_string(),
                Style::default()
                    .fg(self.theme.text_tertiary)
                    .add_modifier(Modifier::CROSSED_OUT),
            );
        }

        Span::styled(text.to_string(), Style::default().fg(color))
    }

    /// Render the source ID column (full UUID)
    fn source_id_span(&self, base_style: Style) -> Span<'static> {
        let id = self.record.source_id.to_string();
        Span::styled(format!("{:<36}", id), base_style)
    }

    /// Render a field value column with dynamic width
    fn field_value_span_with_width(
        &self,
        field: &str,
        width: usize,
        base_style: Style,
    ) -> Span<'static> {
        let value = self.record.fields.get(field);

        // Check if this field is a lookup that will be bound
        let is_bound_lookup = self
            .lookup_context
            .as_ref()
            .map_or(false, |ctx| ctx.is_lookup(field));

        // For bound lookups with GUID values, show a special indicator
        if is_bound_lookup {
            if let Some(Value::Guid(guid)) = value {
                // Get the target entity set name for display
                let target = self
                    .lookup_context
                    .as_ref()
                    .and_then(|ctx| ctx.get(field))
                    .map(|info| info.target_entity_set.as_str())
                    .unwrap_or("?");

                // Show as "→entity(guid)" - use full guid, truncate_str will handle overflow
                let display = format!("→{}({})", target, guid);
                return Span::styled(
                    format!("{:<width$}", truncate_str(&display, width), width = width),
                    Style::default().fg(self.theme.accent_secondary),
                );
            } else if let Some(Value::String(s)) = value {
                // Check if it's a GUID string
                if uuid::Uuid::parse_str(s).is_ok() {
                    let target = self
                        .lookup_context
                        .as_ref()
                        .and_then(|ctx| ctx.get(field))
                        .map(|info| info.target_entity_set.as_str())
                        .unwrap_or("?");

                    let display = format!("→{}({})", target, s);
                    return Span::styled(
                        format!("{:<width$}", truncate_str(&display, width), width = width),
                        Style::default().fg(self.theme.accent_secondary),
                    );
                }
            }
        }

        let value_str = value
            .map(|v| format_value(v))
            .unwrap_or_else(|| "(null)".to_string());

        Span::styled(
            format!("{:<width$}", truncate_str(&value_str, width), width = width),
            base_style,
        )
    }
}

/// Format a Value for display in the table
fn format_value(value: &Value) -> String {
    match value {
        Value::Null => "(null)".to_string(),
        Value::String(s) => sanitize_for_display(s),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => format!("{:.2}", f),
        Value::Bool(b) => b.to_string(),
        Value::DateTime(dt) => dt.format("%Y-%m-%d").to_string(),
        Value::Guid(g) => format!("{:.8}...", g),
        Value::OptionSet(n) => n.to_string(),
        Value::Dynamic(dv) => sanitize_for_display(&format!("{:?}", dv)),
    }
}

/// Sanitize a string for display in the table by replacing control characters
pub(super) fn sanitize_for_display(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\n' => '↵',                // Newline indicator
            '\r' => ' ',                // Carriage return
            '\t' => '→',                // Tab indicator
            c if c.is_control() => '·', // Other control characters
            c => c,
        })
        .collect()
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

/// Build subscriptions for keyboard shortcuts
pub fn subscriptions(state: &State) -> Vec<Subscription<Msg>> {
    let mut subs = vec![];

    // Record details modal subscriptions
    if let Some(ref detail) = state.record_detail_state {
        if detail.editing {
            if detail.editing_field {
                // Actively editing a field value - text input captures most keys
                subs.push(Subscription::keyboard(
                    KeyCode::Esc,
                    "Cancel field edit",
                    Msg::CancelFieldEdit,
                ));
                subs.push(Subscription::keyboard(
                    KeyCode::Enter,
                    "Finish editing",
                    Msg::FinishFieldEdit,
                ));
            } else {
                // Edit mode - navigating fields
                subs.push(Subscription::keyboard(
                    KeyCode::Esc,
                    "Cancel edits",
                    Msg::CancelRecordEdits,
                ));
                subs.push(Subscription::ctrl_key(
                    KeyCode::Char('s'),
                    "Save",
                    Msg::SaveRecordEdits,
                ));
                subs.push(Subscription::keyboard(
                    KeyCode::Up,
                    "Previous field",
                    Msg::RecordDetailFieldNavigate(KeyCode::Up),
                ));
                subs.push(Subscription::keyboard(
                    KeyCode::Down,
                    "Next field",
                    Msg::RecordDetailFieldNavigate(KeyCode::Down),
                ));
                subs.push(Subscription::keyboard(
                    KeyCode::Enter,
                    "Edit field value",
                    Msg::StartFieldEdit,
                ));

                // Action cycling with number keys
                subs.push(Subscription::keyboard(
                    KeyCode::Char('1'),
                    "Set Create",
                    Msg::RecordDetailActionChanged(RecordAction::Create),
                ));
                subs.push(Subscription::keyboard(
                    KeyCode::Char('2'),
                    "Set Update",
                    Msg::RecordDetailActionChanged(RecordAction::Update),
                ));
                subs.push(Subscription::keyboard(
                    KeyCode::Char('3'),
                    "Set Skip",
                    Msg::RecordDetailActionChanged(RecordAction::Skip),
                ));
                subs.push(Subscription::keyboard(
                    KeyCode::Char('4'),
                    "Set NoChange",
                    Msg::RecordDetailActionChanged(RecordAction::NoChange),
                ));
            }
        } else {
            // View mode subscriptions
            subs.push(Subscription::keyboard(
                KeyCode::Esc,
                "Close modal",
                Msg::CloseModal,
            ));
            subs.push(Subscription::keyboard(
                KeyCode::Char('e'),
                "Edit",
                Msg::ToggleEditMode,
            ));
            subs.push(Subscription::keyboard(
                KeyCode::Enter,
                "Edit",
                Msg::ToggleEditMode,
            ));
        }
        return subs;
    }

    // Bulk actions modal subscriptions
    if let Some(PreviewModal::BulkActions) = &state.active_modal {
        subs.push(Subscription::keyboard(
            KeyCode::Esc,
            "Cancel",
            Msg::CloseModal,
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Enter,
            "Apply",
            Msg::ConfirmBulkAction,
        ));

        // Scope selection (1/2/3)
        subs.push(Subscription::keyboard(
            KeyCode::Char('1'),
            "Filtered scope",
            Msg::SetBulkActionScope(BulkActionScope::Filtered),
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Char('2'),
            "All scope",
            Msg::SetBulkActionScope(BulkActionScope::All),
        ));
        if state.list_state.has_multi_selection() {
            subs.push(Subscription::keyboard(
                KeyCode::Char('3'),
                "Selected scope",
                Msg::SetBulkActionScope(BulkActionScope::Selected),
            ));
        }

        // Action selection (a/b/c)
        subs.push(Subscription::keyboard(
            KeyCode::Char('a'),
            "Mark Skip",
            Msg::SetBulkAction(BulkAction::MarkSkip),
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Char('b'),
            "Unmark Skip",
            Msg::SetBulkAction(BulkAction::UnmarkSkip),
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Char('c'),
            "Reset to Original",
            Msg::SetBulkAction(BulkAction::ResetToOriginal),
        ));

        return subs;
    }

    // Export modal subscriptions
    if let Some(PreviewModal::ExportExcel) = &state.active_modal {
        subs.push(Subscription::keyboard(
            KeyCode::Esc,
            "Cancel",
            Msg::CloseModal,
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Up,
            "Navigate up",
            Msg::ExportFileNavigate(KeyCode::Up),
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Down,
            "Navigate down",
            Msg::ExportFileNavigate(KeyCode::Down),
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Enter,
            "Enter directory / Confirm",
            Msg::ExportFileNavigate(KeyCode::Enter),
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Backspace,
            "Go up directory",
            Msg::ExportFileNavigate(KeyCode::Backspace),
        ));
        subs.push(Subscription::ctrl_key(
            KeyCode::Enter,
            "Export",
            Msg::ConfirmExport,
        ));
        return subs;
    }

    // Import file browser modal subscriptions
    if let Some(PreviewModal::ImportExcel) = &state.active_modal {
        subs.push(Subscription::keyboard(
            KeyCode::Esc,
            "Cancel",
            Msg::CloseModal,
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Up,
            "Navigate up",
            Msg::ImportFileNavigate(KeyCode::Up),
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Down,
            "Navigate down",
            Msg::ImportFileNavigate(KeyCode::Down),
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Enter,
            "Select file / Enter directory",
            Msg::ImportFileNavigate(KeyCode::Enter),
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Backspace,
            "Go up directory",
            Msg::ImportFileNavigate(KeyCode::Backspace),
        ));
        return subs;
    }

    // Import confirmation modal subscriptions
    if let Some(PreviewModal::ImportConfirm { .. }) = &state.active_modal {
        subs.push(Subscription::keyboard(
            KeyCode::Esc,
            "Cancel",
            Msg::CancelImport,
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Enter,
            "Confirm import",
            Msg::ConfirmImport,
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Char('y'),
            "Confirm import",
            Msg::ConfirmImport,
        ));
        subs.push(Subscription::keyboard(
            KeyCode::Char('n'),
            "Cancel",
            Msg::CancelImport,
        ));
        return subs;
    }

    // Send to Queue modal subscriptions
    if let Some(PreviewModal::SendToQueue) = &state.active_modal {
        subs.push(Subscription::keyboard(
            KeyCode::Esc,
            "Cancel",
            Msg::CloseModal,
        ));
        return subs;
    }

    // Other modal subscriptions
    if state.active_modal.is_some() {
        subs.push(Subscription::keyboard(
            KeyCode::Esc,
            "Close modal",
            Msg::CloseModal,
        ));
        return subs;
    }

    // Main view subscriptions

    // Entity navigation (] and [ to cycle through entities)
    subs.push(Subscription::keyboard(
        KeyCode::Char(']'),
        "Next entity",
        Msg::NextEntity,
    ));
    subs.push(Subscription::keyboard(
        KeyCode::Char('['),
        "Previous entity",
        Msg::PrevEntity,
    ));

    // Filtering
    subs.push(Subscription::keyboard(
        KeyCode::Char('f'),
        "Cycle filter",
        Msg::CycleFilter,
    ));

    // Horizontal scrolling (columns)
    subs.push(Subscription::keyboard(
        KeyCode::Left,
        "Scroll left",
        Msg::ScrollLeft,
    ));
    subs.push(Subscription::keyboard(
        KeyCode::Right,
        "Scroll right",
        Msg::ScrollRight,
    ));

    // Record actions
    subs.push(Subscription::keyboard(
        KeyCode::Enter,
        "View details",
        Msg::ViewDetails,
    ));
    subs.push(Subscription::keyboard(
        KeyCode::Char('e'),
        "Edit record",
        Msg::EditRecord,
    ));
    subs.push(Subscription::keyboard(
        KeyCode::Char('s'),
        "Toggle skip",
        Msg::ToggleSkip,
    ));

    // Multi-selection
    subs.push(Subscription::keyboard(
        KeyCode::Char(' '),
        "Toggle selection",
        Msg::ListMultiSelect(ListEvent::ToggleMultiSelect),
    ));
    subs.push(Subscription::keyboard(
        KeyCode::Char('*'),
        "Select all",
        Msg::ListMultiSelect(ListEvent::SelectAll),
    ));
    subs.push(Subscription::keyboard(
        KeyCode::Char('-'),
        "Clear selection",
        Msg::ListMultiSelect(ListEvent::ClearMultiSelection),
    ));
    subs.push(Subscription::shift_key(
        KeyCode::Up,
        "Extend selection up",
        Msg::ListMultiSelect(ListEvent::ExtendSelectionUp),
    ));
    subs.push(Subscription::shift_key(
        KeyCode::Down,
        "Extend selection down",
        Msg::ListMultiSelect(ListEvent::ExtendSelectionDown),
    ));

    // Bulk actions
    subs.push(Subscription::keyboard(
        KeyCode::Char('b'),
        "Bulk actions",
        Msg::OpenBulkActions,
    ));

    // Refresh
    subs.push(Subscription::keyboard(
        KeyCode::Char('r'),
        "Refresh",
        Msg::Refresh,
    ));

    // Excel
    subs.push(Subscription::keyboard(
        KeyCode::Char('x'),
        "Export Excel",
        Msg::ExportExcel,
    ));
    subs.push(Subscription::keyboard(
        KeyCode::Char('i'),
        "Import Excel",
        Msg::ImportExcel,
    ));

    // Send to Queue
    subs.push(Subscription::keyboard(
        KeyCode::Char('q'),
        "Send to queue",
        Msg::OpenSendToQueue,
    ));

    subs
}
