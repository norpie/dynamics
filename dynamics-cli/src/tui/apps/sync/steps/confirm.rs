//! Step 5: Confirm & Execute View
//!
//! Summary display, confirmation checkbox, and execution progress.

use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};

use crate::tui::element::Element;
use crate::tui::widgets::ListItem;
use crate::tui::state::theme::Theme;
use crate::{col, row, spacer, use_constraints, button_row};

use super::super::state::State;
use super::super::msg::Msg;
use super::super::logic::operation_builder::{build_operation_summary, OperationSummary};

/// Summary list item for the confirm view
#[derive(Clone)]
struct SummaryItem {
    label: String,
    value: String,
    style: SummaryStyle,
}

#[derive(Clone, Copy)]
enum SummaryStyle {
    Normal,
    Warning,
    Info,
}

impl ListItem for SummaryItem {
    type Msg = Msg;

    fn to_element(&self, is_focused: bool, _is_hovered: bool) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;

        let text = format!("{}: {}", self.label, self.value);

        let style = match self.style {
            SummaryStyle::Normal => Style::default().fg(theme.text_primary),
            SummaryStyle::Warning => Style::default().fg(theme.accent_warning),
            SummaryStyle::Info => Style::default().fg(theme.accent_info),
        };

        let bg_style = if is_focused {
            Style::default().bg(theme.bg_surface)
        } else {
            Style::default()
        };

        Element::styled_text(Line::from(Span::styled(text, style)))
            .background(bg_style)
            .build()
    }
}

/// Render the confirm & execute step
pub fn render_confirm(state: &mut State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let header = render_step_header(state, theme);
    let content = if state.confirm.executing {
        render_execution_progress(state, theme)
    } else {
        render_summary(state, theme)
    };
    let footer = render_step_footer(state, theme);

    col![
        header => Length(3),
        content => Fill(1),
        footer => Length(5),
    ]
}

/// Render step header
fn render_step_header(state: &State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let origin = state.env_select.origin_env.as_deref().unwrap_or("?");
    let target = state.env_select.target_env.as_deref().unwrap_or("?");

    let title = if state.confirm.executing {
        "Step 5: Executing Sync"
    } else {
        "Step 5: Confirm & Execute"
    };

    let env_text = format!("From: {} -> To: {}", origin, target);

    col![
        Element::styled_text(Line::from(Span::styled(
            title.to_string(),
            Style::default().fg(theme.accent_primary).bold()
        ))).build() => Length(1),
        Element::styled_text(Line::from(Span::styled(
            env_text,
            Style::default().fg(theme.text_secondary)
        ))).build() => Length(1),
        spacer!() => Length(1),
    ]
}

/// Render summary view (before execution)
fn render_summary(state: &mut State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let Some(ref plan) = state.sync_plan else {
        return Element::panel(Element::text("No sync plan available"))
            .title("Error")
            .build();
    };

    let summary = build_operation_summary(plan);

    // Build summary items
    let summary_panel = render_operation_summary(&summary, theme);
    let warnings_panel = render_warnings(&summary, theme);
    let confirmation_panel = render_confirmation(state, theme);

    col![
        row![
            summary_panel => Fill(1),
            warnings_panel => Fill(1),
        ] => Fill(1),
        confirmation_panel => Length(5),
    ]
}

/// Render operation summary panel
fn render_operation_summary(summary: &OperationSummary, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let mut lines: Vec<Element<Msg>> = vec![];

    // Deletes section
    if !summary.entities_with_deletes.is_empty() {
        let total_deletes: usize = summary.entities_with_deletes.iter().map(|(_, c)| c).sum();
        lines.push(Element::styled_text(Line::from(Span::styled(
            format!("Records to DELETE: {}", total_deletes),
            Style::default().fg(theme.accent_error).bold()
        ))).build());

        for (entity, count) in &summary.entities_with_deletes {
            lines.push(Element::text(format!("  {} ({} records)", entity, count)));
        }
        lines.push(Element::text(""));
    }

    // Schema changes section
    if !summary.entities_with_schema_changes.is_empty() {
        let total_schema: usize = summary.entities_with_schema_changes.iter().map(|(_, c)| c).sum();
        lines.push(Element::styled_text(Line::from(Span::styled(
            format!("Fields to ADD: {}", total_schema),
            Style::default().fg(theme.accent_info).bold()
        ))).build());

        for (entity, count) in &summary.entities_with_schema_changes {
            lines.push(Element::text(format!("  {} ({} fields)", entity, count)));
        }
        lines.push(Element::text(""));
    }

    // Inserts section
    if !summary.entities_with_inserts.is_empty() {
        let total_inserts: usize = summary.entities_with_inserts.iter().map(|(_, c)| c).sum();
        lines.push(Element::styled_text(Line::from(Span::styled(
            format!("Records to INSERT: {}", total_inserts),
            Style::default().fg(theme.accent_success).bold()
        ))).build());

        for (entity, count) in &summary.entities_with_inserts {
            lines.push(Element::text(format!("  {} ({} records)", entity, count)));
        }
    }

    let content = Element::column(lines).build();

    Element::panel(content)
        .title("Operations Summary")
        .build()
}

