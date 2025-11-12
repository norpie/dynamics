use ratatui::{Frame, style::{Style, Stylize}, widgets::Paragraph, layout::Rect, text::{Line, Span}};
use crossterm::event::{KeyCode, KeyEvent};
use crate::tui::{Element, Theme};
use crate::tui::element::FocusId;
use crate::tui::command::DispatchTarget;
use crate::tui::widgets::MultiSelectEvent;
use crate::tui::renderer::{InteractionRegistry, FocusRegistry, DropdownRegistry, DropdownInfo, DropdownCallback, FocusableInfo};

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

    // Build display content
    let mut lines = Vec::new();

    // Line 1: Selected items as chips
    if selected_items.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  No items selected", Style::default().fg(theme.text_tertiary)),
        ]));
    } else {
        let mut chips = Vec::new();
        chips.push(Span::raw("  "));
        for (i, item) in selected_items.iter().enumerate() {
            if i > 0 {
                chips.push(Span::raw(" "));
            }
            chips.push(Span::styled(
                format!("[{}]", item),
                Style::default().fg(theme.accent_primary),
            ));
        }
        lines.push(Line::from(chips));
    }

    // Line 2: Search input
    let input_text = if search_input.is_empty() {
        if let Some(ph) = placeholder {
            Span::styled(format!("  {}", ph), Style::default().fg(theme.text_tertiary))
        } else {
            Span::raw("  ")
        }
    } else {
        Span::styled(format!("  {}", search_input), Style::default())
    };

    let input_style = if is_focused {
        Style::default().fg(theme.accent_primary)
    } else {
        Style::default()
    };

    lines.push(Line::from(vec![input_text.style(input_style)]));

    // Line 3+: Dropdown options (if open)
    if is_open && !filtered_options.is_empty() {
        let visible_count = (area.height.saturating_sub(2) as usize).min(5);
        for (i, option) in filtered_options.iter().take(visible_count).enumerate() {
            let is_highlighted = i == highlight;
            let style = if is_highlighted {
                Style::default().bg(theme.bg_surface)
            } else {
                Style::default()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("    {}", option), style)
            ]));

            // Register click handler for each option
            if let Some(on_event_fn) = on_event {
                let option_area = Rect {
                    x: area.x,
                    y: area.y + 2 + i as u16,
                    width: area.width,
                    height: 1,
                };
                let option_clone = option.clone();
                registry.register_click(option_area, on_event_fn(MultiSelectEvent::Select(option_clone)));
            }
        }
    }

    // Render paragraph
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);

    // Register dropdown if open
    if is_open && !filtered_options.is_empty() && on_event.is_some() {
        let on_event_fn = on_event.unwrap();
        dropdown_registry.register(DropdownInfo {
            select_area: area,
            options: filtered_options.to_vec(),
            selected: None,  // No checkmark for multi-select (selected items shown as chips)
            highlight,
            on_select: DropdownCallback::MultiSelectEvent(Some(on_event_fn)),
        });
    }
}
