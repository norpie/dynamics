use crate::tui::widgets::{AutocompleteEvent, MultiSelectEvent, SelectEvent};
use ratatui::layout::Rect;

/// Type of dropdown callback
pub enum DropdownCallback<Msg> {
    Select(Option<fn(usize) -> Msg>),            // Select by index
    SelectEvent(Option<fn(SelectEvent) -> Msg>), // Select with unified event pattern
    Autocomplete(Option<fn(String) -> Msg>),     // Select by string value
    AutocompleteEvent(Option<fn(AutocompleteEvent) -> Msg>), // Autocomplete with unified event pattern
    MultiSelectEvent(Option<fn(MultiSelectEvent) -> Msg>), // MultiSelect with unified event pattern
}

/// Information about a dropdown that needs to be rendered as an overlay
pub struct DropdownInfo<Msg> {
    pub select_area: Rect,                // The area of the select widget
    pub options: Vec<String>,             // The dropdown options
    pub selected: Option<usize>,          // Selected index (None for autocomplete)
    pub highlight: usize,                 // Highlighted index
    pub on_select: DropdownCallback<Msg>, // Callback when option selected
}

/// Stores dropdowns to be rendered as overlays after main UI
pub struct DropdownRegistry<Msg> {
    dropdowns: Vec<DropdownInfo<Msg>>,
}

impl<Msg: Clone> DropdownRegistry<Msg> {
    pub fn new() -> Self {
        Self {
            dropdowns: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.dropdowns.clear();
    }

    pub fn register(&mut self, info: DropdownInfo<Msg>) {
        self.dropdowns.push(info);
    }

    pub fn dropdowns(&self) -> &[DropdownInfo<Msg>] {
        &self.dropdowns
    }
}
