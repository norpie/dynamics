use ratatui::{Frame, style::{Style, Stylize}, widgets::Paragraph, layout::Rect, text::{Line, Span}};
use crossterm::event::{KeyCode, KeyEvent};
use crate::tui::{Element, Theme};
use crate::tui::element::FocusId;
use crate::tui::command::DispatchTarget;
use crate::tui::widgets::AutocompleteEvent;
use crate::tui::renderer::{InteractionRegistry, FocusRegistry, DropdownRegistry, DropdownInfo, DropdownCallback, FocusableInfo};

/// Create on_key handler for autocomplete elements (old pattern)
pub fn autocomplete_on_key<Msg: Clone + Send + 'static>(
    is_open: bool,
    on_input: Option<fn(KeyCode) -> Msg>,
    on_navigate: Option<fn(KeyCode) -> Msg>,
) -> Box<dyn Fn(KeyEvent) -> DispatchTarget<Msg> + Send> {
    Box::new(move |key_event| {
        if is_open {
            // Dropdown open: Up/Down/Enter/Esc go to navigate, others to input
            match key_event.code {
                KeyCode::Up | KeyCode::Down | KeyCode::Enter | KeyCode::Esc => {
                    if let Some(f) = on_navigate {
                        DispatchTarget::AppMsg(f(key_event.code))
                    } else {
                        DispatchTarget::WidgetEvent(Box::new(AutocompleteEvent::Navigate(key_event.code)))
                    }
                }
                _ => {
                    // All other keys go to input for typing
                    if let Some(f) = on_input {
                        DispatchTarget::AppMsg(f(key_event.code))
                    } else {
                        DispatchTarget::WidgetEvent(Box::new(AutocompleteEvent::Input(key_event.code)))
                    }
                }
            }
        } else {
            // Dropdown closed: Escape passes through for unfocus, others go to input
            match key_event.code {
                KeyCode::Esc => DispatchTarget::PassThrough,
                _ => {
                    if let Some(f) = on_input {
                        DispatchTarget::AppMsg(f(key_event.code))
                    } else {
                        DispatchTarget::WidgetEvent(Box::new(AutocompleteEvent::Input(key_event.code)))
                    }
                }
            }
        }
    })
}

/// Create on_key handler for autocomplete elements (new event pattern)
pub fn autocomplete_on_key_event<Msg: Clone + Send + 'static>(
    is_open: bool,
    on_event: fn(AutocompleteEvent) -> Msg,
) -> Box<dyn Fn(KeyEvent) -> DispatchTarget<Msg> + Send> {
    Box::new(move |key_event| {
        if is_open {
            // Dropdown open: Up/Down/Enter/Esc go to navigate, others to input
            match key_event.code {
                KeyCode::Up | KeyCode::Down | KeyCode::Enter | KeyCode::Esc => {
                    DispatchTarget::AppMsg(on_event(AutocompleteEvent::Navigate(key_event.code)))
                }
                _ => {
                    // All other keys go to input for typing
                    DispatchTarget::AppMsg(on_event(AutocompleteEvent::Input(key_event.code)))
                }
            }
        } else {
            // Dropdown closed: Escape passes through for unfocus, others go to input
            match key_event.code {
                KeyCode::Esc => DispatchTarget::PassThrough,
                _ => DispatchTarget::AppMsg(on_event(AutocompleteEvent::Input(key_event.code))),
            }
        }
    })
}

