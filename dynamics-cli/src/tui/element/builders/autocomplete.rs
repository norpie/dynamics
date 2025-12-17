use crate::tui::Element;
use crate::tui::element::FocusId;
use crate::tui::widgets::AutocompleteEvent;

/// Builder for autocomplete elements
pub struct AutocompleteBuilder<Msg> {
    pub(crate) id: FocusId,
    pub(crate) all_options: Vec<String>,
    pub(crate) current_input: String,
    pub(crate) cursor_pos: usize,
    pub(crate) scroll_offset: usize,
    pub(crate) placeholder: Option<String>,
    pub(crate) is_open: bool,
    pub(crate) filtered_options: Vec<String>,
    pub(crate) highlight: usize,
    pub(crate) on_input: Option<fn(crossterm::event::KeyCode) -> Msg>,
    pub(crate) on_select: Option<fn(String) -> Msg>,
    pub(crate) on_navigate: Option<fn(crossterm::event::KeyCode) -> Msg>,
    pub(crate) on_event: Option<fn(AutocompleteEvent) -> Msg>,
    pub(crate) on_focus: Option<Msg>,
    pub(crate) on_blur: Option<Msg>,
}

impl<Msg> AutocompleteBuilder<Msg> {
    /// Set placeholder text when input is empty
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Set callback for text input changes
    pub fn on_input(mut self, msg: fn(crossterm::event::KeyCode) -> Msg) -> Self {
        self.on_input = Some(msg);
        self
    }

    /// Set callback when option is selected from dropdown
    pub fn on_select(mut self, msg: fn(String) -> Msg) -> Self {
        self.on_select = Some(msg);
        self
    }

    /// Set callback for dropdown navigation
    pub fn on_navigate(mut self, msg: fn(crossterm::event::KeyCode) -> Msg) -> Self {
        self.on_navigate = Some(msg);
        self
    }

    /// Set unified event callback (new event pattern)
    /// This replaces on_input, on_select, and on_navigate
    pub fn on_event(mut self, msg: fn(AutocompleteEvent) -> Msg) -> Self {
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
        Element::Autocomplete {
            id: self.id,
            all_options: self.all_options,
            current_input: self.current_input,
            cursor_pos: self.cursor_pos,
            scroll_offset: self.scroll_offset,
            placeholder: self.placeholder,
            is_open: self.is_open,
            filtered_options: self.filtered_options,
            highlight: self.highlight,
            on_input: self.on_input,
            on_select: self.on_select,
            on_navigate: self.on_navigate,
            on_event: self.on_event,
            on_focus: self.on_focus,
            on_blur: self.on_blur,
        }
    }
}
