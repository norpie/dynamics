use crate::tui::command::DispatchTarget;
use crate::tui::element::FocusId;
use crate::tui::renderer::{DropdownRegistry, FocusRegistry, FocusableInfo, InteractionRegistry};
use crate::tui::{Element, LayoutConstraint, Theme};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Borders},
};

/// Apply horizontal scroll offset to an element by modifying its text content
fn apply_horizontal_offset<Msg: Clone>(element: &Element<Msg>, offset: usize) -> Element<Msg> {
    if offset == 0 {
        return element.clone();
    }

    match element {
        Element::Text { content, style } => {
            let trimmed = if content.len() > offset {
                content.chars().skip(offset).collect()
            } else {
                String::new()
            };
            Element::Text {
                content: trimmed,
                style: *style,
            }
        }
        Element::StyledText { line, background } => {
            // Apply offset to the line by skipping characters
            let mut remaining_offset = offset;
            let mut new_spans = Vec::new();

            for span in &line.spans {
                let span_len = span.content.len();
                if remaining_offset >= span_len {
                    // Skip entire span
                    remaining_offset -= span_len;
                } else if remaining_offset > 0 {
                    // Partially skip this span
                    let trimmed: String = span.content.chars().skip(remaining_offset).collect();
                    new_spans.push(ratatui::text::Span::styled(trimmed, span.style));
                    remaining_offset = 0;
                } else {
                    // Include full span
                    new_spans.push(span.clone());
                }
            }

            Element::StyledText {
                line: ratatui::text::Line::from(new_spans),
                background: *background,
            }
        }
        Element::Column { items, spacing } => {
            // Apply offset to all children
            let new_items: Vec<_> = items
                .iter()
                .map(|(constraint, child)| (*constraint, apply_horizontal_offset(child, offset)))
                .collect();
            Element::Column {
                items: new_items,
                spacing: *spacing,
            }
        }
        Element::Row { items, spacing } => {
            // For rows, we need to figure out which items are visible after offset
            let mut remaining_offset = offset;
            let mut new_items = Vec::new();

            for (constraint, child) in items {
                let child_width = measure_element_width(child);
                if remaining_offset >= child_width {
                    // Skip this entire child
                    remaining_offset -= child_width;
                    if items.len() > 1 && *spacing > 0 {
                        remaining_offset = remaining_offset.saturating_sub(*spacing as usize);
                    }
                } else if remaining_offset > 0 {
                    // Partially visible child
                    new_items.push((
                        *constraint,
                        apply_horizontal_offset(child, remaining_offset),
                    ));
                    remaining_offset = 0;
                } else {
                    // Fully visible child
                    new_items.push((*constraint, child.clone()));
                }
            }

            Element::Row {
                items: new_items,
                spacing: *spacing,
            }
        }
        // For other element types, just clone as-is
        _ => element.clone(),
    }
}

/// Measure the width of an element (for horizontal scrolling)
fn measure_element_width<Msg>(element: &Element<Msg>) -> usize {
    match element {
        Element::Text { content, .. } => content.len(),
        Element::StyledText { line, .. } => {
            // Sum up all span widths
            line.spans.iter().map(|span| span.content.len()).sum()
        }
        Element::Column { items, .. } => {
            // Max width of any child
            items
                .iter()
                .map(|(_, child)| measure_element_width(child))
                .max()
                .unwrap_or(0)
        }
        Element::Row { items, spacing, .. } => {
            // Sum of all children + spacing
            let content_width: usize = items
                .iter()
                .map(|(_, child)| measure_element_width(child))
                .sum();
            let spacing_width = items.len().saturating_sub(1) * (*spacing as usize);
            content_width + spacing_width
        }
        Element::Container { child, .. } => measure_element_width(child),
        Element::Panel { child, .. } => measure_element_width(child).saturating_sub(2), // Account for borders
        Element::Scrollable { child, .. } => measure_element_width(child),
        _ => 0, // Default for non-measurable elements
    }
}

