use crate::tui::{Element, Theme};
use ratatui::{Frame, layout::Rect, style::Style, widgets::Paragraph};

/// Render primitive elements (None, Text, StyledText)
pub fn render_primitive<Msg: Clone + Send + 'static>(
    frame: &mut Frame,

    element: &Element<Msg>,
    area: Rect,
) {
    let theme = &crate::global_runtime_config().theme;
    match element {
        Element::None => {}

        Element::Text { content, style } => {
            let default_style = Style::default().fg(theme.text_primary);
            let widget = Paragraph::new(content.as_str()).style(style.unwrap_or(default_style));
            frame.render_widget(widget, area);
        }

        Element::StyledText { line, background } => {
            let mut widget = Paragraph::new(line.clone());
            if let Some(bg_style) = background {
                widget = widget.style(*bg_style);
            }
            frame.render_widget(widget, area);
        }

        _ => unreachable!("render_primitive called with non-primitive element"),
    }
}

/// Check if an element is a primitive
pub fn is_primitive<Msg>(element: &Element<Msg>) -> bool {
    matches!(
        element,
        Element::None | Element::Text { .. } | Element::StyledText { .. }
    )
}
