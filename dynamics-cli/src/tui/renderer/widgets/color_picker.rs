use crate::tui::color::color_to_hex;
use crate::tui::command::DispatchTarget;
use crate::tui::element::FocusId;
use crate::tui::renderer::{FocusRegistry, FocusableInfo, InteractionRegistry};
use crate::tui::widgets::{Channel, ColorPickerEvent, ColorPickerMode, ColorPickerState};
use crate::tui::{Element, Theme};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

/// Create on_key handler for color picker
pub fn color_picker_on_key<Msg: Clone + Send + 'static>(
    on_event: fn(ColorPickerEvent) -> Msg,
    state: &ColorPickerState,
) -> Box<dyn Fn(KeyEvent) -> DispatchTarget<Msg> + Send> {
    let current_color = state.color();

    Box::new(move |key_event| match key_event.code {
        KeyCode::Enter => {
            // Submit with current color
            DispatchTarget::AppMsg(on_event(ColorPickerEvent::Submitted(current_color)))
        }
        KeyCode::Esc => DispatchTarget::PassThrough, // Let runtime handle unfocus/modal close
        key_code => {
            // Pass key to app for handling
            DispatchTarget::AppMsg(on_event(ColorPickerEvent::Changed(key_code)))
        }
    })
}

/// Render ColorPicker element
pub fn render_color_picker<Msg: Clone + Send + 'static>(
    frame: &mut Frame,
    _registry: &mut InteractionRegistry<Msg>,
    focus_registry: &mut FocusRegistry<Msg>,
    focused_id: Option<&FocusId>,
    id: &FocusId,
    _value: ratatui::style::Color,
    mode: ColorPickerMode,
    state: &ColorPickerState,
    on_event: &Option<fn(ColorPickerEvent) -> Msg>,
    on_focus: &Option<Msg>,
    on_blur: &Option<Msg>,
    area: Rect,
    _inside_panel: bool,
) {
    let theme = &crate::global_runtime_config().theme;
    let is_focused = focused_id == Some(id);

    // Register in focus registry
    if let Some(event_handler) = on_event {
        focus_registry.register_focusable(FocusableInfo {
            id: id.clone(),
            rect: area,
            on_key: color_picker_on_key(*event_handler, state),
            on_focus: on_focus.clone(),
            on_blur: on_blur.clone(),
            inside_panel: _inside_panel,
        });
    }

    // Get current values
    let hsl = state.hsl();
    let (r, g, b) = state.rgb();
    let hex = state.hex();
    let focused_channel = state.focused_channel();

    // Layout: Preview + Mode | Channels | Hex
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Preview + mode
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Channel 1
            Constraint::Length(1), // Channel 2
            Constraint::Length(1), // Channel 3
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Hex input
        ])
        .split(area);

    // Preview box with current color
    let preview_text = format!("  Preview: ████████  {}", color_to_hex(state.color()));
    let mode_text = format!("  Mode: {:?} (M to toggle)", mode);

    let preview_para = Paragraph::new(vec![
        Line::from(preview_text).fg(state.color()),
        Line::from(mode_text).fg(theme.text_secondary),
    ]);
    frame.render_widget(preview_para, chunks[0]);

    // Render channels based on mode
    match mode {
        ColorPickerMode::HSL => {
            render_slider(
                frame,
                chunks[2],
                "Hue",
                hsl.h,
                0.0,
                360.0,
                "°",
                focused_channel == Channel::Primary && is_focused,
                theme,
            );
            render_slider(
                frame,
                chunks[3],
                "Saturation",
                hsl.s,
                0.0,
                100.0,
                "%",
                focused_channel == Channel::Secondary && is_focused,
                theme,
            );
            render_slider(
                frame,
                chunks[4],
                "Lightness",
                hsl.l,
                0.0,
                100.0,
                "%",
                focused_channel == Channel::Tertiary && is_focused,
                theme,
            );
        }
        ColorPickerMode::RGB => {
            render_slider(
                frame,
                chunks[2],
                "Red",
                r as f32,
                0.0,
                255.0,
                "",
                focused_channel == Channel::Primary && is_focused,
                theme,
            );
            render_slider(
                frame,
                chunks[3],
                "Green",
                g as f32,
                0.0,
                255.0,
                "",
                focused_channel == Channel::Secondary && is_focused,
                theme,
            );
            render_slider(
                frame,
                chunks[4],
                "Blue",
                b as f32,
                0.0,
                255.0,
                "",
                focused_channel == Channel::Tertiary && is_focused,
                theme,
            );
        }
    }

    // Hex input
    let hex_focused = focused_channel == Channel::Hex && is_focused;
    let hex_style = if hex_focused {
        Style::default().fg(theme.accent_primary).bold()
    } else {
        Style::default().fg(theme.text_secondary)
    };

    let hex_text = if state.is_hex_editing() {
        format!("  Hex: #{}_", hex)
    } else {
        format!("  Hex: #{}", hex)
    };

    let hex_para = Paragraph::new(Line::from(hex_text)).style(hex_style);
    frame.render_widget(hex_para, chunks[6]);

    // Help text at bottom (if there's room)
    if area.height > 9 {
        let help = "  ←/→: Adjust  Tab: Next  M: Mode  Enter: Confirm";
        let help_para = Paragraph::new(help).style(Style::default().fg(theme.text_tertiary));

        if let Some(last_chunk) = chunks.last() {
            if last_chunk.y + last_chunk.height < area.y + area.height {
                let help_rect = Rect {
                    x: area.x,
                    y: area.y + area.height - 1,
                    width: area.width,
                    height: 1,
                };
                frame.render_widget(help_para, help_rect);
            }
        }
    }
}

/// Render a slider bar for a channel
fn render_slider(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: f32,
    min: f32,
    max: f32,
    unit: &str,
    is_focused: bool,
    theme: &Theme,
) {
    let percentage = ((value - min) / (max - min)).clamp(0.0, 1.0);

    // Calculate slider width (leave room for label and value)
    let label_width = 15;
    let value_width = 12;
    let slider_width = area.width.saturating_sub(label_width + value_width) as usize;

    // Build slider bar
    let filled = (slider_width as f32 * percentage) as usize;
    let empty = slider_width.saturating_sub(filled);

    let bar = format!("[{}{}]", "=".repeat(filled), "-".repeat(empty));

    // Format value
    let value_str = if unit.is_empty() {
        format!("{:.0}", value)
    } else {
        format!("{:.0}{}", value, unit)
    };

    let style = if is_focused {
        Style::default().fg(theme.accent_primary).bold()
    } else {
        Style::default().fg(theme.text_secondary)
    };

    let line = Line::from(vec![
        Span::raw(format!("  {:<11} ", label)).style(style),
        Span::raw(bar).style(style),
        Span::raw(format!(" {:<8} ({:.0}-{:.0})", value_str, min, max))
            .style(Style::default().fg(theme.text_tertiary)),
    ]);

    let para = Paragraph::new(line);
    frame.render_widget(para, area);
}
