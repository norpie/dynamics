//! Step 3: Analysis View
//!
//! Loading screen with per-entity progress while fetching schemas and records.

use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};

use crate::tui::element::Element;
use crate::tui::state::theme::Theme;
use crate::{button_row, col, row, spacer, use_constraints};

use super::super::msg::Msg;
use super::super::state::{AnalysisPhase, State};
use super::super::{FetchStatus, get_analysis_progress};

/// Render the analysis step (loading screen)
pub fn render_analysis(state: &mut State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let header = render_step_header(theme);
    let content = render_progress(state, theme);
    let footer = render_step_footer(state, theme);

    col![
        header => Length(3),
        content => Fill(1),
        footer => Length(5),
    ]
}

/// Render step header
fn render_step_header(theme: &Theme) -> Element<Msg> {
    use_constraints!();

    col![
        Element::styled_text(Line::from(Span::styled(
            "Step 3: Analyzing Environments".to_string(),
            Style::default().fg(theme.accent_primary).bold()
        ))).build() => Length(1),
        Element::text("Fetching schemas and building sync plan...") => Length(1),
        spacer!() => Length(1),
    ]
}

/// Render progress display
fn render_progress(state: &State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let analysis = &state.analysis;

    // Get real-time progress from global state
    let progress = get_analysis_progress();

    // Spinner animation
    let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner_idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        / 100) as usize
        % spinner_chars.len();
    let spinner = if analysis.phase != AnalysisPhase::Complete {
        spinner_chars[spinner_idx]
    } else {
        "✓"
    };

    // Overall phase line
    let phase_text = if progress.overall_phase.is_empty() {
        "Starting analysis...".to_string()
    } else {
        progress.overall_phase.clone()
    };

    let phase_line = Element::styled_text(Line::from(Span::styled(
        format!("{} {}", spinner, phase_text),
        Style::default().fg(theme.accent_primary).bold(),
    )))
    .build();

    // Build entity progress lines
    let mut entity_lines: Vec<Element<Msg>> = Vec::new();

    // Header row
    entity_lines.push(
        Element::styled_text(Line::from(vec![Span::styled(
            format!(
                "{:<30} {:^10} {:^15} {:^12} {:^8}",
                "Entity", "Schema", "Records", "Refs", "N:N"
            ),
            Style::default().fg(theme.text_secondary).bold(),
        )]))
        .build(),
    );

    // Entity rows
    for entity_name in &progress.entity_order {
        if let Some(ep) = progress.entities.get(entity_name) {
            let display = ep.display_name.as_ref().unwrap_or(&ep.entity);
            let display_truncated = if display.len() > 28 {
                format!("{}...", &display[..25])
            } else {
                display.clone()
            };

            let schema_style = status_style(&ep.schema_status, theme);
            let records_style = status_style(&ep.records_status, theme);
            let refs_style = status_style(&ep.refs_status, theme);
            let nn_style = status_style(&ep.nn_status, theme);

            let records_text = match (&ep.records_status, ep.record_count) {
                (FetchStatus::Done, Some(count)) => {
                    format!("{} ({})", ep.records_status.symbol(), count)
                }
                _ => ep.records_status.symbol().to_string(),
            };

            let refs_text = match (&ep.refs_status, ep.refs_count) {
                (FetchStatus::Done, Some(count)) => {
                    format!("{} ({})", ep.refs_status.symbol(), count)
                }
                _ => ep.refs_status.symbol().to_string(),
            };

            // N:N status: show "—" for non-junction entities (Pending means not applicable)
            let nn_text = match &ep.nn_status {
                FetchStatus::Pending => "—".to_string(),
                status => status.symbol().to_string(),
            };

            entity_lines.push(
                Element::styled_text(Line::from(vec![
                    Span::styled(
                        format!("{:<30} ", display_truncated),
                        Style::default().fg(theme.text_primary),
                    ),
                    Span::styled(format!("{:^10} ", ep.schema_status.symbol()), schema_style),
                    Span::styled(format!("{:^15} ", records_text), records_style),
                    Span::styled(format!("{:^12} ", refs_text), refs_style),
                    Span::styled(format!("{:^8}", nn_text), nn_style),
                ]))
                .build(),
            );
        }
    }

    // Calculate counts
    let total = progress.entities.len();
    let schemas_done = progress
        .entities
        .values()
        .filter(|e| matches!(e.schema_status, FetchStatus::Done))
        .count();
    let records_done = progress
        .entities
        .values()
        .filter(|e| matches!(e.records_status, FetchStatus::Done))
        .count();
    let refs_done = progress
        .entities
        .values()
        .filter(|e| matches!(e.refs_status, FetchStatus::Done))
        .count();

    let summary_line = Element::styled_text(Line::from(Span::styled(
        format!(
            "Schemas: {}/{} | Records: {}/{} | Refs: {}/{}",
            schemas_done, total, records_done, total, refs_done, total
        ),
        Style::default().fg(theme.text_secondary),
    )))
    .build();

    // Wrap entity lines in a scrollable column
    let entity_list = Element::column(entity_lines).build();

    col![
        phase_line => Length(1),
        spacer!() => Length(1),
        summary_line => Length(1),
        spacer!() => Length(1),
        entity_list => Fill(1),
    ]
}

/// Get style for a fetch status
fn status_style(status: &FetchStatus, theme: &Theme) -> Style {
    match status {
        FetchStatus::Pending => Style::default().fg(theme.text_tertiary),
        FetchStatus::Fetching => Style::default().fg(theme.accent_info),
        FetchStatus::Done => Style::default().fg(theme.accent_success),
        FetchStatus::Failed(_) => Style::default().fg(theme.accent_error),
    }
}

/// Get status icon and style for a phase
fn get_phase_status(
    phase: AnalysisPhase,
    current_phase: AnalysisPhase,
    theme: &Theme,
) -> (&'static str, Style) {
    let phase_order = |p: AnalysisPhase| -> u8 {
        match p {
            AnalysisPhase::FetchingOriginSchema => 0,
            AnalysisPhase::FetchingTargetSchema => 1,
            AnalysisPhase::FetchingRecordCounts => 2,
            AnalysisPhase::BuildingDependencyGraph => 3,
            AnalysisPhase::DetectingJunctions => 4,
            AnalysisPhase::ComputingDiff => 5,
            AnalysisPhase::Complete => 6,
        }
    };

    let phase_num = phase_order(phase);
    let current_num = phase_order(current_phase);

    if phase_num < current_num {
        ("✓", Style::default().fg(theme.accent_success))
    } else if phase_num == current_num {
        ("→", Style::default().fg(theme.accent_info).bold())
    } else {
        ("○", Style::default().fg(theme.text_tertiary))
    }
}

/// Render step footer
fn render_step_footer(state: &State, theme: &Theme) -> Element<Msg> {
    use_constraints!();

    let status = if state.analysis.phase == AnalysisPhase::Complete {
        Element::text("Analysis complete")
    } else {
        Element::text("")
    };

    let buttons = if state.analysis.phase == AnalysisPhase::Complete {
        button_row![
            ("analysis-back-btn", "Back", Msg::Back),
            ("analysis-next-btn", "Review Diff", Msg::Next),
        ]
    } else {
        button_row![("analysis-cancel-btn", "Cancel", Msg::CancelAnalysis),]
    };

    col![
        status => Length(1),
        spacer!() => Length(1),
        buttons => Length(3),
    ]
}
