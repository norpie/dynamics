use ratatui::{Frame, style::Style, widgets::{Block, Borders, Paragraph}, layout::Rect};
use crossterm::event::{KeyCode, KeyEvent};
use crate::tui::element::FocusId;
use crate::tui::command::DispatchTarget;
use crate::tui::renderer::{InteractionRegistry, FocusRegistry, FocusableInfo};

/// Create on_key handler for checkboxes (Enter or Space toggles)
pub fn checkbox_on_key<Msg: Clone + Send + 'static>(on_toggle: Option<Msg>) -> Box<dyn Fn(KeyEvent) -> DispatchTarget<Msg> + Send> {
    Box::new(move |key_event| match key_event.code {
        KeyCode::Enter | KeyCode::Char(' ') => {
            if let Some(msg) = on_toggle.clone() {
                DispatchTarget::AppMsg(msg)
            } else {
                DispatchTarget::PassThrough
            }
        }
        _ => DispatchTarget::PassThrough
    })
}

/// Render Checkbox element
pub fn render_checkbox<Msg: Clone + Send + 'static>(
    frame: &mut Frame,
    registry: &mut InteractionRegistry<Msg>,
    focus_registry: &mut FocusRegistry<Msg>,
    focused_id: Option<&FocusId>,
    id: &FocusId,
    label: &str,
    checked: bool,
    on_toggle: &Option<Msg>,
    on_focus: &Option<Msg>,
    on_blur: &Option<Msg>,
    area: Rect,
    inside_panel: bool,
) {
    let theme = &crate::global_runtime_config().theme;

    // Register in focus registry
    focus_registry.register_focusable(FocusableInfo {
        id: id.clone(),
        rect: area,
        on_key: checkbox_on_key(on_toggle.clone()),
        on_focus: on_focus.clone(),
        on_blur: on_blur.clone(),
        inside_panel,
    });

    // Register click handler
    if let Some(msg) = on_toggle {
        registry.register_click(area, msg.clone());
    }

    // Check if this checkbox is focused
    let is_focused = focused_id == Some(id);

    // Build checkbox display: [x] Label or [ ] Label
    let checkbox_char = if checked { "x" } else { " " };
    let display_text = format!("[{}] {}", checkbox_char, label);

    // Style based on focus and checked state
    let text_style = if checked {
        Style::default().fg(theme.accent_primary)
    } else {
        Style::default().fg(theme.text_primary)
    };

    let border_style = if is_focused {
        Style::default().fg(theme.accent_primary)
    } else {
        Style::default().fg(theme.border_secondary)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);

    let widget = Paragraph::new(display_text)
        .block(block)
        .style(text_style);

    frame.render_widget(widget, area);
}
