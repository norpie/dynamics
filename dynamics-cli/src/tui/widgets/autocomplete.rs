use crossterm::event::KeyCode;
use crate::tui::widgets::TextInputState;

/// Manages state for Autocomplete input widgets
/// Combines text input with fuzzy-matched dropdown suggestions
#[derive(Debug, Clone)]
pub struct AutocompleteState {
    /// Text input state (cursor, scroll)
    input_state: TextInputState,

    /// Whether dropdown is currently open
    is_open: bool,

    /// Index of highlighted option in dropdown
    highlight_index: usize,

    /// Filtered and scored options (option_text, score)
    filtered_options: Vec<(String, i64)>,

    /// Total count of available options (for validation)
    total_option_count: usize,
}

impl Default for AutocompleteState {
    fn default() -> Self {
        Self::new()
    }
}

impl AutocompleteState {
    /// Create a new AutocompleteState
    pub fn new() -> Self {
        Self {
            input_state: TextInputState::new(),
            is_open: false,
            highlight_index: 0,
            filtered_options: Vec::new(),
            total_option_count: 0,
        }
    }

    /// Get reference to text input state
    pub fn input_state(&self) -> &TextInputState {
        &self.input_state
    }

    /// Get mutable reference to text input state
    pub fn input_state_mut(&mut self) -> &mut TextInputState {
        &mut self.input_state
    }

    /// Set cursor to end of text (used after programmatically setting value)
    pub fn set_cursor_to_end(&mut self, text: &str) {
        self.input_state = TextInputState::new();
        // Use End key logic to move cursor to end
        self.input_state.handle_key(crossterm::event::KeyCode::End, text, None);
        // Update scroll to keep cursor visible at end
        self.input_state.update_scroll(Self::DEFAULT_VISIBLE_WIDTH, text);
    }

    /// Get whether dropdown is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Get currently highlighted index in dropdown
    pub fn highlighted(&self) -> usize {
        self.highlight_index
    }

    /// Get filtered options (top 100 by score)
    pub fn filtered_options(&self) -> Vec<String> {
        self.filtered_options.iter().map(|(opt, _)| opt.clone()).collect()
    }

    /// Update filtered options using fuzzy matching
    /// Automatically opens/closes dropdown based on results
    pub fn update_filtered_options(&mut self, input: &str, all_options: &[String]) {
        use fuzzy_matcher::FuzzyMatcher;
        use fuzzy_matcher::skim::SkimMatcherV2;

        self.total_option_count = all_options.len();

        if input.is_empty() {
            // No input - close dropdown
            self.filtered_options.clear();
            self.is_open = false;
            self.highlight_index = 0;
            return;
        }

        // Fuzzy match and score all options
        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<(String, i64)> = all_options
            .iter()
            .filter_map(|opt| {
                matcher.fuzzy_match(opt, input)
                    .map(|score| (opt.clone(), score))
            })
            .collect();

        // Sort by score descending (higher score = better match)
        scored.sort_by(|a, b| b.1.cmp(&a.1));

        // Take top 100 results (with scrolling, we can handle larger lists)
        self.filtered_options = scored.into_iter().take(100).collect();

        // Auto-open dropdown if we have results
        if !self.filtered_options.is_empty() {
            self.is_open = true;
            // Reset highlight to first option
            self.highlight_index = 0;
        } else {
            self.is_open = false;
            self.highlight_index = 0;
        }
    }

    /// Open the dropdown
    pub fn open(&mut self) {
        if !self.filtered_options.is_empty() {
            self.is_open = true;
            self.highlight_index = 0;
        }
    }

    /// Close the dropdown
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Navigate to next option in dropdown (wraps around)
    pub fn navigate_next(&mut self) {
        let count = self.filtered_options.len();
        if count > 0 {
            self.highlight_index = (self.highlight_index + 1) % count;
        }
    }

    /// Navigate to previous option in dropdown (wraps around)
    pub fn navigate_prev(&mut self) {
        let count = self.filtered_options.len();
        if count > 0 {
            self.highlight_index = if self.highlight_index == 0 {
                count - 1
            } else {
                self.highlight_index - 1
            };
        }
    }

    /// Get the currently highlighted option text
    pub fn get_highlighted_option(&self) -> Option<String> {
        if self.highlight_index < self.filtered_options.len() {
            Some(self.filtered_options[self.highlight_index].0.clone())
        } else {
            None
        }
    }

    /// Handle a key press in the text input
    /// Returns Some(new_value) if text changed, None otherwise
    pub fn handle_input_key(
        &mut self,
        key: KeyCode,
        current_value: &str,
        max_length: Option<usize>,
    ) -> Option<String> {
        let result = self.input_state.handle_key(key, current_value, max_length);
        // Update scroll to keep cursor visible
        // Use the new value if changed, otherwise current value
        let text = result.as_deref().unwrap_or(current_value);
        self.input_state.update_scroll(Self::DEFAULT_VISIBLE_WIDTH, text);
        result
    }

    /// Default visible width for scroll calculations
    /// This is an estimate - actual width depends on render area
    const DEFAULT_VISIBLE_WIDTH: usize = 40;

    /// Handle navigation in dropdown
    /// Returns true if the key was handled
    pub fn handle_dropdown_key(&mut self, key: KeyCode) -> bool {
        if !self.is_open {
            return false;
        }

        match key {
            KeyCode::Up => {
                self.navigate_prev();
                true
            }
            KeyCode::Down => {
                self.navigate_next();
                true
            }
            KeyCode::Esc => {
                self.close();
                true
            }
            _ => false,
        }
    }

    /// Handle navigation key when dropdown is open
    /// This is the unified handler for the new event pattern
    pub fn handle_navigate_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up => self.navigate_prev(),
            KeyCode::Down => self.navigate_next(),
            KeyCode::Enter => {
                // Selection is handled by the field, not the state
                // This method just handles navigation
            }
            KeyCode::Esc => self.close(),
            _ => {}
        }
    }
}
