use crate::tui::Element;
use crate::tui::element::FocusId;

/// Builder for list elements
pub struct ListBuilder<Msg> {
    pub(crate) id: FocusId,
    pub(crate) items: Vec<Element<Msg>>,
    pub(crate) selected: Option<usize>,
    pub(crate) scroll_offset: usize,
    pub(crate) on_select: Option<fn(usize) -> Msg>,
    pub(crate) on_activate: Option<fn(usize) -> Msg>,
    pub(crate) on_navigate: Option<fn(crossterm::event::KeyCode) -> Msg>,
    pub(crate) on_focus: Option<Msg>,
    pub(crate) on_blur: Option<Msg>,
    pub(crate) on_render: Option<fn(usize) -> Msg>,
}

impl<Msg> ListBuilder<Msg> {
    pub fn on_select(mut self, msg: fn(usize) -> Msg) -> Self {
        self.on_select = Some(msg);
        self
    }

    pub fn on_activate(mut self, msg: fn(usize) -> Msg) -> Self {
        self.on_activate = Some(msg);
        self
    }

    pub fn on_navigate(mut self, msg: fn(crossterm::event::KeyCode) -> Msg) -> Self {
        self.on_navigate = Some(msg);
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

    /// Override the scroll offset (useful for virtual scrolling)
    pub fn scroll_offset(mut self, offset: usize) -> Self {
        self.scroll_offset = offset;
        self
    }

    pub fn build(self) -> Element<Msg> {
        Element::List {
            id: self.id,
            items: self.items,
            selected: self.selected,
            scroll_offset: self.scroll_offset,
            on_select: self.on_select,
            on_activate: self.on_activate,
            on_navigate: self.on_navigate,
            on_focus: self.on_focus,
            on_blur: self.on_blur,
            on_render: self.on_render,
        }
    }
}
