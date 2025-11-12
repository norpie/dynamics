use crate::tui::command::Command;
use super::{AutocompleteState, TextInputState, SelectState, MultiSelectState};
use super::events::{AutocompleteEvent, TextInputEvent, SelectEvent, MultiSelectEvent};

/// Field that combines value + state for Autocomplete widget
#[derive(Clone, Default)]
pub struct AutocompleteField {
    pub value: String,
    pub state: AutocompleteState,
}

impl AutocompleteField {
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle autocomplete event and return command (usually None)
    /// Pass in the available options for filtering
    pub fn handle_event<Msg>(&mut self, event: AutocompleteEvent, options: &[String]) -> Command<Msg> {
        match event {
            AutocompleteEvent::Input(key) => {
                if let Some(new_value) = self.state.handle_input_key(key, &self.value, None) {
                    self.value = new_value;
                    self.state.update_filtered_options(&self.value, options);
                }
            }
            AutocompleteEvent::Navigate(key) => {
                use crossterm::event::KeyCode;

                // Handle Enter specially - select highlighted option
                if key == KeyCode::Enter {
                    if let Some(selected) = self.state.get_highlighted_option() {
                        self.value = selected;
                        self.state.close();
                        self.state.set_cursor_to_end(&self.value);
                    }
                } else {
                    self.state.handle_navigate_key(key);
                }
            }
            AutocompleteEvent::Select(selected) => {
                self.value = selected;
                self.state.close();
                self.state.set_cursor_to_end(&self.value);
            }
        }
        Command::None
    }

    /// Get current value
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Set value (useful for initialization)
    /// Cursor is positioned at the end of the value
    pub fn set_value(&mut self, value: String) {
        self.value = value;
        self.state.set_cursor_to_end(&self.value);
    }

    /// Check if dropdown is open
    pub fn is_open(&self) -> bool {
        self.state.is_open()
    }
}

/// Field that combines value + state for TextInput widget
#[derive(Clone, Default)]
pub struct TextInputField {
    pub value: String,
    pub state: TextInputState,
}

impl TextInputField {
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle text input event and return command (usually None unless Submit)
    /// Returns Some(value) on Submit, None otherwise
    pub fn handle_event(&mut self, event: TextInputEvent, max_length: Option<usize>) -> Option<String> {
        match event {
            TextInputEvent::Changed(key) => {
                if let Some(new_value) = self.state.handle_key(key, &self.value, max_length) {
                    self.value = new_value;
                }
                None
            }
            TextInputEvent::Submit => {
                Some(self.value.clone())
            }
        }
    }

    /// Get current value
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Set value (useful for initialization)
    /// Cursor is positioned at the end of the value
    pub fn set_value(&mut self, value: String) {
        self.value = value;
        self.state.set_cursor_to_end(&self.value);
    }
}

/// Field that combines value + state for Select widget
#[derive(Clone, Default)]
pub struct SelectField {
    selected_option: Option<String>,
    pub state: SelectState,
}

impl SelectField {
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle select event and update selected value
    /// Returns a SelectEvent::Select if an item was selected (for app notification)
    pub fn handle_event<Msg>(&mut self, event: SelectEvent, options: &[String]) -> (Command<Msg>, Option<SelectEvent>) {
        use crossterm::event::KeyCode;

        let mut selection_made = None;

        match event {
            SelectEvent::Navigate(key) => {
                if !self.state.is_open() {
                    // Closed: Enter/Space toggles open
                    match key {
                        KeyCode::Enter | KeyCode::Char(' ') => {
                            self.state.toggle();
                        }
                        _ => {}
                    }
                } else {
                    // Open: handle navigation
                    match key {
                        KeyCode::Up => self.state.navigate_prev(),
                        KeyCode::Down => self.state.navigate_next(),
                        KeyCode::Enter => {
                            // Select highlighted and close
                            self.state.select_highlighted();
                            let idx = self.state.selected();
                            if idx < options.len() {
                                self.selected_option = Some(options[idx].clone());
                                selection_made = Some(SelectEvent::Select(idx));
                            }
                        }
                        KeyCode::Esc => {
                            self.state.close();
                        }
                        _ => {}
                    }
                }
            }
            SelectEvent::Select(idx) => {
                self.state.select(idx);
                if idx < options.len() {
                    self.selected_option = Some(options[idx].clone());
                    selection_made = Some(SelectEvent::Select(idx));
                }
            }
            SelectEvent::Blur => {
                // Close dropdown when losing focus
                self.state.handle_blur();
            }
        }
        (Command::None, selection_made)
    }

