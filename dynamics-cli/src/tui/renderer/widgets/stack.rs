use crate::tui::element::FocusId;
use crate::tui::renderer::{DropdownRegistry, FocusRegistry, InteractionRegistry};
use crate::tui::{Alignment as LayerAlignment, Element, Layer, Theme};
use ratatui::{Frame, layout::Rect, style::Style};

/// Render a semi-transparent dim overlay
pub fn render_dim_overlay(frame: &mut Frame, area: Rect) {
    let theme = &crate::global_runtime_config().theme;
    use ratatui::widgets::Paragraph;

    // Render dim overlay using Paragraph for reliable background fill
    // Paragraph properly fills the entire area with the background color,
    // unlike Block which doesn't reliably fill without borders
    let dim_overlay = Paragraph::new("").style(Style::default().bg(theme.bg_surface));
    frame.render_widget(dim_overlay, area);
}

/// Calculate the position of a layer based on its alignment
pub fn calculate_layer_position<Msg>(
    element: &Element<Msg>,
    alignment: LayerAlignment,
    container: Rect,
    estimate_fn: impl Fn(&Element<Msg>, Rect) -> (u16, u16),
) -> Rect {
    // Estimate element size (for centered modal, use reasonable defaults)
    let (width, height) = estimate_fn(element, container);

    match alignment {
        LayerAlignment::TopLeft => Rect {
            x: container.x,
            y: container.y,
            width,
            height,
        },
        LayerAlignment::TopCenter => Rect {
            x: container.x + (container.width.saturating_sub(width)) / 2,
            y: container.y,
            width,
            height,
        },
        LayerAlignment::TopRight => Rect {
            x: container.x + container.width.saturating_sub(width),
            y: container.y,
            width,
            height,
        },
        LayerAlignment::Center => Rect {
            x: container.x + (container.width.saturating_sub(width)) / 2,
            y: container.y + (container.height.saturating_sub(height)) / 2,
            width,
            height,
        },
        LayerAlignment::BottomLeft => Rect {
            x: container.x,
            y: container.y + container.height.saturating_sub(height),
            width,
            height,
        },
        LayerAlignment::BottomCenter => Rect {
            x: container.x + (container.width.saturating_sub(width)) / 2,
            y: container.y + container.height.saturating_sub(height),
            width,
            height,
        },
        LayerAlignment::BottomRight => Rect {
            x: container.x + container.width.saturating_sub(width),
            y: container.y + container.height.saturating_sub(height),
            width,
            height,
        },
    }
}

/// Render Stack element
pub fn render_stack<Msg: Clone + Send + 'static>(
    frame: &mut Frame,

    registry: &mut InteractionRegistry<Msg>,
    focus_registry: &mut FocusRegistry<Msg>,
    dropdown_registry: &mut DropdownRegistry<Msg>,
    focused_id: Option<&FocusId>,
    layers: &[Layer<Msg>],
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
    estimate_fn: impl Fn(&Element<Msg>, Rect) -> (u16, u16),
) {
    log::debug!("Stack::render_stack - rendering {} layers", layers.len());

    // Render all layers visually
    for (layer_idx, layer) in layers.iter().enumerate() {
        log::debug!("  Stack: rendering layer {}", layer_idx);
        // Render dim overlay if requested
        if layer.dim_below {
            render_dim_overlay(frame, area);
        }

        // Calculate position based on alignment
        let layer_area =
            calculate_layer_position(&layer.element, layer.alignment, area, &estimate_fn);

        // Push focus layer context for this stack layer
        focus_registry.push_layer(layer_idx);

        // Render the layer element
        render_fn(
            frame,
            registry,
            focus_registry,
            dropdown_registry,
            focused_id,
            &layer.element,
            layer_area,
            inside_panel,
        );

        // Pop focus layer context
        focus_registry.pop_layer();
    }

    log::debug!(
        "Stack: all layers rendered visually, now clearing registries to re-render topmost only"
    );
    // Clear all interactions and focus, then re-render topmost layer to register only its interactions/focus
    registry.clear();
    log::debug!("Stack: calling focus_registry.clear() to reset for topmost layer");
    focus_registry.clear();
    if let Some(last_layer) = layers.last() {
        let layer_idx = layers.len() - 1;
        log::debug!(
            "Stack: re-rendering topmost layer {} for interaction/focus registration",
            layer_idx
        );
        let layer_area = calculate_layer_position(
            &last_layer.element,
            last_layer.alignment,
            area,
            &estimate_fn,
        );

        // Re-push the topmost layer context
        focus_registry.push_layer(layer_idx);
        render_fn(
            frame,
            registry,
            focus_registry,
            dropdown_registry,
            focused_id,
            &last_layer.element,
            layer_area,
            inside_panel,
        );
        // Keep the layer pushed so focusables remain in active layer

        // Debug: Log focus registry state
        if let Some(layer) = focus_registry.active_layer() {
            log::debug!(
                "Stack: focus registry after re-render - layer {} has {} focusables",
                layer.layer_index,
                layer.focusables.len()
            );
            for focusable in &layer.focusables {
                log::debug!("    Focusable: {:?} at {:?}", focusable.id, focusable.rect);
            }
        }
    }
}
