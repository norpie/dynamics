use crate::tui::command::DispatchTarget;
use crate::tui::element::FocusId;
use crate::tui::renderer::{FocusRegistry, FocusableInfo, InteractionRegistry};
use crate::tui::widgets::TextInputEvent;
use crate::tui::{Element, Theme};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

/// Create on_key handler for text inputs (all keys pass to on_change, Enter also fires on_submit)
pub fn text_input_on_key<Msg: Clone + Send + 'static>(
    on_change: Option<fn(KeyCode) -> Msg>,
    on_submit: Option<Msg>,
) -> Box<dyn Fn(KeyEvent) -> DispatchTarget<Msg> + Send> {
    Box::new(move |key_event| match key_event.code {
        KeyCode::Enter => {
            // Enter fires on_submit (app handles whether to also send on_change)
            if let Some(msg) = on_submit.clone() {
                DispatchTarget::AppMsg(msg)
            } else {
                // No handler - use WidgetEvent for auto-dispatch
                DispatchTarget::WidgetEvent(Box::new(TextInputEvent::Submit))
            }
        }
        KeyCode::Esc => DispatchTarget::PassThrough, // Let runtime handle unfocus/modal close
        _ => {
            // All other keys go to on_change for app to handle via TextInputState
            if let Some(f) = on_change {
                DispatchTarget::AppMsg(f(key_event.code))
            } else {
                // No handler - use WidgetEvent for auto-dispatch
                DispatchTarget::WidgetEvent(Box::new(TextInputEvent::Changed(key_event.code)))
            }
        }
    })
}

/// Create on_key handler for text inputs using unified event pattern
pub fn text_input_on_key_event<Msg: Clone + Send + 'static>(
    on_event: fn(TextInputEvent) -> Msg,
) -> Box<dyn Fn(KeyEvent) -> DispatchTarget<Msg> + Send> {
    Box::new(move |key_event| match key_event.code {
        KeyCode::Enter => DispatchTarget::AppMsg(on_event(TextInputEvent::Submit)),
        KeyCode::Esc => DispatchTarget::PassThrough, // Let runtime handle unfocus/modal close
        _ => DispatchTarget::AppMsg(on_event(TextInputEvent::Changed(key_event.code))),
    })
}

/// Render TextInput element
pub fn render_text_input<Msg: Clone + Send + 'static>(
    frame: &mut Frame,

    registry: &mut InteractionRegistry<Msg>,
    focus_registry: &mut FocusRegistry<Msg>,
    focused_id: Option<&FocusId>,
    id: &FocusId,
    value: &str,
    cursor_pos: usize,
    scroll_offset: usize,
    placeholder: &Option<String>,
    max_length: &Option<usize>,
    masked: bool,
    on_change: &Option<fn(KeyCode) -> Msg>,
    on_submit: &Option<Msg>,
    on_event: &Option<fn(TextInputEvent) -> Msg>,
    on_focus: &Option<Msg>,
    on_blur: &Option<Msg>,
    area: Rect,
    inside_panel: bool,
) {
    let theme = &crate::global_runtime_config().theme;
    // Choose handler based on which callback is provided
    let on_key = if let Some(event_handler) = on_event {
        text_input_on_key_event(*event_handler)
    } else {
        text_input_on_key(on_change.clone(), on_submit.clone())
    };

    // Register in focus registry
    focus_registry.register_focusable(FocusableInfo {
        id: id.clone(),
        rect: area,
        on_key,
        on_focus: on_focus.clone(),
        on_blur: on_blur.clone(),
        inside_panel,
    });

    // Check if this input is focused
    let is_focused = focused_id == Some(id);

    // Calculate visible width (area width - 2 for minimal padding)
    let visible_width = area.width.saturating_sub(2) as usize;

    // Get visible portion of text (masked if password)
    let chars: Vec<char> = value.chars().collect();
    let start_idx = scroll_offset;
    let end_idx = (start_idx + visible_width).min(chars.len());
    let visible_text: String = if masked {
        // Replace all characters with bullets for password masking
        chars[start_idx..end_idx].iter().map(|_| '•').collect()
    } else {
        chars[start_idx..end_idx].iter().collect()
    };

    // Calculate cursor position in visible area
    let cursor_in_visible = cursor_pos.saturating_sub(start_idx);

    // Build display with styled spans for block cursor
    let widget = if value.is_empty() && !is_focused {
        // Show placeholder
        let placeholder_text = if let Some(ph) = placeholder {
            format!(" {}", ph) // Add left padding
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
            " ".to_string() // Cursor at end - use space
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
            .bg(theme.border_primary); // Semi-transparent block cursor

        let mut spans = vec![Span::raw(" ")]; // Left padding
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
}
