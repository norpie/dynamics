use crate::tui::element::FocusId;
use crate::tui::renderer::{DropdownRegistry, FocusRegistry, InteractionRegistry};
use crate::tui::{Element, Theme};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders},
};

/// Check if an element or its descendants contain a focused widget (excluding buttons) with the given ID
/// This is used for panel focus styling - all focusable widgets except buttons trigger panel focus borders
pub fn element_contains_focused_non_button<Msg>(
    element: &Element<Msg>,
    focused_id: &FocusId,
) -> bool {
    match element {
        // Check all focusable widgets EXCEPT buttons
        Element::TextInput { id, .. }
        | Element::Select { id, .. }
        | Element::Autocomplete { id, .. }
        | Element::MultiSelect { id, .. }
        | Element::List { id, .. }
        | Element::Tree { id, .. }
        | Element::TableTree { id, .. }
        | Element::Scrollable { id, .. }
        | Element::FileBrowser { id, .. } => id == focused_id,
        // Recurse through containers
        Element::Column { items, .. } | Element::Row { items, .. } => items
            .iter()
            .any(|(_, child)| element_contains_focused_non_button(child, focused_id)),
        Element::Container { child, .. } | Element::Panel { child, .. } => {
            element_contains_focused_non_button(child, focused_id)
        }
        Element::Stack { layers } => layers
            .iter()
            .any(|layer| element_contains_focused_non_button(&layer.element, focused_id)),
        // Don't trigger panel focus for buttons
        Element::Button { .. } => false,
        _ => false,
    }
}

/// Check if an element tree contains a Panel that itself contains a focused widget (excluding buttons)
/// This is used to determine if a panel should delegate focus styling to a nested panel
pub fn element_contains_focused_non_button_panel<Msg>(
    element: &Element<Msg>,
    focused_id: &FocusId,
) -> bool {
    match element {
        Element::Panel { child, .. } => {
            // This is a panel - check if it contains a focused non-button widget
            element_contains_focused_non_button(child, focused_id)
        }
        Element::Column { items, .. } | Element::Row { items, .. } => items
            .iter()
            .any(|(_, child)| element_contains_focused_non_button_panel(child, focused_id)),
        Element::Container { child, .. } => {
            element_contains_focused_non_button_panel(child, focused_id)
        }
        Element::Stack { layers } => layers
            .iter()
            .any(|layer| element_contains_focused_non_button_panel(&layer.element, focused_id)),
        _ => false,
    }
}

/// Render Panel element
pub fn render_panel<Msg: Clone + Send + 'static>(
    frame: &mut Frame,

    registry: &mut InteractionRegistry<Msg>,
    focus_registry: &mut FocusRegistry<Msg>,
    dropdown_registry: &mut DropdownRegistry<Msg>,
    focused_id: Option<&FocusId>,
    child: &Element<Msg>,
    title: &Option<String>,
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
    // Check if the child (or any descendant) contains a focused widget (excluding buttons)
    // All focusable widgets except buttons trigger panel focus styling
    let child_has_focused_widget = focused_id
        .map(|fid| element_contains_focused_non_button(child, fid))
        .unwrap_or(false);

    // Check if any descendant Panel contains a focused widget (excluding buttons)
    // If so, delegate focus styling to that inner panel
    let has_nested_focused_panel = focused_id
        .map(|fid| element_contains_focused_non_button_panel(child, fid))
        .unwrap_or(false);

    // Use focus color for panel border ONLY if:
    // 1. Child contains a focused widget (TextInput, Select, Autocomplete, List, Tree, Scrollable), AND
    // 2. No descendant Panel contains a focused widget (this is the innermost panel)
    // Note: Buttons do NOT trigger panel focus styling
    let border_color = if child_has_focused_widget && !has_nested_focused_panel {
        theme.accent_primary
    } else {
        theme.border_secondary
    };

    // Render border block with background
    let block = if let Some(title_text) = title {
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(theme.bg_base))
            .title(title_text.as_str())
    } else {
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(theme.bg_base))
    };

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    // Render child in the inner area, marking it as inside a panel
    render_fn(
        frame,
        registry,
        focus_registry,
        dropdown_registry,
        focused_id,
        child,
        inner_area,
        true,
    );
}