/// Create on_key handler for scrollable elements (scroll navigation)
pub fn scrollable_on_key<Msg: Clone + Send + 'static>(
    on_navigate: Option<fn(KeyCode) -> Msg>,
) -> Box<dyn Fn(KeyEvent) -> DispatchTarget<Msg> + Send> {
    Box::new(move |key_event| match key_event.code {
        // Scroll navigation keys (including horizontal)
        KeyCode::Up
        | KeyCode::Down
        | KeyCode::PageUp
        | KeyCode::PageDown
        | KeyCode::Home
        | KeyCode::End
        | KeyCode::Left
        | KeyCode::Right => {
            if let Some(f) = on_navigate {
                DispatchTarget::AppMsg(f(key_event.code))
            } else {
                // No callback - pass through to global subscriptions
                DispatchTarget::PassThrough
            }
        }
        _ => {
            // Unhandled key - pass through to global subscriptions
            DispatchTarget::PassThrough
        }
    })
}

/// Render Scrollable element
pub fn render_scrollable<Msg: Clone + Send + 'static>(
    frame: &mut Frame,

    registry: &mut InteractionRegistry<Msg>,
    focus_registry: &mut FocusRegistry<Msg>,
    dropdown_registry: &mut DropdownRegistry<Msg>,
    focused_id: Option<&FocusId>,
    id: &FocusId,
    child: &Element<Msg>,
    scroll_offset: usize,
    content_height: &Option<usize>,
    horizontal_scroll_offset: usize,
    content_width: &Option<usize>,
    on_navigate: &Option<fn(KeyCode) -> Msg>,
    on_render: &Option<fn(usize, usize, usize, usize) -> Msg>,
    on_focus: &Option<Msg>,
    on_blur: &Option<Msg>,
    area: Rect,
    inside_panel: bool,
    render_fn: impl Fn(
        &mut Frame,
        &mut InteractionRegistry<Msg>,
        &mut FocusRegistry<Msg>,
        &mut DropdownRegistry<Msg>,
        Option<&FocusId>,
        &Element<Msg>,
        Rect,
        bool,
    ),
) {
    let theme = &crate::global_runtime_config().theme;
    // Register in focus registry
    focus_registry.register_focusable(FocusableInfo {
        id: id.clone(),
        rect: area,
        on_key: scrollable_on_key(on_navigate.clone()),
        on_focus: on_focus.clone(),
        on_blur: on_blur.clone(),
        inside_panel,
    });

    // Check if this scrollable is focused
    let is_focused = focused_id == Some(id);

    // Calculate dimensions
    let viewport_height = area.height as usize;
    let viewport_width = area.width as usize;

    // Determine content height
    let detected_content_height = match child {
        Element::Column { items, spacing } => {
            // Sum up item heights + spacing between items
            let total_item_height: usize = items
                .iter()
                .map(|(constraint, _)| {
                    match constraint {
                        LayoutConstraint::Length(n) => *n as usize,
                        _ => 1, // Default to 1 for other constraint types
                    }
                })
                .sum();

            // Add spacing between items (N items = N-1 gaps)
            let total_spacing = items.len().saturating_sub(1) * (*spacing as usize);
            total_item_height + total_spacing
        }
        _ => content_height.unwrap_or(viewport_height),
    };
    let actual_content_height = content_height.unwrap_or(detected_content_height);

    // Determine content width by measuring actual content
    let detected_content_width = match child {
        Element::Column { items, .. } => {
            // Find maximum width of any item
            items
                .iter()
                .map(|(_, item)| measure_element_width(item))
                .max()
                .unwrap_or(viewport_width)
        }
        Element::Row { items, .. } => {
            // Sum widths of all items
            items
                .iter()
                .map(|(_, item)| measure_element_width(item))
                .sum()
        }
        _ => measure_element_width(child),
    };
    let actual_content_width = content_width.unwrap_or(detected_content_width);

    // Call on_render with actual dimensions (all four: height and width)
    if let Some(render_fn) = on_render {
        registry.add_render_message(render_fn(
            viewport_height,
            actual_content_height,
            viewport_width,
            actual_content_width,
        ));
    }

    // Reserve space for scrollbars if needed
    let needs_vertical_scrollbar = actual_content_height > viewport_height;
    let needs_horizontal_scrollbar = actual_content_width > viewport_width;

    let available_width = if needs_vertical_scrollbar {
        area.width.saturating_sub(1)
    } else {
        area.width
    };

    let available_height = if needs_horizontal_scrollbar {
        area.height.saturating_sub(1)
    } else {
        area.height
    };

    let content_area = Rect {
        x: area.x,
        y: area.y,
        width: available_width,
        height: available_height,
    };

    // Clamp scroll offsets
    let max_vertical_scroll = actual_content_height.saturating_sub(available_height as usize);
    let clamped_vertical_scroll = scroll_offset.min(max_vertical_scroll);

    let max_horizontal_scroll = actual_content_width.saturating_sub(available_width as usize);
    let clamped_horizontal_scroll = horizontal_scroll_offset.min(max_horizontal_scroll);

    // Render content based on type
    match child {
        Element::Column { items, spacing } => {
            // Manually position each item with proper spacing, accounting for scroll
            let mut current_y = 0i32; // Virtual Y position (0 = top of content, can be negative due to scroll)

            for (idx, (constraint, item_child)) in items.iter().enumerate() {
                // Get item height from constraint (simplified - assume Length constraints for now)
                let item_height = match constraint {
                    LayoutConstraint::Length(n) => *n as usize,
                    _ => 1, // Default to 1 line for other constraint types
                };

                // Calculate where this item appears after scrolling
                let scrolled_y = current_y - clamped_vertical_scroll as i32;

                // Check if item is within visible viewport
                if scrolled_y + item_height as i32 > 0 && scrolled_y < available_height as i32 {
                    // Calculate actual screen position
                    let screen_y =
                        (content_area.y as i32 + scrolled_y).max(content_area.y as i32) as u16;
                    let available_item_height =
                        (content_area.y + content_area.height).saturating_sub(screen_y);

                    if available_item_height > 0 {
                        // Apply horizontal scroll offset to the content
                        let scrolled_child = if clamped_horizontal_scroll > 0 {
                            apply_horizontal_offset(item_child, clamped_horizontal_scroll)
                        } else {
                            item_child.clone()
                        };

                        let item_area = Rect {
                            x: content_area.x,
                            y: screen_y,
                            width: content_area.width,
                            height: (item_height as u16).min(available_item_height),
                        };

                        render_fn(
                            frame,
                            registry,
                            focus_registry,
                            dropdown_registry,
                            focused_id,
                            &scrolled_child,
                            item_area,
                            inside_panel,
                        );
                    }
                }

                // Advance Y position (item height)
                current_y += item_height as i32;

                // Add spacing only between items, not after the last one
                if idx < items.len() - 1 && *spacing > 0 {
                    current_y += *spacing as i32;
                }
            }
        }
        _ => {
            // Apply horizontal scroll offset to the content
            let scrolled_child = if clamped_horizontal_scroll > 0 {
                apply_horizontal_offset(child, clamped_horizontal_scroll)
            } else {
                child.clone()
            };

            render_fn(
                frame,
                registry,
                focus_registry,
                dropdown_registry,
                focused_id,
                &scrolled_child,
                content_area,
                inside_panel,
            );
        }
    }

    // Render vertical scrollbar if needed
    if needs_vertical_scrollbar {
        let scrollbar_area = Rect {
            x: area.x + available_width,
            y: area.y,
            width: 1,
            height: available_height,
        };

        let scrollbar_position = if max_vertical_scroll > 0 {
            (clamped_vertical_scroll as f32 / max_vertical_scroll as f32
                * (available_height - 1) as f32) as u16
        } else {
            0
        };

        // Render scrollbar thumb
        if scrollbar_position < available_height {
            let thumb_area = Rect {
                x: scrollbar_area.x,
                y: scrollbar_area.y + scrollbar_position,
                width: 1,
                height: 1,
            };
            let thumb = Block::default().style(Style::default().fg(theme.border_primary));
            frame.render_widget(thumb, thumb_area);
        }
    }

    // Render horizontal scrollbar if needed
    if needs_horizontal_scrollbar {
        let scrollbar_area = Rect {
            x: area.x,
            y: area.y + available_height,
            width: available_width,
            height: 1,
        };

        let scrollbar_position = if max_horizontal_scroll > 0 {
            (clamped_horizontal_scroll as f32 / max_horizontal_scroll as f32
                * (available_width - 1) as f32) as u16
        } else {
            0
        };

        // Render scrollbar thumb
        if scrollbar_position < available_width {
            let thumb_area = Rect {
                x: scrollbar_area.x + scrollbar_position,
                y: scrollbar_area.y,
                width: 1,
                height: 1,
            };
            let thumb = Block::default().style(Style::default().fg(theme.border_primary));
            frame.render_widget(thumb, thumb_area);
        }
    }

    // Only render focus border if NOT inside a panel
    if is_focused && !inside_panel {
        let border = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_primary));
        frame.render_widget(border, area);
    }
}
