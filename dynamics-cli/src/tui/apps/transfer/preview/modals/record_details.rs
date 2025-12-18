//! Record details/edit modal for viewing and editing individual records

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::transfer::{LookupBindingContext, RecordAction, ResolvedRecord};
use crate::tui::element::{ColumnBuilder, FocusId, RowBuilder};
use crate::tui::widgets::TextInputEvent;
use crate::tui::{Element, LayoutConstraint, Theme};

use super::super::state::{Msg, RecordDetailState};
use super::super::view::sanitize_for_display;

/// Render the record details/edit modal
pub fn render(
    state: &RecordDetailState,
    record: &ResolvedRecord,
    lookup_context: Option<&LookupBindingContext>,
    theme: &Theme,
) -> Element<Msg> {
    let content = if state.editing {
        render_edit_mode(state, record, lookup_context, theme)
    } else {
        render_view_mode(state, record, lookup_context, theme)
    };

    let title = if state.editing {
        "Edit Record"
    } else {
        "Record Details"
    };

    Element::panel(content)
        .title(title)
        .width(90)
        .height(35)
        .build()
}

/// Render view mode - read-only display of record
fn render_view_mode(
    state: &RecordDetailState,
    record: &ResolvedRecord,
    lookup_context: Option<&LookupBindingContext>,
    theme: &Theme,
) -> Element<Msg> {
    // Source ID row
    let source_id_row = Element::styled_text(Line::from(vec![
        Span::styled("Source ID: ", Style::default().fg(theme.text_secondary)),
        Span::styled(record.source_id.to_string(), Style::default().fg(theme.text_primary)),
    ]))
    .build();

    // Action row
    let action_row = Element::styled_text(Line::from(vec![
        Span::styled("Action:    ", Style::default().fg(theme.text_secondary)),
        Span::styled(
            format!("{}", record.action),
            Style::default().fg(action_color(record.action, theme)),
        ),
    ]))
    .build();

    // Fields section
    let fields_content = render_fields_view(state, lookup_context, theme);
    let fields_panel = Element::panel(fields_content)
        .title("Fields")
        .build();

    // Error row (if present)
    let error_element = if let Some(ref err) = record.error {
        Element::styled_text(Line::from(vec![
            Span::styled("Error: ", Style::default().fg(theme.accent_error).add_modifier(Modifier::BOLD)),
            Span::styled(err.clone(), Style::default().fg(theme.accent_error)),
        ]))
        .build()
    } else {
        Element::text("")
    };

    // Buttons row
    let buttons = render_view_buttons(theme);

    // Build layout
    ColumnBuilder::new()
        .add(source_id_row, LayoutConstraint::Length(1))
        .add(action_row, LayoutConstraint::Length(1))
        .add(Element::text(""), LayoutConstraint::Length(1))
        .add(fields_panel, LayoutConstraint::Fill(1))
        .add(error_element, LayoutConstraint::Length(1))
        .add(Element::text(""), LayoutConstraint::Length(1))
        .add(buttons, LayoutConstraint::Length(3))
        .build()
}

/// Render edit mode - editable form for record
fn render_edit_mode(
    state: &RecordDetailState,
    record: &ResolvedRecord,
    lookup_context: Option<&LookupBindingContext>,
    theme: &Theme,
) -> Element<Msg> {
    // Source ID row (read-only)
    let source_id_row = Element::styled_text(Line::from(vec![
        Span::styled("Source ID: ", Style::default().fg(theme.text_secondary)),
        Span::styled(record.source_id.to_string(), Style::default().fg(theme.text_primary)),
    ]))
    .build();

    // Action selector row
    let action_row = render_action_selector(state, theme);

    // Fields section (editable)
    let fields_content = render_fields_edit(state, lookup_context, theme);
    let fields_title = if state.editing_field {
        "Fields - Enter: confirm, Esc: cancel"
    } else {
        "Fields - ↑↓: navigate, Enter: edit, 1-4: action"
    };
    let fields_panel = Element::panel(fields_content)
        .title(fields_title)
        .build();

    // Buttons row
    let buttons = render_edit_buttons(state, theme);

    // Build layout
    ColumnBuilder::new()
        .add(source_id_row, LayoutConstraint::Length(1))
        .add(action_row, LayoutConstraint::Length(1))
        .add(Element::text(""), LayoutConstraint::Length(1))
        .add(fields_panel, LayoutConstraint::Fill(1))
        .add(Element::text(""), LayoutConstraint::Length(1))
        .add(buttons, LayoutConstraint::Length(3))
        .build()
}

