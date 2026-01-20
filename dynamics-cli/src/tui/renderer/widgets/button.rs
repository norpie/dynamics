use crate::tui::command::DispatchTarget;
use crate::tui::element::FocusId;
use crate::tui::renderer::{FocusRegistry, FocusableInfo, InteractionRegistry};
use crate::tui::{Element, Theme};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    widgets::{Block, Borders, Paragraph},
};

/// Create on_key handler for buttons (Enter or Space activates)
/// Buttons don't support auto-dispatch since they have no Field type
pub fn button_on_key<Msg: Clone + Send + 'static>(
    on_press: Option<Msg>,
) -> Box<dyn Fn(KeyEvent) -> DispatchTarget<Msg> + Send> {
    Box::new(move |key_event| match key_event.code {
        KeyCode::Enter | KeyCode::Char(' ') => {
            if let Some(msg) = on_press.clone() {
                DispatchTarget::AppMsg(msg)
            } else {
                // Button has no callback - pass through to global subscriptions
                DispatchTarget::PassThrough
            }
        }
        _ => {
            // Unhandled key - pass through to global subscriptions
            DispatchTarget::PassThrough
        }
    })
}

/// Render Button element
pub fn render_button<Msg: Clone + Send + 'static>(
    frame: &mut Frame,

    registry: &mut InteractionRegistry<Msg>,
    focus_registry: &mut FocusRegistry<Msg>,
    focused_id: Option<&FocusId>,
    id: &FocusId,
    label: &str,
    on_press: &Option<Msg>,
    on_hover: &Option<Msg>,
    on_hover_exit: &Option<Msg>,
    on_focus: &Option<Msg>,
    on_blur: &Option<Msg>,
    style: &Option<Style>,
    area: Rect,
    inside_panel: bool,
) {
    let theme = &crate::global_runtime_config().theme;
    // Register in focus registry
    focus_registry.register_focusable(FocusableInfo {
        id: id.clone(),
        rect: area,
        on_key: button_on_key(on_press.clone()),
        on_focus: on_focus.clone(),
        on_blur: on_blur.clone(),
        inside_panel,
    });

    // Register interaction handlers
    if let Some(msg) = on_press {
        registry.register_click(area, msg.clone());
    }
    if let Some(msg) = on_hover {
        registry.register_hover(area, msg.clone());
    }
    if let Some(msg) = on_hover_exit {
        registry.register_hover_exit(area, msg.clone());
    }

    // Check if this button is focused
    let is_focused = focused_id == Some(id);

    // Render button widget
    let default_style = Style::default().fg(theme.text_primary);
    // Always show focus border on button (unlike other widgets, buttons need clear visual focus)
    let border_style = if is_focused {
        Style::default().fg(theme.accent_primary)
    } else {
        Style::default().fg(theme.border_secondary)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);
    let widget = Paragraph::new(label)
        .block(block)
        .alignment(Alignment::Center)
        .style(style.unwrap_or(default_style));
    frame.render_widget(widget, area);
}
