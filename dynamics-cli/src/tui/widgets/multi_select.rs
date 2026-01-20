use crate::tui::widgets::TextInputState;
use crossterm::event::KeyCode;

/// Manages state for multi-select input widgets
/// Combines text input for filtering with a list of selected items and dropdown suggestions
#[derive(Debug, Clone)]
pub struct MultiSelectState {
    /// Current input text value
    value: String,

    /// Text input state for search/filter (cursor, scroll)
    input_state: TextInputState,

    /// Currently selected items
    selected_items: Vec<String>,

    /// Whether dropdown is currently open
    is_open: bool,

    /// Index of highlighted option in dropdown
    highlight_index: usize,

    /// Filtered and scored options (option_text, score)
    filtered_options: Vec<(String, i64)>,

    /// Total count of available options (for validation)
    total_option_count: usize,
}

impl Default for MultiSelectState {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiSelectState {
    /// Create a new MultiSelectState
    pub fn new() -> Self {
        Self {
            value: String::new(),
            input_state: TextInputState::new(),
            selected_items: Vec::new(),
            is_open: false,
            highlight_index: 0,
            filtered_options: Vec::new(),
            total_option_count: 0,
        }
    }

    /// Get current input value
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Set input value and update cursor
    pub fn set_value(&mut self, value: String) {
        self.value = value;
        self.input_state.set_cursor_to_end(&self.value);
    }

    /// Get reference to text input state
    pub fn input_state(&self) -> &TextInputState {
        &self.input_state
    }

    /// Get mutable reference to text input state
    pub fn input_state_mut(&mut self) -> &mut TextInputState {
        &mut self.input_state
    }

    /// Get selected items
    pub fn selected_items(&self) -> &[String] {
        &self.selected_items
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
        self.filtered_options
            .iter()
            .map(|(opt, _)| opt.clone())
            .collect()
    }

    /// Clear search input
    pub fn clear_input(&mut self) {
        self.value = String::new();
        self.input_state = TextInputState::new();
    }

    /// Update filtered options using fuzzy matching
    /// Automatically opens/closes dropdown based on results
    /// Excludes already selected items from the dropdown
    pub fn update_filtered_options(&mut self, input: &str, all_options: &[String]) {
        use fuzzy_matcher::FuzzyMatcher;
        use fuzzy_matcher::skim::SkimMatcherV2;

        self.total_option_count = all_options.len();

        if input.is_empty() {
            // Show unselected options when input is empty
            self.filtered_options = all_options
                .iter()
                .filter(|opt| !self.selected_items.contains(opt))
                .map(|opt| (opt.clone(), 0))
                .take(100)
                .collect();
            self.is_open = !self.filtered_options.is_empty();
            self.highlight_index = 0;
            return;
        }

        // Fuzzy match
        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<(String, i64)> = all_options
            .iter()
            .filter(|opt| !self.selected_items.contains(opt))
            .filter_map(|opt| {
                matcher
                    .fuzzy_match(opt, input)
                    .map(|score| (opt.clone(), score))
            })
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.1.cmp(&a.1));

        // Take top 100
        self.filtered_options = scored.into_iter().take(100).collect();

        // Auto-open/close dropdown
        self.is_open = !self.filtered_options.is_empty();
        self.highlight_index = 0;
    }

    /// Handle keyboard navigation in dropdown
    /// Returns true if key was handled
    pub fn handle_navigate_key(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Up => {
                if !self.filtered_options.is_empty() {
                    self.highlight_index = self.highlight_index.saturating_sub(1);
                }
                true
            }
            KeyCode::Down => {
                if !self.filtered_options.is_empty()
                    && self.highlight_index < self.filtered_options.len() - 1
                {
                    self.highlight_index += 1;
                }
                true
            }
            KeyCode::Esc => {
                self.is_open = false;
                true
            }
            KeyCode::Enter => {
                // Toggle highlighted item
                if let Some(option) = self.get_highlighted_option() {
                    self.toggle_item(&option);
                    // Clear input after selection
                    self.input_state = TextInputState::new();
                }
                true
            }
            _ => false,
        }
    }

    /// Get the currently highlighted option (if any)
    pub fn get_highlighted_option(&self) -> Option<String> {
        self.filtered_options
            .get(self.highlight_index)
            .map(|(opt, _)| opt.clone())
    }

    /// Toggle an item (add if not present, remove if present)
    pub fn toggle_item(&mut self, item: &str) {
        if let Some(pos) = self.selected_items.iter().position(|x| x == item) {
            self.selected_items.remove(pos);
        } else {
            self.selected_items.push(item.to_string());
        }
    }

    /// Add an item to selected items (if not already present)
    pub fn add_item(&mut self, item: String) {
        if !self.selected_items.contains(&item) {
            self.selected_items.push(item);
        }
    }

    /// Remove an item from selected items
    pub fn remove_item(&mut self, item: &str) {
        self.selected_items.retain(|x| x != item);
    }

    /// Clear all selected items
    pub fn clear_all(&mut self) {
        self.selected_items.clear();
    }

    /// Set selected items (replaces current selection)
    pub fn set_selected_items(&mut self, items: Vec<String>) {
        self.selected_items = items;
    }
}