/// Render the action selector for edit mode
fn render_action_selector(state: &RecordDetailState, theme: &Theme) -> Element<Msg> {
    let actions = RecordDetailState::available_actions();
    let mut spans = vec![Span::styled("Action:    ", Style::default().fg(theme.text_secondary))];

    for (idx, action) in actions.iter().enumerate() {
        let is_selected = *action == state.current_action;
        let color = action_color(*action, theme);
        let style = if is_selected {
            Style::default().fg(color).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(theme.text_tertiary)
        };

        if idx > 0 {
            spans.push(Span::raw(" | "));
        }
        // Show keyboard shortcut number
        spans.push(Span::styled(format!("[{}] ", idx + 1), Style::default().fg(theme.text_tertiary)));
        spans.push(Span::styled(format!("{}", action), style));
    }

    Element::styled_text(Line::from(spans)).build()
}

/// Render fields in view mode (read-only)
fn render_fields_view(
    state: &RecordDetailState,
    lookup_context: Option<&LookupBindingContext>,
    theme: &Theme,
) -> Element<Msg> {
    let field_rows: Vec<Element<Msg>> = state
        .fields
        .iter()
        .map(|field| {
            let is_lookup = lookup_context.map_or(false, |ctx| ctx.is_lookup(&field.field_name));
            let lookup_target = lookup_context
                .and_then(|ctx| ctx.get(&field.field_name))
                .map(|info| info.target_entity_set.as_str());

            let value_display = if field.input.value().is_empty() {
                "(null)".to_string()
            } else if is_lookup && is_guid_string(field.input.value()) {
                // Show lookup binding indicator
                let target = lookup_target.unwrap_or("?");
                format!("→ {}({})", target, truncate_str(field.input.value(), 20))
            } else {
                sanitize_for_display(field.input.value())
            };

            let dirty_indicator = if field.is_dirty {
                Span::styled(" *", Style::default().fg(theme.accent_warning))
            } else {
                Span::raw("")
            };

            // Use different color for lookup-bound fields
            let value_style = if is_lookup && is_guid_string(field.input.value()) {
                Style::default().fg(theme.accent_secondary)
            } else {
                Style::default().fg(theme.text_primary)
            };

            Element::styled_text(Line::from(vec![
                Span::styled(
                    format!("{:<20}", truncate_str(&field.field_name, 20)),
                    Style::default().fg(theme.text_secondary),
                ),
                Span::raw(" │ "),
                Span::styled(
                    truncate_str(&value_display, 40),
                    value_style,
                ),
                dirty_indicator,
            ]))
            .build()
        })
        .collect();

    if field_rows.is_empty() {
        return Element::text("No fields");
    }

    let mut builder = ColumnBuilder::new();
    for row in field_rows {
        builder = builder.add(row, LayoutConstraint::Length(1));
    }
    builder.build()
}

