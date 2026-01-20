use crate::tui::Element;
use crate::tui::element::FocusId;
use crate::tui::widgets::{FlatTableNode, TreeEvent};
use ratatui::layout::Constraint;

/// Builder for table tree elements
pub struct TableTreeBuilder<Msg> {
    pub(crate) id: FocusId,
    pub(crate) flattened_nodes: Vec<FlatTableNode>,
    pub(crate) node_ids: Vec<String>,
    pub(crate) selected: Option<String>,
    pub(crate) scroll_offset: usize,
    pub(crate) column_widths: Vec<Constraint>,
    pub(crate) column_headers: Vec<String>,
    pub(crate) on_select: Option<fn(String) -> Msg>,
    pub(crate) on_event: Option<fn(TreeEvent) -> Msg>,
    pub(crate) on_focus: Option<Msg>,
    pub(crate) on_blur: Option<Msg>,
    pub(crate) on_render: Option<fn(usize) -> Msg>,
}

impl<Msg> TableTreeBuilder<Msg> {
    pub fn on_select(mut self, msg: fn(String) -> Msg) -> Self {
        self.on_select = Some(msg);
        self
    }

    pub fn on_event(mut self, msg: fn(TreeEvent) -> Msg) -> Self {
        self.on_event = Some(msg);
        self
    }

    pub fn on_focus(mut self, msg: Msg) -> Self {
        self.on_focus = Some(msg);
        self
    }

    pub fn on_blur(mut self, msg: Msg) -> Self {
        self.on_blur = Some(msg);
        self
    }

    pub fn on_render(mut self, msg: fn(usize) -> Msg) -> Self {
        self.on_render = Some(msg);
        self
    }

    pub fn build(self) -> Element<Msg> {
        Element::TableTree {
            id: self.id,
            flattened_nodes: self.flattened_nodes,
            node_ids: self.node_ids,
            selected: self.selected,
            scroll_offset: self.scroll_offset,
            column_widths: self.column_widths,
            column_headers: self.column_headers,
            on_select: self.on_select,
            on_event: self.on_event,
            on_focus: self.on_focus,
            on_blur: self.on_blur,
            on_render: self.on_render,
        }
    }
}
