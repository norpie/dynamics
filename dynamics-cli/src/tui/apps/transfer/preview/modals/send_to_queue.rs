//! Send to Queue confirmation modal

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::transfer::ResolvedTransfer;
use crate::tui::element::{ColumnBuilder, FocusId, RowBuilder};
use crate::tui::{Element, LayoutConstraint, Theme};

use super::super::state::Msg;

/// Render the send to queue confirmation modal
pub fn render(resolved: &ResolvedTransfer, theme: &Theme) -> Element<Msg> {
    let create_count = resolved.create_count();
    let update_count = resolved.update_count();
    let skip_count = resolved.skip_count();
    let nochange_count = resolved.nochange_count();
    let total_actionable = create_count + update_count;

    // Summary by entity
    let mut entity_lines: Vec<Element<Msg>> = vec![];
    for entity in &resolved.entities {
        let entity_creates = entity.create_count();
        let entity_updates = entity.update_count();
        let entity_total = entity_creates + entity_updates;

        if entity_total > 0 {
            let line = Element::styled_text(Line::from(vec![
                Span::styled(
                    format!("  {:<20}", entity.entity_name),
                    Style::default().fg(theme.text_primary),
                ),
                Span::styled(
                    format!("{} create", entity_creates),
                    Style::default().fg(theme.accent_success),
                ),
                Span::raw(" + "),
                Span::styled(
                    format!("{} update", entity_updates),
                    Style::default().fg(theme.accent_secondary),
                ),
            ]))
            .build();
            entity_lines.push(line);
        }
    }

    // Total summary
    let total_line = Element::styled_text(Line::from(vec![
        Span::styled(
            format!("Total: {} operations", total_actionable),
            Style::default()
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ("),
        Span::styled(format!("{} create", create_count), Style::default().fg(theme.accent_success)),
        Span::raw(", "),
        Span::styled(format!("{} update", update_count), Style::default().fg(theme.accent_secondary)),
        Span::raw(")"),
    ]))
    .build();

    // Skipped info
    let skipped_line = Element::styled_text(Line::from(vec![
        Span::styled(
            format!("Skipped: {} records", skip_count),
            Style::default().fg(theme.accent_warning),
        ),
        Span::raw(", "),
        Span::styled(
            format!("Unchanged: {} records", nochange_count),
            Style::default().fg(theme.text_tertiary),
        ),
    ]))
    .build();

    // Target environment
    let target_line = Element::styled_text(Line::from(vec![
        Span::raw("Target: "),
        Span::styled(
            resolved.target_env.clone(),
            Style::default()
                .fg(theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .build();

    // Buttons
    let cancel_btn = Element::button(FocusId::new("queue-cancel"), "Cancel")
        .on_press(Msg::CloseModal)
        .build();

    let confirm_btn = Element::button(FocusId::new("queue-confirm"), "Send to Queue")
        .on_press(Msg::ConfirmSendToQueue)
        .build();

    let button_row = RowBuilder::new()
        .add(cancel_btn, LayoutConstraint::Length(12))
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(confirm_btn, LayoutConstraint::Length(16))
        .build();

    // Build content
    let mut builder = ColumnBuilder::new();

    // Add entity lines
    for entity_line in entity_lines {
        builder = builder.add(entity_line, LayoutConstraint::Length(1));
    }

    let content = builder
        .add(Element::text(""), LayoutConstraint::Length(1)) // Spacer
        .add(total_line, LayoutConstraint::Length(1))
        .add(skipped_line, LayoutConstraint::Length(1))
        .add(Element::text(""), LayoutConstraint::Length(1)) // Spacer
        .add(target_line, LayoutConstraint::Length(1))
        .add(Element::text(""), LayoutConstraint::Fill(1)) // Spacer
        .add(button_row, LayoutConstraint::Length(3))
        .build();

    Element::panel(content)
        .title("Send to Queue")
        .width(50)
        .height((14 + resolved.entities.len().min(5)) as u16)
        .build()
}