    /// Get selected value as Option
    pub fn value(&self) -> Option<&str> {
        self.selected_option.as_deref()
    }

    /// Set selected value (useful for initialization)
    /// If value is None, also clears the visual state
    pub fn set_value(&mut self, value: Option<String>) {
        // Check if clearing before moving value
        let is_clearing = value.is_none();
        self.selected_option = value;
        // If clearing value, also clear the state
        if is_clearing {
            self.state.clear();
        }
    }

    /// Set selected value and update state index to match (requires options list)
    pub fn set_value_with_options(&mut self, value: Option<String>, options: &[String]) {
        self.selected_option = value.clone();

        // Update option count first (needed for select() to work)
        self.state.update_option_count(options.len());

        // Update state index to match
        if let Some(val) = value {
            if let Some(idx) = options.iter().position(|opt| opt == &val) {
                self.state.select(idx);
            }
        }
    }

    /// Check if dropdown is open
    pub fn is_open(&self) -> bool {
        self.state.is_open()
    }
}

/// Field that combines selected items + state for MultiSelect widget
#[derive(Clone, Default)]
pub struct MultiSelectField {
    pub selected_items: Vec<String>,
    pub search_input: String,  // Current search text
    pub state: MultiSelectState,
}

impl MultiSelectField {
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle multi-select event and return command (usually None)
    /// Pass in the available options for filtering
    pub fn handle_event<Msg>(&mut self, event: MultiSelectEvent, options: &[String]) -> Command<Msg> {
        match event {
            MultiSelectEvent::Input(key) => {
                // Handle input key and update search text
                if let Some(new_value) = self.state.input_state_mut().handle_key(key, &self.search_input, None) {
                    self.search_input = new_value.clone();
                    self.state.set_value(new_value.clone());
                    // Update filtered options
                    self.state.update_filtered_options(&new_value, options);
                }
            }
            MultiSelectEvent::Navigate(key) => {
                self.state.handle_navigate_key(key);
                // Sync selected_items with state
                self.selected_items = self.state.selected_items().to_vec();
                // Clear search input after selection
                if matches!(key, crossterm::event::KeyCode::Enter) {
                    self.search_input.clear();
                    self.state.clear_input();
                    self.state.update_filtered_options("", options);
                }
            }
            MultiSelectEvent::Toggle(item) => {
                self.state.toggle_item(&item);
                self.selected_items = self.state.selected_items().to_vec();
            }
            MultiSelectEvent::Remove(item) => {
                self.state.remove_item(&item);
                self.selected_items = self.state.selected_items().to_vec();
            }
            MultiSelectEvent::Clear => {
                self.state.clear_all();
                self.selected_items.clear();
            }
            MultiSelectEvent::Select(item) => {
                self.state.toggle_item(&item);
                self.selected_items = self.state.selected_items().to_vec();
                // Clear input after selection
                self.search_input.clear();
                self.state.clear_input();
                self.state.update_filtered_options("", options);
            }
        }
        Command::None
    }

    /// Get selected items
    pub fn selected_items(&self) -> &[String] {
        &self.selected_items
    }

    /// Set selected items (useful for initialization)
    pub fn set_selected_items(&mut self, items: Vec<String>) {
        self.selected_items = items.clone();
        self.state.set_selected_items(items);
    }

    /// Check if dropdown is open
    pub fn is_open(&self) -> bool {
        self.state.is_open()
    }

    /// Check if any items are selected
    pub fn has_selection(&self) -> bool {
        !self.selected_items.is_empty()
    }
}