/// Render Autocomplete element
pub fn render_autocomplete<Msg: Clone + Send + 'static>(
    frame: &mut Frame,

    registry: &mut InteractionRegistry<Msg>,
    focus_registry: &mut FocusRegistry<Msg>,
    dropdown_registry: &mut DropdownRegistry<Msg>,
    focused_id: Option<&FocusId>,
    id: &FocusId,
    all_options: &[String],
    current_input: &str,
    cursor_pos: usize,
    scroll_offset: usize,
    placeholder: &Option<String>,
    is_open: bool,
    filtered_options: &[String],
    highlight: usize,
    on_input: &Option<fn(KeyCode) -> Msg>,
    on_select: &Option<fn(String) -> Msg>,
    on_navigate: &Option<fn(KeyCode) -> Msg>,
    on_event: &Option<fn(AutocompleteEvent) -> Msg>,
    on_focus: &Option<Msg>,
    on_blur: &Option<Msg>,
    area: Rect,
    inside_panel: bool,
) {
    let theme = &crate::global_runtime_config().theme;
    // Register in focus registry - prefer on_event if available
    let on_key_handler = if let Some(event_fn) = on_event {
        autocomplete_on_key_event(is_open, *event_fn)
    } else {
        autocomplete_on_key(is_open, *on_input, *on_navigate)
    };

    focus_registry.register_focusable(FocusableInfo {
        id: id.clone(),
        rect: area,
        on_key: on_key_handler,
        on_focus: on_focus.clone(),
        on_blur: on_blur.clone(),
        inside_panel,
    });

    let is_focused = focused_id == Some(id);

    // Calculate visible width (area width - 2 for minimal padding)
    let visible_width = area.width.saturating_sub(2) as usize;

    // Get visible portion of text with scroll support
    let chars: Vec<char> = current_input.chars().collect();
    let start_idx = scroll_offset;
    let end_idx = (start_idx + visible_width).min(chars.len());
    let visible_text: String = chars.get(start_idx..end_idx)
        .map(|c| c.iter().collect())
        .unwrap_or_default();

    // Calculate cursor position in visible area
    let cursor_in_visible = cursor_pos.saturating_sub(start_idx);

    // Build display with styled spans for block cursor (matching TextInput)
    let widget = if current_input.is_empty() && !is_focused {
        // Show placeholder
        let placeholder_text = if let Some(ph) = placeholder {
            format!(" {}", ph)
        } else {
            String::from(" ")
        };
        let placeholder_style = Style::default().fg(theme.border_primary).italic();
        Paragraph::new(placeholder_text).style(placeholder_style)
    } else if is_focused && cursor_in_visible <= visible_text.len() {
        // Show text with block cursor
        let chars: Vec<char> = visible_text.chars().collect();

        // Split into: before cursor, at cursor, after cursor
        let before: String = chars[..cursor_in_visible].iter().collect();
        let cursor_char = if cursor_in_visible < chars.len() {
            chars[cursor_in_visible].to_string()
        } else {
            " ".to_string()  // Cursor at end - use space
        };
        let after: String = if cursor_in_visible < chars.len() {
            chars[cursor_in_visible + 1..].iter().collect()
        } else {
            String::new()
        };

        // Create styled spans
        let text_style = Style::default().fg(theme.text_primary);
        let cursor_style = Style::default()
            .fg(theme.text_primary)
            .bg(theme.border_primary);  // Block cursor with inverted colors

        let mut spans = vec![Span::raw(" ")];  // Left padding
        if !before.is_empty() {
            spans.push(Span::styled(before, text_style));
        }
        spans.push(Span::styled(cursor_char, cursor_style));
        if !after.is_empty() {
            spans.push(Span::styled(after, text_style));
        }

        Paragraph::new(Line::from(spans))
    } else {
        // Not focused or cursor out of view - show text normally
        let text_style = Style::default().fg(theme.text_primary);
        Paragraph::new(format!(" {}", visible_text)).style(text_style)
    };

    frame.render_widget(widget, area);

    // If open, register dropdown for overlay rendering
    if is_open && !filtered_options.is_empty() {
        let callback = if let Some(event_fn) = on_event {
            DropdownCallback::AutocompleteEvent(Some(*event_fn))
        } else {
            DropdownCallback::Autocomplete(*on_select)
        };

        dropdown_registry.register(DropdownInfo {
            select_area: area,
            options: filtered_options.to_vec(),
            selected: None,  // No checkmark for autocomplete
            highlight,
            on_select: callback,
        });
    }
}