/// Render fields in edit mode
/// - Navigate with Up/Down arrows
/// - Press Enter to edit the focused field
/// - When editing, type to change value, Enter/Tab to confirm, Esc to cancel
fn render_fields_edit(
    state: &RecordDetailState,
    lookup_context: Option<&LookupBindingContext>,
    theme: &Theme,
) -> Element<Msg> {
    let mut builder = ColumnBuilder::new();

    for (idx, field) in state.fields.iter().enumerate() {
        let is_focused = idx == state.focused_field_idx;
        let is_editing = is_focused && state.editing_field;
        let is_lookup = lookup_context.map_or(false, |ctx| ctx.is_lookup(&field.field_name));
        let lookup_target = lookup_context
            .and_then(|ctx| ctx.get(&field.field_name))
            .map(|info| info.target_entity_set.as_str());

        let dirty_indicator = if field.is_dirty { " *" } else { "" };

        // Label with focus indicator and lookup hint
        let label_style = if is_focused {
            Style::default().fg(theme.accent_primary).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_secondary)
        };

        let focus_indicator = if is_focused { "> " } else { "  " };

        // Add lookup indicator to label if it's a lookup field
        let field_label = if is_lookup {
            format!("{}→", truncate_str(&field.field_name, 17))
        } else {
            truncate_str(&field.field_name, 18)
        };

        let label = Element::styled_text(Line::from(vec![
            Span::raw(focus_indicator),
            Span::styled(
                format!("{:<18}", field_label),
                label_style,
            ),
            Span::raw(" │ "),
        ]))
        .build();

        if is_editing {
            // Show text input for the actively edited field
            let placeholder = if is_lookup {
                let target = lookup_target.unwrap_or("entity");
                format!("GUID (→ {})", target)
            } else {
                "(empty)".to_string()
            };

            let input = Element::text_input(
                FocusId::new("field-edit-input"),
                field.input.value(),
                &field.input.state,
            )
            .on_event(focused_field_input_handler)
            .placeholder(&placeholder)
            .build();

            let dirty_span = Element::styled_text(Line::from(vec![
                Span::styled(dirty_indicator, Style::default().fg(theme.accent_warning)),
            ]))
            .build();

            let row = RowBuilder::new()
                .add(label, LayoutConstraint::Length(24))
                .add(input, LayoutConstraint::Fill(1))
                .add(dirty_span, LayoutConstraint::Length(2))
                .build();

            builder = builder.add(row, LayoutConstraint::Length(1));
        } else {
            // Show value as text (sanitized for display)
            let value_display = if field.input.value().is_empty() {
                "(null)".to_string()
            } else if is_lookup && is_guid_string(field.input.value()) {
                let target = lookup_target.unwrap_or("?");
                format!("→ {}({})", target, truncate_str(field.input.value(), 20))
            } else {
                sanitize_for_display(field.input.value())
            };

            let value_style = if is_focused {
                if is_lookup && is_guid_string(field.input.value()) {
                    Style::default().fg(theme.accent_secondary).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text_primary).add_modifier(Modifier::BOLD)
                }
            } else if is_lookup && is_guid_string(field.input.value()) {
                Style::default().fg(theme.accent_secondary)
            } else {
                Style::default().fg(theme.text_primary)
            };

            let row = Element::styled_text(Line::from(vec![
                Span::raw(focus_indicator),
                Span::styled(
                    format!("{:<18}", field_label),
                    label_style,
                ),
                Span::raw(" │ "),
                Span::styled(
                    truncate_str(&value_display, 45),
                    value_style,
                ),
                Span::styled(dirty_indicator, Style::default().fg(theme.accent_warning)),
            ]))
            .build();

            builder = builder.add(row, LayoutConstraint::Length(1));
        }
    }

    if state.fields.is_empty() {
        return Element::text("No fields");
    }

    builder.build()
}

/// Handler function for focused field text input events
fn focused_field_input_handler(event: TextInputEvent) -> Msg {
    Msg::FocusedFieldInput(event)
}

/// Render buttons for view mode
fn render_view_buttons(theme: &Theme) -> Element<Msg> {
    let edit_btn = Element::button(FocusId::new("edit-btn"), "[e] Edit")
        .on_press(Msg::ToggleEditMode)
        .build();

    let close_btn = Element::button(FocusId::new("close-btn"), "[Esc] Close")
        .on_press(Msg::CloseModal)
        .build();

    RowBuilder::new()
        .add(edit_btn, LayoutConstraint::Length(14))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(close_btn, LayoutConstraint::Length(14))
        .build()
}

/// Render buttons for edit mode
fn render_edit_buttons(state: &RecordDetailState, theme: &Theme) -> Element<Msg> {
    let has_changes = state.has_changes();

    let save_btn = if has_changes {
        Element::button(FocusId::new("save-btn"), "[Ctrl+S] Save")
            .on_press(Msg::SaveRecordEdits)
            .build()
    } else {
        Element::button(FocusId::new("save-btn"), "[Ctrl+S] Save").build()
    };

    let cancel_btn = Element::button(FocusId::new("cancel-btn"), "[Esc] Cancel")
        .on_press(Msg::CancelRecordEdits)
        .build();

    RowBuilder::new()
        .add(save_btn, LayoutConstraint::Length(16))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(cancel_btn, LayoutConstraint::Length(16))
        .build()
}

/// Get the color for an action type
fn action_color(action: RecordAction, theme: &Theme) -> ratatui::style::Color {
    match action {
        RecordAction::Create => theme.accent_success,
        RecordAction::Update => theme.accent_secondary,
        RecordAction::NoChange => theme.text_tertiary,
        RecordAction::TargetOnly => theme.accent_primary,
        RecordAction::Skip => theme.accent_warning,
        RecordAction::Error => theme.accent_error,
    }
}

/// Truncate a string to max length with ellipsis
fn truncate_str(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len - 1).collect();
        format!("{}…", truncated)
    }
}

/// Check if a string is a valid GUID
fn is_guid_string(s: &str) -> bool {
    uuid::Uuid::parse_str(s).is_ok()
}
