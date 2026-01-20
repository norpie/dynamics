use crate::tui::command::DispatchTarget;
use crate::tui::element::FocusId;
use crate::tui::renderer::{
    DropdownCallback, DropdownInfo, DropdownRegistry, FocusRegistry, FocusableInfo,
    InteractionRegistry,
};
use crate::tui::widgets::SelectEvent;
use crate::tui::{Element, Theme};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Frame, layout::Rect, style::Style, widgets::Paragraph};

/// Create on_key handler for select elements (dropdown navigation) - old pattern
pub fn select_on_key<Msg: Clone + Send + 'static>(
    is_open: bool,
    on_toggle: Option<Msg>,
    on_navigate: Option<fn(KeyCode) -> Msg>,
) -> Box<dyn Fn(KeyEvent) -> DispatchTarget<Msg> + Send> {
    Box::new(move |key_event| {
        if !is_open {
            // Closed: Enter/Space toggles dropdown, Esc passes through for unfocus
            match key_event.code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(msg) = on_toggle.clone() {
                        DispatchTarget::AppMsg(msg)
                    } else {
                        DispatchTarget::WidgetEvent(Box::new(SelectEvent::Navigate(key_event.code)))
                    }
                }
                KeyCode::Esc => {
                    // Let runtime handle unfocus/modal close
                    DispatchTarget::PassThrough
                }
                _ => {
                    // Unhandled key - pass through to global subscriptions
                    DispatchTarget::PassThrough
                }
            }
        } else {
            // Open: Up/Down/Enter/Esc handled via on_navigate
            match key_event.code {
                KeyCode::Up | KeyCode::Down | KeyCode::Enter | KeyCode::Esc => {
                    if let Some(f) = on_navigate {
                        DispatchTarget::AppMsg(f(key_event.code))
                    } else {
                        DispatchTarget::WidgetEvent(Box::new(SelectEvent::Navigate(key_event.code)))
                    }
                }
                _ => {
                    // Unhandled key - pass through to global subscriptions
                    DispatchTarget::PassThrough
                }
            }
        }
    })
}

/// Create on_key handler for select elements (new event pattern)
pub fn select_on_key_event<Msg: Clone + Send + 'static>(
    is_open: bool,
    on_event: fn(SelectEvent) -> Msg,
) -> Box<dyn Fn(KeyEvent) -> DispatchTarget<Msg> + Send> {
    Box::new(move |key_event| {
        if !is_open {
            // Closed: Enter/Space toggles dropdown, Esc passes through for unfocus
            match key_event.code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    DispatchTarget::AppMsg(on_event(SelectEvent::Navigate(key_event.code)))
                }
                KeyCode::Esc => {
                    // Let runtime handle unfocus/modal close
                    DispatchTarget::PassThrough
                }
                _ => {
                    // Unhandled key - pass through to global subscriptions
                    DispatchTarget::PassThrough
                }
            }
        } else {
            // Open: Up/Down/Enter/Esc handled via Navigate event
            match key_event.code {
                KeyCode::Up | KeyCode::Down | KeyCode::Enter | KeyCode::Esc => {
                    DispatchTarget::AppMsg(on_event(SelectEvent::Navigate(key_event.code)))
                }
                _ => {
                    // Unhandled key - pass through to global subscriptions
                    DispatchTarget::PassThrough
                }
            }
        }
    })
}

/// Render Select element
pub fn render_select<Msg: Clone + Send + 'static>(
    frame: &mut Frame,

    registry: &mut InteractionRegistry<Msg>,
    focus_registry: &mut FocusRegistry<Msg>,
    dropdown_registry: &mut DropdownRegistry<Msg>,
    focused_id: Option<&FocusId>,
    id: &FocusId,
    options: &[String],
    selected: usize,
    is_open: bool,
    highlight: usize,
    on_select: &Option<fn(usize) -> Msg>,
    on_toggle: &Option<Msg>,
    on_navigate: &Option<fn(KeyCode) -> Msg>,
    on_event: &Option<fn(SelectEvent) -> Msg>,
    on_focus: &Option<Msg>,
    on_blur: &Option<Msg>,
    area: Rect,
    inside_panel: bool,
) {
    let theme = &crate::global_runtime_config().theme;
    // Register in focus registry - prefer on_event if available
    let on_key_handler = if let Some(event_fn) = on_event {
        select_on_key_event(is_open, *event_fn)
    } else {
        select_on_key(is_open, on_toggle.clone(), on_navigate.clone())
    };

    // Wrap on_blur to also send SelectEvent::Blur when using event pattern
    let on_blur_handler = if let Some(event_fn) = on_event {
        // If using event pattern, send Blur event along with any custom on_blur
        let blur_msg = event_fn(SelectEvent::Blur);
        if let Some(custom_blur) = on_blur {
            Some(blur_msg) // For now just use the Blur event, custom_blur would need batching
        } else {
            Some(blur_msg)
        }
    } else {
        on_blur.clone()
    };

    focus_registry.register_focusable(FocusableInfo {
        id: id.clone(),
        rect: area,
        on_key: on_key_handler,
        on_focus: on_focus.clone(),
        on_blur: on_blur_handler,
        inside_panel,
    });

    let is_focused = focused_id == Some(id);

    // Get selected option text
    // If options is empty, show placeholder text
    let selected_text = if options.is_empty() {
        ""
    } else if selected < options.len() {
        &options[selected]
    } else {
        ""
    };

    // Render borderless: selected value + arrow (like TextInput)
    let arrow = if is_open { " ▲" } else { " ▼" };
    let display_text = format!(" {}{}", selected_text, arrow); // Add left padding

    // Render text without border
    let text_widget = Paragraph::new(display_text).style(Style::default().fg(theme.text_primary));
    frame.render_widget(text_widget, area);

    // Register click handler for toggle
    if let Some(toggle_msg) = on_toggle {
        registry.register_click(area, toggle_msg.clone());
    }

    // If open, register dropdown for overlay rendering (rendered after main UI)
    if is_open && !options.is_empty() {
        let callback = if let Some(event_fn) = on_event {
            DropdownCallback::SelectEvent(Some(*event_fn))
        } else {
            DropdownCallback::Select(*on_select)
        };

        dropdown_registry.register(DropdownInfo {
            select_area: area,
            options: options.to_vec(),
            selected: Some(selected),
            highlight,
            on_select: callback,
        });
    }
}
