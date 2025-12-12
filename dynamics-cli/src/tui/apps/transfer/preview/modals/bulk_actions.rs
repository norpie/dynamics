//! Bulk actions modal for applying actions to multiple records

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::tui::element::{ColumnBuilder, FocusId, RowBuilder};
use crate::tui::{Element, LayoutConstraint, Theme};

use super::super::state::{BulkAction, BulkActionScope, Msg, State};

/// Render the bulk actions modal
pub fn render(state: &State, theme: &Theme) -> Element<Msg> {
    // Calculate counts for each scope
    let (all_count, filtered_count, selected_count) = calculate_counts(state);

    let has_multi_selection = state.list_state.has_multi_selection();

    // Title
    let title = Element::styled_text(Line::from(vec![
        Span::styled("Apply action to records", Style::default().fg(theme.text_primary).add_modifier(Modifier::BOLD)),
    ]))
    .build();

    // Scope section
    let scope_header = Element::styled_text(Line::from(vec![
        Span::styled("Scope:", Style::default().fg(theme.text_secondary)),
    ]))
    .build();

    let scope_options = render_scope_options(state, theme, all_count, filtered_count, selected_count, has_multi_selection);

    // Action section
    let action_header = Element::styled_text(Line::from(vec![
        Span::styled("Action:", Style::default().fg(theme.text_secondary)),
    ]))
    .build();

    let action_options = render_action_options(state, theme);

    // Buttons
    let buttons = render_buttons(theme);

    // Build layout
    let content = ColumnBuilder::new()
        .add(title, LayoutConstraint::Length(1))
        .add(Element::text(""), LayoutConstraint::Length(1))
        .add(scope_header, LayoutConstraint::Length(1))
        .add(scope_options, LayoutConstraint::Length(if has_multi_selection { 3 } else { 2 }))
        .add(Element::text(""), LayoutConstraint::Length(1))
        .add(action_header, LayoutConstraint::Length(1))
        .add(action_options, LayoutConstraint::Length(3))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(buttons, LayoutConstraint::Length(3))
        .build();

    Element::panel(content)
        .title("Bulk Actions")
        .width(60)
        .height(18)
        .build()
}

/// Calculate record counts for each scope
fn calculate_counts(state: &State) -> (usize, usize, usize) {
    let (all_count, filtered_count) = if let crate::tui::resource::Resource::Success(resolved) = &state.resolved {
        if let Some(entity) = resolved.entities.get(state.current_entity_idx) {
            let all = entity.records.len();
            let query = state.search_field.value().to_lowercase();
            let filtered = entity.records.iter()
                .filter(|r| state.filter.matches(r.action))
                .filter(|r| {
                    if query.is_empty() { return true; }
                    if r.source_id.to_string().to_lowercase().contains(&query) { return true; }
                    r.fields.values().any(|v| format!("{:?}", v).to_lowercase().contains(&query))
                })
                .count();
            (all, filtered)
        } else {
            (0, 0)
        }
    } else {
        (0, 0)
    };

    let selected_count = state.list_state.multi_select_count();

    (all_count, filtered_count, selected_count)
}

/// Render scope options as radio buttons
fn render_scope_options(
    state: &State,
    theme: &Theme,
    all_count: usize,
    filtered_count: usize,
    selected_count: usize,
    has_multi_selection: bool,
) -> Element<Msg> {
    let mut builder = ColumnBuilder::new();

    // Filtered option
    let filtered_selected = state.bulk_action_scope == BulkActionScope::Filtered;
    let filtered_radio = if filtered_selected { "(*)" } else { "( )" };
    builder = builder.add(
        Element::styled_text(Line::from(vec![
            Span::styled("[1] ", Style::default().fg(theme.text_tertiary)),
            Span::styled(filtered_radio, Style::default().fg(if filtered_selected { theme.accent_primary } else { theme.text_tertiary })),
            Span::styled(
                format!(" Filtered ({} records)", filtered_count),
                Style::default().fg(if filtered_selected { theme.text_primary } else { theme.text_secondary }),
            ),
        ]))
        .build(),
        LayoutConstraint::Length(1),
    );

    // All option
    let all_selected = state.bulk_action_scope == BulkActionScope::All;
    let all_radio = if all_selected { "(*)" } else { "( )" };
    builder = builder.add(
        Element::styled_text(Line::from(vec![
            Span::styled("[2] ", Style::default().fg(theme.text_tertiary)),
            Span::styled(all_radio, Style::default().fg(if all_selected { theme.accent_primary } else { theme.text_tertiary })),
            Span::styled(
                format!(" All ({} records)", all_count),
                Style::default().fg(if all_selected { theme.text_primary } else { theme.text_secondary }),
            ),
        ]))
        .build(),
        LayoutConstraint::Length(1),
    );

    // Selected option (only show if multi-selection is active)
    if has_multi_selection {
        let selected_selected = state.bulk_action_scope == BulkActionScope::Selected;
        let selected_radio = if selected_selected { "(*)" } else { "( )" };
        builder = builder.add(
            Element::styled_text(Line::from(vec![
                Span::styled("[3] ", Style::default().fg(theme.text_tertiary)),
                Span::styled(selected_radio, Style::default().fg(if selected_selected { theme.accent_primary } else { theme.text_tertiary })),
                Span::styled(
                    format!(" Selected ({} records)", selected_count),
                    Style::default().fg(if selected_selected { theme.text_primary } else { theme.text_secondary }),
                ),
            ]))
            .build(),
            LayoutConstraint::Length(1),
        );
    }

    builder.build()
}

/// Render action options
fn render_action_options(state: &State, theme: &Theme) -> Element<Msg> {
    let mut builder = ColumnBuilder::new();

    let actions = [
        ('a', BulkAction::MarkSkip),
        ('b', BulkAction::UnmarkSkip),
        ('c', BulkAction::ResetToOriginal),
    ];

    for (key, action) in actions {
        let is_selected = state.bulk_action_selection == action;
        let radio = if is_selected { "(*)" } else { "( )" };

        builder = builder.add(
            Element::styled_text(Line::from(vec![
                Span::styled(format!("[{}] ", key), Style::default().fg(theme.text_tertiary)),
                Span::styled(radio, Style::default().fg(if is_selected { theme.accent_primary } else { theme.text_tertiary })),
                Span::styled(
                    format!(" {}", action.display_name()),
                    Style::default().fg(if is_selected { theme.text_primary } else { theme.text_secondary }),
                ),
            ]))
            .build(),
            LayoutConstraint::Length(1),
        );
    }

    builder.build()
}

/// Render Apply and Cancel buttons
fn render_buttons(theme: &Theme) -> Element<Msg> {
    let apply_btn = Element::button(FocusId::new("apply-btn"), "[Enter] Apply")
        .on_press(Msg::ConfirmBulkAction)
        .build();

    let cancel_btn = Element::button(FocusId::new("cancel-btn"), "[Esc] Cancel")
        .on_press(Msg::CloseModal)
        .build();

    RowBuilder::new()
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(apply_btn, LayoutConstraint::Length(16))
        .add(Element::text(""), LayoutConstraint::Length(2))
        .add(cancel_btn, LayoutConstraint::Length(14))
        .build()
}
