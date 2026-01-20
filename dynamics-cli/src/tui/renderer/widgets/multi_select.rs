use crate::tui::command::DispatchTarget;
use crate::tui::element::FocusId;
use crate::tui::renderer::{
    DropdownCallback, DropdownInfo, DropdownRegistry, FocusRegistry, FocusableInfo,
    InteractionRegistry,
};
use crate::tui::widgets::MultiSelectEvent;
use crate::tui::{Element, Theme};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

/// Create on_key handler for multi-select elements
pub fn multi_select_on_key_event<Msg: Clone + Send + 'static>(
    is_open: bool,
    on_event: fn(MultiSelectEvent) -> Msg,
) -> Box<dyn Fn(KeyEvent) -> DispatchTarget<Msg> + Send> {
    Box::new(move |key_event| {
        if is_open {
            // Dropdown open: Up/Down/Enter/Esc go to navigate, others to input
            match key_event.code {
                KeyCode::Up | KeyCode::Down | KeyCode::Enter | KeyCode::Esc => {
                    DispatchTarget::AppMsg(on_event(MultiSelectEvent::Navigate(key_event.code)))
                }
                _ => {
                    // All other keys go to input for typing
                    DispatchTarget::AppMsg(on_event(MultiSelectEvent::Input(key_event.code)))
                }
            }
        } else {
            // Dropdown closed: Escape passes through for unfocus, others go to input
            match key_event.code {
                KeyCode::Esc => DispatchTarget::PassThrough,
                _ => DispatchTarget::AppMsg(on_event(MultiSelectEvent::Input(key_event.code))),
            }
        }
    })
}

/// Render MultiSelect element
pub fn render_multi_select<Msg: Clone + Send + 'static>(
    frame: &mut Frame,

    registry: &mut InteractionRegistry<Msg>,
    focus_registry: &mut FocusRegistry<Msg>,
    dropdown_registry: &mut DropdownRegistry<Msg>,
    focused_id: Option<&FocusId>,
    id: &FocusId,
    selected_items: &[String],
    search_input: &str,
    placeholder: &Option<String>,
    is_open: bool,
    filtered_options: &[String],
    highlight: usize,
    on_event: &Option<fn(MultiSelectEvent) -> Msg>,
    on_focus: &Option<Msg>,
    on_blur: &Option<Msg>,
    area: Rect,
    inside_panel: bool,
) {
    let theme = &crate::global_runtime_config().theme;
    let is_focused = focused_id == Some(id);

    // Register focus handler if provided
    if let Some(on_event_fn) = on_event {
        let on_key = multi_select_on_key_event(is_open, *on_event_fn);
        focus_registry.register_focusable(FocusableInfo {
            id: id.clone(),
            rect: area,
            on_key,
            on_focus: on_focus.clone(),
            on_blur: on_blur.clone(),
            inside_panel,
        });
    }

    // Build display content (single line like autocomplete)
    let display_text = if search_input.is_empty() && !is_focused {
        // Show placeholder or selected count
        if selected_items.is_empty() {
            if let Some(ph) = placeholder {
                format!(" {}", ph)
            } else {
                String::from(" ")
            }
        } else {
            format!(" ({} selected)", selected_items.len())
        }
    } else if is_focused {
        // Show cursor when focused
        format!(" {}│", search_input)
    } else {
        format!(" {}", search_input)
    };

    let text_style = if search_input.is_empty() && !is_focused {
        Style::default().fg(theme.border_primary).italic()
    } else {
        Style::default().fg(theme.text_primary)
    };

    // Render text (single line)
    let text_widget = Paragraph::new(display_text).style(text_style);
    frame.render_widget(text_widget, area);

    // Register dropdown overlay (rendered by dropdown_registry)
    if is_open && !filtered_options.is_empty() && on_event.is_some() {
        let on_event_fn = on_event.unwrap();
        dropdown_registry.register(DropdownInfo {
            select_area: area,
            options: filtered_options.to_vec(),
            selected: None, // No checkmark for multi-select (selected items shown as chips)
            highlight,
            on_select: DropdownCallback::MultiSelectEvent(Some(on_event_fn)),
        });
    }
}
