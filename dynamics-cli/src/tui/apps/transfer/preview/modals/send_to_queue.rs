//! Send to Queue confirmation modal

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::transfer::ResolvedTransfer;
use crate::tui::element::{ColumnBuilder, FocusId, RowBuilder};
use crate::tui::{Element, LayoutConstraint, Theme};

use super::super::state::Msg;

/// Render the send to queue confirmation modal
pub fn render(resolved: &ResolvedTransfer, theme: &Theme) -> Element<Msg> {
    // Calculate counts per entity and totals
    let mut total_creates = 0usize;
    let mut total_updates = 0usize;
    let mut total_deletes = 0usize;
    let mut total_deactivates = 0usize;
    let mut total_disabled = 0usize;

    let mut entity_lines: Vec<Element<Msg>> = vec![];

    for entity in &resolved.entities {
        let raw_creates = entity.create_count();
        let raw_updates = entity.update_count();
        let raw_deletes = entity.delete_count();
        let raw_deactivates = entity.deactivate_count();

        // Skip entities with no operations
        if raw_creates == 0 && raw_updates == 0 && raw_deletes == 0 && raw_deactivates == 0 {
            continue;
        }

        // Calculate filtered counts based on operation filter
        let (filtered_creates, disabled_creates) = if entity.operation_filter.creates {
            (raw_creates, 0)
        } else {
            (0, raw_creates)
        };
        let (filtered_updates, disabled_updates) = if entity.operation_filter.updates {
            (raw_updates, 0)
        } else {
            (0, raw_updates)
        };
        // Deletes and deactivates are always enabled if they exist (they're created based on the filter)
        let filtered_deletes = raw_deletes;
        let filtered_deactivates = raw_deactivates;

        total_creates += filtered_creates;
        total_updates += filtered_updates;
        total_deletes += filtered_deletes;
        total_deactivates += filtered_deactivates;
        total_disabled += disabled_creates + disabled_updates;

        // Entity name line
        let entity_header = Element::styled_text(Line::from(vec![Span::styled(
            format!("  {}", entity.entity_name),
            Style::default()
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        )]))
        .build();
        entity_lines.push(entity_header);

        // Create line
        if raw_creates > 0 {
            let create_line = if entity.operation_filter.creates {
                Element::styled_text(Line::from(vec![
                    Span::styled("    - ", Style::default().fg(theme.text_tertiary)),
                    Span::styled(
                        format!("{} records to create", filtered_creates),
                        Style::default().fg(theme.accent_success),
                    ),
                ]))
                .build()
            } else {
                Element::styled_text(Line::from(vec![
                    Span::styled("    - ", Style::default().fg(theme.text_tertiary)),
                    Span::styled(
                        format!("{} records to create (disabled)", raw_creates),
                        Style::default()
                            .fg(theme.text_tertiary)
                            .add_modifier(Modifier::CROSSED_OUT),
                    ),
                ]))
                .build()
            };
            entity_lines.push(create_line);
        }

        // Update line
        if raw_updates > 0 {
            let update_line = if entity.operation_filter.updates {
                Element::styled_text(Line::from(vec![
                    Span::styled("    - ", Style::default().fg(theme.text_tertiary)),
                    Span::styled(
                        format!("{} records to update", filtered_updates),
                        Style::default().fg(theme.accent_secondary),
                    ),
                ]))
                .build()
            } else {
                Element::styled_text(Line::from(vec![
                    Span::styled("    - ", Style::default().fg(theme.text_tertiary)),
                    Span::styled(
                        format!("{} records to update (disabled)", raw_updates),
                        Style::default()
                            .fg(theme.text_tertiary)
                            .add_modifier(Modifier::CROSSED_OUT),
                    ),
                ]))
                .build()
            };
            entity_lines.push(update_line);
        }

        // Delete line
        if raw_deletes > 0 {
            let delete_line = Element::styled_text(Line::from(vec![
                Span::styled("    - ", Style::default().fg(theme.text_tertiary)),
                Span::styled(
                    format!("{} records to delete", filtered_deletes),
                    Style::default().fg(theme.accent_error),
                ),
            ]))
            .build();
            entity_lines.push(delete_line);
        }

        // Deactivate line
        if raw_deactivates > 0 {
            let deactivate_line = Element::styled_text(Line::from(vec![
                Span::styled("    - ", Style::default().fg(theme.text_tertiary)),
                Span::styled(
                    format!("{} records to deactivate", filtered_deactivates),
                    Style::default().fg(theme.accent_warning),
                ),
            ]))
            .build();
            entity_lines.push(deactivate_line);
        }
    }

    let total_actionable = total_creates + total_updates + total_deletes + total_deactivates;

    // Summary section
    let summary_header = Element::styled_text(Line::from(vec![Span::styled(
        "Summary",
        Style::default()
            .fg(theme.text_primary)
            .add_modifier(Modifier::BOLD),
    )]))
    .build();

    let total_line = Element::styled_text(Line::from(vec![
        Span::styled("  - ", Style::default().fg(theme.text_tertiary)),
        Span::styled(
            format!("{} operations to execute", total_actionable),
            Style::default().fg(theme.text_primary),
        ),
    ]))
    .build();

    // Not queued section (only if there's something to show)
    let skip_count = resolved.skip_count();
    let nochange_count = resolved.nochange_count();
    let has_not_queued = skip_count > 0 || nochange_count > 0 || total_disabled > 0;

    // Target environment
    let target_line = Element::styled_text(Line::from(vec![
        Span::styled("  - Target: ", Style::default().fg(theme.text_tertiary)),
        Span::styled(
            resolved.target_env.clone(),
            Style::default().fg(theme.accent_primary),
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
    for entity_line in &entity_lines {
        builder = builder.add(entity_line.clone(), LayoutConstraint::Length(1));
    }

    // Add spacer and summary
    builder = builder
        .add(Element::text(""), LayoutConstraint::Length(1))
        .add(summary_header, LayoutConstraint::Length(1))
        .add(total_line, LayoutConstraint::Length(1))
        .add(target_line, LayoutConstraint::Length(1));

    // Add "not queued" info if applicable
    if has_not_queued {
        builder = builder.add(Element::text(""), LayoutConstraint::Length(1));

        let not_queued_header = Element::styled_text(Line::from(vec![Span::styled(
            "Not queued",
            Style::default()
                .fg(theme.text_secondary)
                .add_modifier(Modifier::BOLD),
        )]))
        .build();
        builder = builder.add(not_queued_header, LayoutConstraint::Length(1));

        if nochange_count > 0 {
            let nochange_line = Element::styled_text(Line::from(vec![
                Span::styled("  - ", Style::default().fg(theme.text_tertiary)),
                Span::styled(
                    format!("{} unchanged", nochange_count),
                    Style::default().fg(theme.text_tertiary),
                ),
            ]))
            .build();
            builder = builder.add(nochange_line, LayoutConstraint::Length(1));
        }

        if skip_count > 0 {
            let skip_line = Element::styled_text(Line::from(vec![
                Span::styled("  - ", Style::default().fg(theme.text_tertiary)),
                Span::styled(
                    format!("{} skipped", skip_count),
                    Style::default().fg(theme.accent_warning),
                ),
            ]))
            .build();
            builder = builder.add(skip_line, LayoutConstraint::Length(1));
        }

        if total_disabled > 0 {
            let disabled_line = Element::styled_text(Line::from(vec![
                Span::styled("  - ", Style::default().fg(theme.text_tertiary)),
                Span::styled(
                    format!("{} disabled by filter", total_disabled),
                    Style::default()
                        .fg(theme.text_tertiary)
                        .add_modifier(Modifier::CROSSED_OUT),
                ),
            ]))
            .build();
            builder = builder.add(disabled_line, LayoutConstraint::Length(1));
        }
    }

    // Add spacer and buttons
    let content = builder
        .add(Element::text(""), LayoutConstraint::Fill(1))
        .add(button_row, LayoutConstraint::Length(3))
        .build();

    // Calculate dynamic height based on content
    let entity_line_count = entity_lines.len();
    let not_queued_line_count = if has_not_queued {
        1 + (if nochange_count > 0 { 1 } else { 0 })
            + (if skip_count > 0 { 1 } else { 0 })
            + (if total_disabled > 0 { 1 } else { 0 })
    } else {
        0
    };
    let base_height = 10; // Panel chrome + summary + buttons
    let content_height = entity_line_count + not_queued_line_count;
    let height = (base_height + content_height).min(25) as u16;

    Element::panel(content)
        .title("Send to Queue")
        .width(45)
        .height(height)
        .build()
}
