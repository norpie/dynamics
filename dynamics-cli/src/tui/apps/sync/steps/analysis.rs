//! Step 3: Analysis View
//!
//! Loading screen with progress phases while fetching schemas and building dependency graph.

use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};

use crate::tui::element::Element;
use crate::tui::state::theme::Theme;
use crate::{col, spacer, use_constraints, button_row};

use super::super::state::{State, AnalysisPhase};
use super::super::msg::Msg;
use super::super::get_analysis_progress;

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

    // Current status message (from global progress)
    let status_line = if !progress.message.is_empty() {
        Element::styled_text(Line::from(Span::styled(
            progress.message.clone(),
            Style::default().fg(theme.accent_info).bold()
        ))).build()
    } else {
        Element::text("Starting analysis...")
    };

    // Current entity being processed
    let entity_line = match &progress.entity {
        Some(entity) => {
            let text = format!("Entity: {}", entity);
            Element::styled_text(Line::from(Span::styled(
                text,
                Style::default().fg(theme.text_secondary)
            ))).build()
        }
        None => Element::text(""),
    };

    // Current step/phase
    let step_line = match &progress.step {
        Some(step) => {
            let text = format!("Phase: {}", step);
            Element::text(text)
        }
        None => Element::text(""),
    };

    // Spinner animation (simple rotating chars)
    let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner_idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() / 100) as usize % spinner_chars.len();
    let spinner = if analysis.phase != AnalysisPhase::Complete {
        spinner_chars[spinner_idx]
    } else {
        "✓"
    };

    let spinner_line = Element::styled_text(Line::from(Span::styled(
        format!("{} Analyzing...", spinner),
        Style::default().fg(theme.accent_primary)
    ))).build();

    // Build content
    col![
        spacer!() => Fill(1),
        spinner_line => Length(1),
        spacer!() => Length(1),
        status_line => Length(1),
        entity_line => Length(1),
        step_line => Length(1),
        spacer!() => Fill(1),
    ]
}

/// Get status icon and style for a phase
fn get_phase_status(phase: AnalysisPhase, current_phase: AnalysisPhase, theme: &Theme) -> (&'static str, Style) {
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
        button_row![
            ("analysis-cancel-btn", "Cancel", Msg::CancelAnalysis),
        ]
    };

    col![
        status => Length(1),
        spacer!() => Length(1),
        buttons => Length(3),
    ]
}
