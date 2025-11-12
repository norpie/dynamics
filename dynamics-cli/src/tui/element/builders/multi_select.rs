use crate::tui::Element;
use crate::tui::element::FocusId;
use crate::tui::widgets::MultiSelectEvent;

/// Builder for multi-select elements
pub struct MultiSelectBuilder<Msg> {
    pub(crate) id: FocusId,
    pub(crate) all_options: Vec<String>,
    pub(crate) selected_items: Vec<String>,
    pub(crate) search_input: String,
    pub(crate) placeholder: Option<String>,
    pub(crate) is_open: bool,
    pub(crate) filtered_options: Vec<String>,
    pub(crate) highlight: usize,
    pub(crate) on_event: Option<fn(MultiSelectEvent) -> Msg>,
    pub(crate) on_focus: Option<Msg>,
    pub(crate) on_blur: Option<Msg>,
}

impl<Msg> MultiSelectBuilder<Msg> {
    /// Set placeholder text when input is empty
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Set unified event callback
    pub fn on_event(mut self, msg: fn(MultiSelectEvent) -> Msg) -> Self {
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

    pub fn build(self) -> Element<Msg> {
        Element::MultiSelect {
            id: self.id,
            all_options: self.all_options,
            selected_items: self.selected_items,
            search_input: self.search_input,
            placeholder: self.placeholder,
            is_open: self.is_open,
            filtered_options: self.filtered_options,
            highlight: self.highlight,
            on_event: self.on_event,
            on_focus: self.on_focus,
            on_blur: self.on_blur,
        }
    }
}
