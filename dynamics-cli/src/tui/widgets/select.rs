/// Manages state for Select/Dropdown widgets
#[derive(Debug, Clone)]
pub struct SelectState {
    selected_index: usize,
    is_open: bool,
    highlight_index: usize, // For keyboard navigation when dropdown is open
    option_count: usize,    // Cached for bounds checking
}

impl Default for SelectState {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectState {
    /// Create a new SelectState with first option selected
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            is_open: false,
            highlight_index: 0,
            option_count: 0,
        }
    }

    /// Create a SelectState with a specific option pre-selected
    pub fn with_selected(index: usize) -> Self {
        Self {
            selected_index: index,
            is_open: false,
            highlight_index: index,
            option_count: 0,
        }
    }

    /// Get currently selected index
    pub fn selected(&self) -> usize {
        self.selected_index
    }

    /// Get whether dropdown is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Get currently highlighted index (when dropdown is open)
    pub fn highlighted(&self) -> usize {
        self.highlight_index
    }

    /// Update the option count (called internally during rendering)
    pub fn update_option_count(&mut self, count: usize) {
        self.option_count = count;
        // Clamp selected and highlight to valid range
        if self.selected_index >= count && count > 0 {
            self.selected_index = count - 1;
        }
        if self.highlight_index >= count && count > 0 {
            self.highlight_index = count - 1;
        }
    }

    /// Open the dropdown
    pub fn open(&mut self) {
        self.is_open = true;
        // Start highlighting at selected index
        self.highlight_index = self.selected_index;
    }

    /// Close the dropdown
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Toggle dropdown open/closed
    pub fn toggle(&mut self) {
        if self.is_open {
            self.close();
        } else {
            self.open();
        }
    }

    /// Select the currently highlighted option (when dropdown is open)
    pub fn select_highlighted(&mut self) {
        self.selected_index = self.highlight_index;
        self.close();
    }

    /// Select an option by index
    pub fn select(&mut self, index: usize) {
        if index < self.option_count {
            self.selected_index = index;
            self.highlight_index = index;
        }
    }

    /// Clear selection (used when clearing the field)
    pub fn clear(&mut self) {
        self.selected_index = 0;
        self.highlight_index = 0;
        self.option_count = 0;
        self.is_open = false;
    }

    /// Navigate to next option (when dropdown is open)
    pub fn navigate_next(&mut self) {
        if self.option_count > 0 {
            self.highlight_index = (self.highlight_index + 1) % self.option_count;
        }
    }

    /// Navigate to previous option (when dropdown is open)
    pub fn navigate_prev(&mut self) {
        if self.option_count > 0 {
            self.highlight_index = if self.highlight_index == 0 {
                self.option_count - 1
            } else {
                self.highlight_index - 1
            };
        }
    }

    /// Cycle to next option directly (when dropdown is closed)
    pub fn cycle_next(&mut self) {
        if self.option_count > 0 {
            self.selected_index = (self.selected_index + 1) % self.option_count;
            self.highlight_index = self.selected_index;
        }
    }

    /// Cycle to previous option directly (when dropdown is closed)
    pub fn cycle_prev(&mut self) {
        if self.option_count > 0 {
            self.selected_index = if self.selected_index == 0 {
                self.option_count - 1
            } else {
                self.selected_index - 1
            };
            self.highlight_index = self.selected_index;
        }
    }

    /// Handle select event (unified event pattern)
    /// Returns Some(selected_index) on Select event, None otherwise
    pub fn handle_event(
        &mut self,
        event: crate::tui::widgets::events::SelectEvent,
    ) -> Option<usize> {
        use crate::tui::widgets::events::SelectEvent;
        use crossterm::event::KeyCode;

        match event {
            SelectEvent::Navigate(key) => {
                if self.is_open {
                    match key {
                        KeyCode::Up => self.navigate_prev(),
                        KeyCode::Down => self.navigate_next(),
                        _ => {}
                    }
                }
                None
            }
            SelectEvent::Select(index) => {
                self.select(index);
                Some(self.selected_index)
            }
            SelectEvent::Blur => {
                self.handle_blur();
                None
            }
        }
    }

    /// Handle blur event - close dropdown if open
    /// Call this when the select loses focus
    pub fn handle_blur(&mut self) {
        self.close();
    }
}
