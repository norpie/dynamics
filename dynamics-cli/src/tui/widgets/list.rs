use crossterm::event::KeyCode;
use std::collections::HashSet;
use crate::tui::{Element, Theme};

/// Trait for items that can be displayed in a list
pub trait ListItem {
    type Msg: Clone;

    /// Render this item as an Element
    /// is_selected: whether this item is the primary/anchor selection (cursor position)
    /// is_multi_selected: whether this item is in the multi-selection set
    /// is_hovered: whether the mouse is hovering over this item
    fn to_element(&self, is_selected: bool, is_multi_selected: bool, is_hovered: bool) -> Element<Self::Msg>;

    /// Optional: height in lines (default 1)
    fn height(&self) -> u16 {
        1
    }
}

/// Manages list selection and scrolling state
#[derive(Debug, Clone)]
pub struct ListState {
    selected: Option<usize>,
    scroll_offset: usize,
    scroll_off: usize, // Rows from edge before scrolling (like vim scrolloff)
    wrap_around: bool, // Wrap to bottom/top when reaching edges
    viewport_height: Option<usize>, // Last known viewport height from renderer

    // Multi-selection support
    multi_selected: HashSet<usize>, // Additional selected indices
    anchor_selection: Option<usize>, // Anchor for range selection (Shift+Arrow)
}

impl Default for ListState {
    fn default() -> Self {
        Self::new()
    }
}

impl ListState {
    /// Create a new ListState with no selection
    pub fn new() -> Self {
        Self {
            selected: None,
            scroll_offset: 0,
            scroll_off: 3,
            wrap_around: true,
            viewport_height: None,
            multi_selected: HashSet::new(),
            anchor_selection: None,
        }
    }

    /// Create a new ListState with first item selected
    pub fn with_selection() -> Self {
        Self {
            selected: Some(0),
            scroll_offset: 0,
            scroll_off: 3,
            wrap_around: true,
            viewport_height: None,
            multi_selected: HashSet::new(),
            anchor_selection: None,
        }
    }

