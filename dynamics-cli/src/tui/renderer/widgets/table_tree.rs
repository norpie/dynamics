use ratatui::{
    Frame,
    style::Style,
    widgets::{Block, Borders, Row, Table, Cell},
    layout::{Rect, Constraint},
    prelude::Stylize,
};
use crossterm::event::{KeyCode, KeyEvent};
use crate::tui::{Theme, LayoutConstraint};
use crate::tui::element::FocusId;
use crate::tui::command::DispatchTarget;
use crate::tui::widgets::{TreeEvent, FlatTableNode};
use crate::tui::renderer::{InteractionRegistry, FocusRegistry, DropdownRegistry, FocusableInfo};

/// Create on_key handler for table trees (reuse tree event handling)
pub fn table_tree_on_key_event<Msg: Clone + Send + 'static>(
    on_event: fn(TreeEvent) -> Msg,
) -> Box<dyn Fn(KeyEvent) -> DispatchTarget<Msg> + Send> {
    Box::new(move |key_event| match key_event.code {
        // Navigation keys
        KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown
        | KeyCode::Home | KeyCode::End | KeyCode::Left | KeyCode::Right => {
            DispatchTarget::AppMsg(on_event(TreeEvent::Navigate(key_event.code)))
        }
        // Enter toggles expansion
        KeyCode::Enter => {
            DispatchTarget::AppMsg(on_event(TreeEvent::Toggle))
        }
        _ => {
            // Unhandled key - pass through to global subscriptions
            DispatchTarget::PassThrough
        }
    })
}

/// Render TableTree element
#[allow(clippy::too_many_arguments)]
pub fn render_table_tree<Msg: Clone + Send + 'static>(
    frame: &mut Frame,
    
    registry: &mut InteractionRegistry<Msg>,
    focus_registry: &mut FocusRegistry<Msg>,
    _dropdown_registry: &mut DropdownRegistry<Msg>,
    focused_id: Option<&FocusId>,
    id: &FocusId,
    flattened_nodes: &[FlatTableNode],
    node_ids: &[String],
    selected: &Option<String>,
    scroll_offset: usize,
    column_widths: &[Constraint],
    column_headers: &[String],
    on_select: &Option<fn(String) -> Msg>,
    on_event: &Option<fn(TreeEvent) -> Msg>,
    on_focus: &Option<Msg>,
    on_blur: &Option<Msg>,
    on_render: &Option<fn(usize) -> Msg>,
    area: Rect,
    inside_panel: bool,
) {
    let theme = &crate::global_runtime_config().theme;
    // Call on_render with actual viewport height from renderer
    // Subtract 1 for header row
    let content_height = area.height.saturating_sub(1) as usize;
    if let Some(render_fn) = on_render {
        registry.add_render_message(render_fn(content_height));
    }

    // Register in focus registry
    if let Some(event_fn) = on_event {
        let on_key_handler = table_tree_on_key_event(*event_fn);
        focus_registry.register_focusable(FocusableInfo {
            id: id.clone(),
            rect: area,
            on_key: on_key_handler,
            on_focus: on_focus.clone(),
            on_blur: on_blur.clone(),
            inside_panel,
        });
    }

    // Check if this tree is focused
    let is_focused = focused_id == Some(id);

    // Calculate visible height (subtract header only, no borders)
    let visible_height = area.height.saturating_sub(1) as usize;

    // Virtual scrolling: only render visible rows
    // Bound start_idx to prevent panic when scroll_offset is stale (e.g., after clearing queue)
    let start_idx = scroll_offset.min(flattened_nodes.len());
    let end_idx = (start_idx + visible_height).min(flattened_nodes.len());

    // Build table rows from flattened nodes
    let rows: Vec<Row> = flattened_nodes[start_idx..end_idx]
        .iter()
        .map(|node| {
            let mut columns = node.columns.clone();

            // Apply tree indentation to first column
            if !columns.is_empty() {
                let indent = "  ".repeat(node.depth);

                // Add expansion indicator (▶ or ▼) for nodes with children
                // We can't easily check has_children here, so we'll use is_expanded as a proxy
                // Proper implementation would need to pass has_children flag
                let expansion_indicator = if node.depth > 0 {
                    "  " // Child nodes just get indentation
                } else {
                    if node.is_expanded {
                        "▼ "
                    } else {
                        "▶ "
                    }
                };

                columns[0] = format!("{}{}{}", indent, expansion_indicator, columns[0]);
            }

            // Convert to cells
            let cells: Vec<Cell> = columns.into_iter().map(Cell::from).collect();

            // Apply selection highlighting
            let mut row = Row::new(cells);
            if node.is_selected {
                row = row.style(Style::default().bg(theme.bg_surface));
            }

            row
        })
        .collect();

    // Create header row
    let header_cells: Vec<Cell> = column_headers
        .iter()
        .map(|h| Cell::from(h.as_str()))
        .collect();
    let header = Row::new(header_cells)
        .style(Style::default().fg(theme.accent_primary).bold())
        .height(1);

    // Create table widget without borders (parent panel handles that)
    let table = Table::new(rows, column_widths)
        .header(header);

    frame.render_widget(table, area);

    // Register click handlers for rows
    if let Some(on_select_fn) = on_select {
        // Calculate row height (1 per row + header)
        let row_area_start_y = area.y + 1; // Skip header
        for (idx, _node) in flattened_nodes[start_idx..end_idx].iter().enumerate() {
            let node_idx = start_idx + idx;
            if node_idx < node_ids.len() {
                let row_area = Rect {
                    x: area.x,
                    y: row_area_start_y + idx as u16,
                    width: area.width,
                    height: 1,
                };
                let node_id = node_ids[node_idx].clone();
                registry.register_click(row_area, on_select_fn(node_id));
            }
        }
    }

    // Render scrollbar if needed
    if flattened_nodes.len() > visible_height {
        let scrollbar_area = Rect {
            x: area.x + area.width - 1,
            y: area.y + 1, // Skip header
            width: 1,
            height: area.height.saturating_sub(1),
        };

        let total_content = flattened_nodes.len();
        let scrollbar_position = if total_content > visible_height {
            (scroll_offset as f32 / (total_content - visible_height) as f32
                * (scrollbar_area.height - 1) as f32) as u16
        } else {
            0
        };

        // Render scrollbar thumb
        if scrollbar_position < scrollbar_area.height {
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
}
