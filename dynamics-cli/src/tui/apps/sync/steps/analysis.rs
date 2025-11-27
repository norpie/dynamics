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

    // Phase list with completion status
    let phases = [
        (AnalysisPhase::FetchingOriginSchema, "Fetching origin schema"),
        (AnalysisPhase::FetchingTargetSchema, "Fetching target schema"),
        (AnalysisPhase::FetchingRecordCounts, "Counting records"),
        (AnalysisPhase::BuildingDependencyGraph, "Building dependency graph"),
        (AnalysisPhase::DetectingJunctions, "Detecting junction entities"),
        (AnalysisPhase::ComputingDiff, "Computing schema diff"),
    ];

    let current_phase = analysis.phase;
    let phase_lines: Vec<Element<Msg>> = phases.iter().map(|(phase, label)| {
        let (icon, _style) = get_phase_status(*phase, current_phase, theme);
        let text = format!("{} {}", icon, label);
        Element::text(text)
    }).collect();

    // Current entity being processed
    let current_entity = match &analysis.current_entity {
        Some(entity) => {
            let text = format!("Processing: {}", entity);
            Element::text(text)
        }
        None => Element::text(""),
    };

    // Overall progress bar
    let progress_bar = Element::progress_bar(analysis.progress as usize, 100)
        .label(analysis.status_message.clone())
        .build();

    // Build content
    let mut items = vec![];
    for line in phase_lines {
        items.push((crate::tui::LayoutConstraint::Length(1), line));
    }
    items.push((crate::tui::LayoutConstraint::Length(1), Element::text("")));
    items.push((crate::tui::LayoutConstraint::Length(1), current_entity));
    items.push((crate::tui::LayoutConstraint::Length(1), Element::text("")));
    items.push((crate::tui::LayoutConstraint::Length(1), progress_bar));

    let content = crate::tui::element::ColumnBuilder::from_items(items).build();

    Element::panel(content)
        .title("Progress")
        .build()
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
        Element::text("✓ Analysis complete")
    } else {
        Element::text("Press Esc to cancel")
    };

    let buttons = if state.analysis.phase == AnalysisPhase::Complete {
        button_row![
            ("analysis-back-btn", "Back (Esc)", Msg::Back),
            ("analysis-next-btn", "Review Diff (Enter)", Msg::Next),
        ]
    } else {
        button_row![
            ("analysis-cancel-btn", "Cancel (Esc)", Msg::CancelAnalysis),
        ]
    };

    col![
        status => Length(1),
        spacer!() => Length(1),
        buttons => Length(3),
    ]
}