    /// Set the viewport height (called by renderer with actual area height)
    pub fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = Some(height);
    }

    /// Set the scroll-off distance (rows from edge before scrolling)
    pub fn with_scroll_off(mut self, scroll_off: usize) -> Self {
        self.scroll_off = scroll_off;
        self
    }

    /// Enable or disable wrap-around navigation
    pub fn with_wrap_around(mut self, wrap_around: bool) -> Self {
        self.wrap_around = wrap_around;
        self
    }

    /// Get currently selected index
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Get current scroll offset
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Set selected index (useful for initialization)
    /// Note: This does NOT adjust scroll. Use select_and_scroll() if you need
    /// to ensure the selected item is visible.
    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index;
    }

    /// Set selected index and adjust scroll to ensure it's visible
    /// This should be used when programmatically changing selection.
    pub fn select_and_scroll(&mut self, index: Option<usize>, item_count: usize) {
        self.selected = index;
        if let Some(height) = self.viewport_height {
            self.update_scroll(height, item_count);
        }
    }

    /// Handle navigation key, returns true if handled
    /// Uses stored viewport_height if available, otherwise falls back to provided visible_height
    pub fn handle_key(&mut self, key: KeyCode, item_count: usize, visible_height: usize) -> bool {
        if item_count == 0 {
            return false;
        }

        // Use stored viewport_height if available, otherwise use provided value
        let height = self.viewport_height.unwrap_or(visible_height);

        match key {
            KeyCode::Up => {
                self.move_up(item_count, height);
                true
            }
            KeyCode::Down => {
                self.move_down(item_count, height);
                true
            }
            KeyCode::PageUp => {
                self.page_up(height, item_count);
                true
            }
            KeyCode::PageDown => {
                self.page_down(item_count, height);
                true
            }
            KeyCode::Home => {
                self.select_first(height, item_count);
                true
            }
            KeyCode::End => {
                self.select_last(item_count, height);
                true
            }
            _ => false,
        }
    }

    fn move_up(&mut self, item_count: usize, visible_height: usize) {
        if item_count == 0 {
            return;
        }

        if let Some(sel) = self.selected {
            if sel > 0 {
                self.selected = Some(sel - 1);
            } else if self.wrap_around {
                // At top, wrap to bottom
                self.selected = Some(item_count - 1);
            }
        } else {
            // No selection, select first
            self.selected = Some(0);
        }

        // Ensure the new selection is visible
        self.update_scroll(visible_height, item_count);
    }

    fn move_down(&mut self, item_count: usize, visible_height: usize) {
        if item_count == 0 {
            return;
        }

        if let Some(sel) = self.selected {
            if sel < item_count - 1 {
                self.selected = Some(sel + 1);
            } else if self.wrap_around {
                // At bottom, wrap to top
                self.selected = Some(0);
            }
        } else {
            // No selection, select first
            self.selected = Some(0);
        }

        // Ensure the new selection is visible
        self.update_scroll(visible_height, item_count);
    }

    fn page_up(&mut self, visible_height: usize, item_count: usize) {
        if let Some(sel) = self.selected {
            let new_sel = sel.saturating_sub(visible_height);
            self.selected = Some(new_sel);
        } else {
            self.selected = Some(0);
        }

        // Ensure the new selection is visible
        self.update_scroll(visible_height, item_count);
    }

    fn page_down(&mut self, item_count: usize, visible_height: usize) {
        if let Some(sel) = self.selected {
            let new_sel = (sel + visible_height).min(item_count - 1);
            self.selected = Some(new_sel);
        } else if item_count > 0 {
            self.selected = Some(0);
        }

        // Ensure the new selection is visible
        self.update_scroll(visible_height, item_count);
    }

    fn select_first(&mut self, visible_height: usize, item_count: usize) {
        self.selected = Some(0);
        // Ensure the selection is visible
        self.update_scroll(visible_height, item_count);
    }

    fn select_last(&mut self, item_count: usize, visible_height: usize) {
        if item_count > 0 {
            self.selected = Some(item_count - 1);
            // Ensure the selection is visible
            self.update_scroll(visible_height, item_count);
        }
    }

    /// Update scroll offset based on selection and visible height
    /// Called during rendering to ensure scrolloff is maintained
    pub fn update_scroll(&mut self, visible_height: usize, item_count: usize) {
        if let Some(sel) = self.selected {
            // Calculate ideal scroll range to keep selection visible with scrolloff
            let min_scroll = sel.saturating_sub(visible_height.saturating_sub(self.scroll_off + 1));
            let max_scroll = sel.saturating_sub(self.scroll_off);

            if self.scroll_offset < min_scroll {
                self.scroll_offset = min_scroll;
            } else if self.scroll_offset > max_scroll {
                self.scroll_offset = max_scroll;
            }

            // Clamp to valid range
            let max_offset = item_count.saturating_sub(visible_height);
            self.scroll_offset = self.scroll_offset.min(max_offset);
        }
    }

    // === Multi-selection methods ===

    /// Toggle multi-selection for a specific index (Space key)
    /// If the index is currently multi-selected, remove it. Otherwise, add it.
    pub fn toggle_multi_select(&mut self, index: usize) {
        if self.multi_selected.contains(&index) {
            self.multi_selected.remove(&index);
        } else {
            self.multi_selected.insert(index);
            // Set anchor for range selection
            self.anchor_selection = Some(index);
        }
    }

    /// Toggle multi-selection for the currently selected item
    pub fn toggle_multi_select_current(&mut self) {
        if let Some(current) = self.selected {
            self.toggle_multi_select(current);
        }
    }

    /// Select range from anchor to end_index (Shift+Arrow)
    /// Adds all indices between anchor and end to multi_selected
    pub fn select_range(&mut self, end_index: usize, item_count: usize) {
        let anchor = self.anchor_selection.or(self.selected);

        if let Some(anchor) = anchor {
            let (from, to) = if anchor <= end_index {
                (anchor, end_index)
            } else {
                (end_index, anchor)
            };

            // Add all indices in range to multi_selected
            for idx in from..=to.min(item_count.saturating_sub(1)) {
                self.multi_selected.insert(idx);
            }
        }

        // Update anchor to end position
        self.anchor_selection = Some(end_index);
    }

    /// Extend selection up (Shift+Up) - select range to previous item
    pub fn extend_selection_up(&mut self, item_count: usize, visible_height: usize) {
        if let Some(current) = self.selected {
            if current > 0 {
                let target = current - 1;
                self.select_range(target, item_count);
                self.selected = Some(target);
                self.update_scroll(visible_height, item_count);
            }
        }
    }

    /// Extend selection down (Shift+Down) - select range to next item
    pub fn extend_selection_down(&mut self, item_count: usize, visible_height: usize) {
        if let Some(current) = self.selected {
            if current + 1 < item_count {
                let target = current + 1;
                self.select_range(target, item_count);
                self.selected = Some(target);
                self.update_scroll(visible_height, item_count);
            }
        }
    }

    /// Clear all multi-selections (Ctrl+D or Esc)
    pub fn clear_multi_selection(&mut self) {
        self.multi_selected.clear();
        self.anchor_selection = None;
    }

    /// Select all items (Ctrl+A)
    pub fn select_all(&mut self, item_count: usize) {
        self.multi_selected = (0..item_count).collect();
        self.anchor_selection = Some(0);
    }

    /// Get all selected indices (primary selection + multi-selected)
    /// Returns a Vec with all unique selected indices
    pub fn all_selected(&self) -> Vec<usize> {
        let mut result = Vec::new();

        // Add primary selection first (if not in multi_selected)
        if let Some(primary) = self.selected {
            if !self.multi_selected.contains(&primary) {
                result.push(primary);
            }
        }

        // Add all multi-selected indices (sorted for consistency)
        let mut multi: Vec<_> = self.multi_selected.iter().copied().collect();
        multi.sort();
        result.extend(multi);

        result
    }

    /// Check if an index is in the multi-selection set
    pub fn is_multi_selected(&self, index: usize) -> bool {
        self.multi_selected.contains(&index)
    }

    /// Get count of multi-selected items (excludes primary selection)
    pub fn multi_select_count(&self) -> usize {
        self.multi_selected.len()
    }

    /// Get total selection count (primary + multi-selected, deduplicated)
    pub fn total_selection_count(&self) -> usize {
        let mut count = self.multi_selected.len();

        // Add 1 if primary selection exists and is not in multi_selected
        if let Some(primary) = self.selected {
            if !self.multi_selected.contains(&primary) {
                count += 1;
            }
        }

        count
    }

    /// Check if multi-selection is active (any items selected beyond primary)
    pub fn has_multi_selection(&self) -> bool {
        !self.multi_selected.is_empty()
    }

    /// Set multi-selection indices directly (for windowed/virtual scrolling)
    pub fn set_multi_selected(&mut self, indices: HashSet<usize>) {
        self.multi_selected = indices;
    }

    /// Create a windowed copy of multi-selection state, mapping global indices to windowed indices
    /// global_start is the start index of the window in the full list
    pub fn windowed_multi_selection(&self, global_start: usize, window_size: usize) -> HashSet<usize> {
        self.multi_selected
            .iter()
            .filter_map(|&global_idx| {
                if global_idx >= global_start && global_idx < global_start + window_size {
                    Some(global_idx - global_start)
                } else {
                    None
                }
            })
            .collect()
    }

    // === End multi-selection methods ===

    /// Handle list event (unified event pattern)
    /// Returns Some(selected_index) on Select event, None otherwise
    pub fn handle_event(&mut self, event: crate::tui::widgets::events::ListEvent, item_count: usize, visible_height: usize) -> Option<usize> {
        use crate::tui::widgets::events::ListEvent;

        match event {
            ListEvent::Navigate(key) => {
                self.handle_key(key, item_count, visible_height);
                None
            }
            ListEvent::Select => self.selected,
            ListEvent::ToggleMultiSelect => {
                self.toggle_multi_select_current();
                None
            }
            ListEvent::SelectAll => {
                self.select_all(item_count);
                None
            }
            ListEvent::ClearMultiSelection => {
                self.clear_multi_selection();
                None
            }
            ListEvent::ExtendSelectionUp => {
                self.extend_selection_up(item_count, visible_height);
                None
            }
            ListEvent::ExtendSelectionDown => {
                self.extend_selection_down(item_count, visible_height);
                None
            }
        }
    }
}
