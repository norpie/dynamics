use crate::tui::element::FocusId;
use crate::tui::renderer::{DropdownRegistry, FocusRegistry, InteractionRegistry};
use crate::tui::{Element, LayoutConstraint, Theme};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

/// Calculate ratatui Constraints from our LayoutConstraints
pub fn calculate_constraints<Msg>(
    items: &[(LayoutConstraint, Element<Msg>)],
    available_space: u16,
) -> Vec<Constraint> {
    // Pass 1: Calculate fixed and minimum sizes
    let mut fixed_total = 0u16;
    let mut fill_total_weight = 0u16;

    for (constraint, _) in items {
        match constraint {
            LayoutConstraint::Length(n) => fixed_total += n,
            LayoutConstraint::Min(n) => fixed_total += n,
            LayoutConstraint::Fill(weight) => fill_total_weight += weight,
        }
    }

    // Pass 2: Calculate remaining space for Fill elements
    let remaining = available_space.saturating_sub(fixed_total);

    // Pass 3: Build ratatui constraints
    items
        .iter()
        .map(|(constraint, _)| match constraint {
            LayoutConstraint::Length(n) => Constraint::Length(*n),
            LayoutConstraint::Min(n) => Constraint::Min(*n),
            LayoutConstraint::Fill(weight) => {
                if fill_total_weight > 0 {
                    // Calculate proportional space
                    let space =
                        (remaining as u32 * *weight as u32 / fill_total_weight as u32) as u16;
                    Constraint::Length(space)
                } else {
                    Constraint::Length(0)
                }
            }
        })
        .collect()
}

/// Render Column element
pub fn render_column<Msg: Clone + Send + 'static>(
    frame: &mut Frame,

    registry: &mut InteractionRegistry<Msg>,
    focus_registry: &mut FocusRegistry<Msg>,
    dropdown_registry: &mut DropdownRegistry<Msg>,
    focused_id: Option<&FocusId>,
    items: &[(LayoutConstraint, Element<Msg>)],
    spacing: u16,
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
    if items.is_empty() {
        return;
    }

    // Calculate ratatui constraints from layout constraints
    let constraints = calculate_constraints(items, area.height);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Render each child
    for ((_, child), chunk) in items.iter().zip(chunks.iter()) {
        render_fn(
            frame,
            registry,
            focus_registry,
            dropdown_registry,
            focused_id,
            child,
            *chunk,
            inside_panel,
        );
    }
}

/// Render Row element
pub fn render_row<Msg: Clone + Send + 'static>(
    frame: &mut Frame,

    registry: &mut InteractionRegistry<Msg>,
    focus_registry: &mut FocusRegistry<Msg>,
    dropdown_registry: &mut DropdownRegistry<Msg>,
    focused_id: Option<&FocusId>,
    items: &[(LayoutConstraint, Element<Msg>)],
    spacing: u16,
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
    if items.is_empty() {
        return;
    }

    // Calculate ratatui constraints from layout constraints
    let constraints = calculate_constraints(items, area.width);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    // Render each child
    for ((_, child), chunk) in items.iter().zip(chunks.iter()) {
        render_fn(
            frame,
            registry,
            focus_registry,
            dropdown_registry,
            focused_id,
            child,
            *chunk,
            inside_panel,
        );
    }
}

/// Render Container element
pub fn render_container<Msg: Clone + Send + 'static>(
    frame: &mut Frame,

    registry: &mut InteractionRegistry<Msg>,
    focus_registry: &mut FocusRegistry<Msg>,
    dropdown_registry: &mut DropdownRegistry<Msg>,
    focused_id: Option<&FocusId>,
    child: &Element<Msg>,
    padding: u16,
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
    // Apply padding by shrinking the area
    let padded_area = Rect {
        x: area.x + padding,
        y: area.y + padding,
        width: area.width.saturating_sub(padding * 2),
        height: area.height.saturating_sub(padding * 2),
    };
    render_fn(
        frame,
        registry,
        focus_registry,
        dropdown_registry,
        focused_id,
        child,
        padded_area,
        inside_panel,
    );
}