/// Render warnings panel
fn render_warnings(summary: &OperationSummary, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let mut lines: Vec<Element<Msg>> = vec![];

    // Fields needing review
    if !summary.fields_needing_review.is_empty() {
        lines.push(Element::styled_text(Line::from(Span::styled(
            format!("Fields needing manual review: {}", summary.fields_needing_review.len()),
            Style::default().fg(theme.accent_warning).bold()
        ))).build());

        for (entity, field, reason) in summary.fields_needing_review.iter().take(5) {
            lines.push(Element::text(format!("  {}.{}: {}", entity, field, reason)));
        }
        if summary.fields_needing_review.len() > 5 {
            lines.push(Element::text(format!(
                "  ... and {} more",
                summary.fields_needing_review.len() - 5
            )));
        }
        lines.push(Element::text(""));
    }

    // Nulled lookups
    if !summary.lookups_to_null.is_empty() {
        lines.push(Element::styled_text(Line::from(Span::styled(
            format!("Lookups to be NULLED: {}", summary.lookups_to_null.len()),
            Style::default().fg(theme.accent_warning).bold()
        ))).build());

        for (entity, field, target, count) in summary.lookups_to_null.iter().take(5) {
            lines.push(Element::text(format!(
                "  {}.{} -> {} ({} records)",
                entity, field, target, count
            )));
        }
        if summary.lookups_to_null.len() > 5 {
            lines.push(Element::text(format!(
                "  ... and {} more",
                summary.lookups_to_null.len() - 5
            )));
        }
    }

    // No warnings message
    if summary.fields_needing_review.is_empty() && summary.lookups_to_null.is_empty() {
        lines.push(Element::styled_text(Line::from(Span::styled(
            "No warnings - all fields match",
            Style::default().fg(theme.accent_success)
        ))).build());
    }

    let content = Element::column(lines).build();

    Element::panel(content)
        .title("Warnings")
        .build()
}

/// Render confirmation panel
fn render_confirmation(state: &State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let checkbox = if state.confirm.confirmed {
        "[X]"
    } else {
        "[ ]"
    };

    let confirm_text = format!(
        "{} I understand this will DELETE existing records and replace them",
        checkbox
    );

    let confirm_style = if state.confirm.confirmed {
        Style::default().fg(theme.accent_success)
    } else {
        Style::default().fg(theme.text_secondary)
    };

    let export_hint = if state.confirm.export_path.is_some() {
        Element::styled_text(Line::from(Span::styled(
            "Report will be exported before execution",
            Style::default().fg(theme.accent_info)
        ))).build()
    } else {
        Element::text("Press 'e' to export a report before executing")
    };

    let content = Element::column(vec![
        Element::styled_text(Line::from(Span::styled(confirm_text, confirm_style))).build(),
        spacer!(),
        export_hint,
    ]).build();

    Element::panel(content)
        .title("Confirmation")
        .build()
}

/// Render execution progress
fn render_execution_progress(state: &State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let progress = state.confirm.execution_progress;
    let status = &state.confirm.execution_status;

    // Progress bar
    let progress_bar = Element::progress_bar(progress as usize, 100)
        .label(format!("{}%", progress))
        .build();

    // Status message
    let status_text = Element::styled_text(Line::from(Span::styled(
        status.clone(),
        Style::default().fg(theme.text_primary)
    ))).build();

    // Phase indicators
    let phase_lines: Vec<Element<Msg>> = vec![
        render_phase_indicator("Deleting records", progress, 0, 33, theme),
        render_phase_indicator("Adding fields", progress, 33, 66, theme),
        render_phase_indicator("Inserting records", progress, 66, 100, theme),
    ];

    let content = Element::column(vec![
        Element::text(""),
        progress_bar,
        Element::text(""),
        status_text,
        Element::text(""),
        Element::column(phase_lines).build(),
    ]).build();

    Element::panel(content)
        .title("Execution Progress")
        .build()
}

/// Render a phase indicator line
fn render_phase_indicator(
    label: &str,
    progress: u8,
    start: u8,
    end: u8,
    theme: &Theme,
) -> Element<Msg> {
    let (icon, style) = if progress >= end {
        ("Done", Style::default().fg(theme.accent_success))
    } else if progress >= start {
        ("...", Style::default().fg(theme.accent_info).bold())
    } else {
        ("Pending", Style::default().fg(theme.text_tertiary))
    };

    let text = format!("  {} {}", icon, label);

    Element::styled_text(Line::from(Span::styled(text, style))).build()
}

/// Render step footer with navigation
fn render_step_footer(state: &State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    if state.confirm.executing {
        // During execution - no navigation
        let status = Element::text("Sync in progress... please wait");

        col![
            status => Length(1),
            spacer!() => Fill(1),
        ]
    } else {
        // Before execution
        let status = if state.confirm.confirmed {
            Element::styled_text(Line::from(Span::styled(
                "Ready to execute",
                Style::default().fg(theme.accent_success)
            ))).build()
        } else {
            Element::styled_text(Line::from(Span::styled(
                "Check the confirmation box to enable execution",
                Style::default().fg(theme.accent_warning)
            ))).build()
        };

        let buttons = button_row![
            ("confirm-back-btn", "Back", Msg::Back),
            ("confirm-export-btn", "Export", Msg::ExportReport),
            ("confirm-exec-btn", "Execute", Msg::Execute),
        ];

        col![
            status => Length(1),
            spacer!() => Length(1),
            buttons => Length(3),
        ]
    }
}
