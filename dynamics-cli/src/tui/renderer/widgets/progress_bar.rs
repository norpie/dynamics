use crate::tui::{Element, Theme};
use ratatui::{
    Frame,
    layout::Rect,
    prelude::Stylize,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

/// Render a progress bar widget
pub fn render_progress_bar<Msg: Clone + Send + 'static>(
    frame: &mut Frame,
    element: &Element<Msg>,
    area: Rect,
    theme: &Theme,
) {
    let (current, total, label, show_percentage, show_count, width) = match element {
        Element::ProgressBar {
            current,
            total,
            label,
            show_percentage,
            show_count,
            width,
        } => (
            *current,
            *total,
            label,
            *show_percentage,
            *show_count,
            *width,
        ),
        _ => unreachable!("render_progress_bar called with non-ProgressBar element"),
    };

    // Calculate percentage (handle division by zero)
    let percentage = if total > 0 {
        ((current as f64 / total as f64) * 100.0) as usize
    } else {
        0
    };

    // Build the status text (right side: "23/47" or "42%" or both)
    let mut status_parts = Vec::new();
    if show_count {
        status_parts.push(format!("{}/{}", current, total));
    }
    if show_percentage {
        status_parts.push(format!("{}%", percentage));
    }
    let status_text = if !status_parts.is_empty() {
        format!(" {}", status_parts.join(" "))
    } else {
        String::new()
    };

    // Calculate available width for bar
    let label_width = label.as_ref().map(|l| l.len() + 1).unwrap_or(0);
    let status_width = status_text.len();
    let total_padding = label_width + status_width;

    if area.width as usize <= total_padding {
        // Not enough space, just render the status
        let fallback_line = Line::from(vec![
            Span::styled(
                label.as_deref().unwrap_or(""),
                Style::default().fg(theme.text_secondary),
            ),
            Span::styled(status_text, Style::default().fg(theme.text_primary)),
        ]);
        frame.render_widget(Paragraph::new(fallback_line), area);
        return;
    }

    let bar_width = width.unwrap_or(area.width - total_padding as u16) as usize;

    // Calculate filled portion
    let filled = if total > 0 {
        ((current as f64 / total as f64) * bar_width as f64) as usize
    } else {
        0
    };
    let empty = bar_width.saturating_sub(filled);

    // Build the progress bar string
    let bar_filled = "█".repeat(filled);
    let bar_empty = "░".repeat(empty);

    // Construct the full line
    let mut spans = Vec::new();

    // Label (if present)
    if let Some(label_text) = label {
        spans.push(Span::styled(
            format!("{} ", label_text),
            Style::default().fg(theme.text_secondary),
        ));
    }

    // Progress bar
    spans.push(Span::styled(
        bar_filled,
        Style::default().fg(theme.accent_success),
    ));
    spans.push(Span::styled(
        bar_empty,
        Style::default().fg(theme.border_primary),
    ));

    // Status text
    if !status_text.is_empty() {
        spans.push(Span::styled(
            status_text,
            Style::default().fg(theme.text_primary),
        ));
    }

    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(line), area);
}
