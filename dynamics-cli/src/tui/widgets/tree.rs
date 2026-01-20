use crate::tui::{Element, Theme};
use crossterm::event::KeyCode;
use std::collections::{HashMap, HashSet};

/// Trait for items that can be displayed in a tree
pub trait TreeItem: Clone {
    type Msg: Clone;

    /// Unique ID for this node (must be stable across frames)
    /// Recommended format: "{name}_{level}" for compatibility with old system
    fn id(&self) -> String;

    /// Check if this node has children
    fn has_children(&self) -> bool;

    /// Get children of this node (only called if has_children() is true)
    fn children(&self) -> Vec<Self>;

    /// Render this node as an Element
    /// depth: indentation level (0 = root)
    /// is_selected: whether this node is currently selected (primary/anchor)
    /// is_multi_selected: whether this node is in the multi-selection set
    /// is_expanded: whether this node is currently expanded
    fn to_element(
        &self,
        depth: usize,
        is_selected: bool,
        is_multi_selected: bool,
        is_expanded: bool,
    ) -> Element<Self::Msg>;
}

/// Trait for items that can be displayed in a table-style tree with columns
///
/// This extends TreeItem to support table rendering with proper column alignment
/// and borders between columns, ideal for queue management UIs.
pub trait TableTreeItem: TreeItem {
    /// Return column values for this row as strings
    ///
    /// Each String represents the content for one column.
    /// The first column will automatically have tree indentation applied.
    ///
    /// # Arguments
    /// * `depth` - indentation level (0 = root)
    /// * `is_selected` - whether this node is currently selected
    /// * `is_expanded` - whether this node is currently expanded
    fn to_table_columns(&self, depth: usize, is_selected: bool, is_expanded: bool) -> Vec<String>;

    /// Define column widths using ratatui Constraints
    ///
    /// This is a static method that returns the layout constraints for all columns.
    /// Example: vec![Constraint::Length(5), Constraint::Fill(1), Constraint::Length(10)]
    fn column_widths() -> Vec<ratatui::layout::Constraint>;

    /// Define column headers
    ///
    /// This is a static method that returns the header labels for each column.
    /// Example: vec!["Pri".to_string(), "Operation".to_string(), "Status".to_string()]
    fn column_headers() -> Vec<String>;
}

/// Manages tree expansion, selection, and scrolling state
#[derive(Debug, Clone)]
pub struct TreeState {
    // Core state
    expanded: HashSet<String>, // IDs of expanded nodes
    selected: Option<String>,  // Selected node ID (primary/anchor)
    scroll_offset: usize,
    scroll_off: usize,              // Scrolloff distance (vim-like)
    wrap_around: bool,              // Wrap to bottom/top when reaching edges
    viewport_height: Option<usize>, // Last known viewport height from renderer

    // Multi-selection support
    multi_selected: HashSet<String>, // Additional selected node IDs (for N:1 mappings)
    anchor_selection: Option<String>, // Anchor for range selection (Shift+Arrow)

    // Cached metadata for O(1) lookups (Approach 4 - Smart State)
    node_parents: HashMap<String, String>, // child_id → parent_id
    node_depths: HashMap<String, usize>,   // id → depth
    visible_order: Vec<String>,            // DFS order of visible nodes
    cache_valid: bool,                     // Whether cache needs rebuild
}

impl Default for TreeState {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeState {
    /// Create a new TreeState with no selection
    pub fn new() -> Self {
        Self {
            expanded: HashSet::new(),
            selected: None,
            scroll_offset: 0,
            scroll_off: 5,
            wrap_around: true,
            viewport_height: None,
            multi_selected: HashSet::new(),
            anchor_selection: None,
            node_parents: HashMap::new(),
            node_depths: HashMap::new(),
            visible_order: vec![],
            cache_valid: false,
        }
    }

    /// Set the viewport height (called by renderer with actual area height)
    pub fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = Some(height);
    }

    /// Invalidate the metadata cache (forces rebuild on next flatten)
    pub fn invalidate_cache(&mut self) {
        self.cache_valid = false;
    }

    /// Create a new TreeState with first node selected
    pub fn with_selection() -> Self {
        let mut state = Self::new();
        // Selection will be set when tree is first built
        state
    }

    /// Set the scroll-off distance (rows from edge before scrolling)
    pub fn with_scroll_off(mut self, scroll_off: usize) -> Self {
        self.scroll_off = scroll_off;
        self
    }

    /// Set whether to wrap around at edges (default: true)
    pub fn with_wrap_around(mut self, wrap_around: bool) -> Self {
        self.wrap_around = wrap_around;
        self
    }

    /// Get currently selected node ID
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Get current scroll offset
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Reset scroll offset to 0 (useful when filtering changes the item list)
    pub fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
    }

    /// Set selected node by ID
    /// Note: This does NOT adjust scroll. Use select_and_scroll() if you need
    /// to ensure the selected item is visible.
    pub fn select(&mut self, node_id: Option<String>) {
        self.selected = node_id;
    }

    /// Set selected node by ID and adjust scroll to ensure it's visible
    /// This should be used when programmatically changing selection.
    pub fn select_and_scroll(&mut self, node_id: Option<String>) {
        self.selected = node_id;
        if let Some(height) = self.viewport_height {
            self.update_scroll(height);
        }
    }

    /// Check if a node is expanded
    pub fn is_expanded(&self, node_id: &str) -> bool {
        self.expanded.contains(node_id)
    }

    /// Expand a node
    pub fn expand(&mut self, node_id: &str) {
        self.expanded.insert(node_id.to_string());
        self.cache_valid = false;
    }

    /// Collapse a node
    pub fn collapse(&mut self, node_id: &str) {
        self.expanded.remove(node_id);
        self.cache_valid = false;
    }

    /// Toggle expansion of a node
    pub fn toggle(&mut self, node_id: &str) {
        if self.expanded.contains(node_id) {
            self.collapse(node_id);
        } else {
            self.expand(node_id);
        }
    }

    /// Get parent of a node (O(1) with cache)
    pub fn parent_of(&self, node_id: &str) -> Option<&str> {
        self.node_parents.get(node_id).map(|s| s.as_str())
    }

    /// Get depth of a node (O(1) with cache)
    pub fn depth_of(&self, node_id: &str) -> Option<usize> {
        self.node_depths.get(node_id).copied()
    }

    /// Navigate to next visible node
    pub fn navigate_next(&mut self) {
        if let Some(current) = &self.selected {
            if let Some(pos) = self.visible_order.iter().position(|id| id == current) {
                if pos + 1 < self.visible_order.len() {
                    self.selected = Some(self.visible_order[pos + 1].clone());
                } else if self.wrap_around && !self.visible_order.is_empty() {
                    // At bottom, wrap to top
                    self.selected = Some(self.visible_order[0].clone());
                }
            } else {
                // Current selection not in visible order (stale state), select first
                if !self.visible_order.is_empty() {
                    self.selected = Some(self.visible_order[0].clone());
                }
            }
        } else if !self.visible_order.is_empty() {
            // No selection, select first
            self.selected = Some(self.visible_order[0].clone());
        }

        // Ensure the new selection is visible
        if let Some(height) = self.viewport_height {
            self.update_scroll(height);
        }
    }

    /// Navigate to previous visible node
    pub fn navigate_prev(&mut self) {
        if let Some(current) = &self.selected {
            if let Some(pos) = self.visible_order.iter().position(|id| id == current) {
                if pos > 0 {
                    self.selected = Some(self.visible_order[pos - 1].clone());
                } else if self.wrap_around && !self.visible_order.is_empty() {
                    // At top, wrap to bottom
                    self.selected = Some(self.visible_order[self.visible_order.len() - 1].clone());
                }
            } else {
                // Current selection not in visible order (stale state), select first
                if !self.visible_order.is_empty() {
                    self.selected = Some(self.visible_order[0].clone());
                }
            }
        } else if !self.visible_order.is_empty() {
            // No selection, select first
            self.selected = Some(self.visible_order[0].clone());
        }

        // Ensure the new selection is visible
        if let Some(height) = self.viewport_height {
            self.update_scroll(height);
        }
    }

    /// Navigate to parent node
    pub fn navigate_to_parent(&mut self) {
        if let Some(current) = &self.selected {
            if let Some(parent) = self.parent_of(current) {
                self.selected = Some(parent.to_string());
            }
        }

        // Ensure the new selection is visible
        if let Some(height) = self.viewport_height {
            self.update_scroll(height);
        }
    }

    // === Multi-selection methods ===

    /// Toggle multi-selection for a specific node (Space key)
    /// If the node is currently multi-selected, remove it. Otherwise, add it.
    /// This does NOT affect the primary selection (anchor).
    pub fn toggle_multi_select(&mut self, node_id: String) {
        if self.multi_selected.contains(&node_id) {
            self.multi_selected.remove(&node_id);
        } else {
            self.multi_selected.insert(node_id.clone());
            // Set anchor for range selection
            self.anchor_selection = Some(node_id);
        }
    }

    /// Toggle multi-selection for the currently selected (navigated) node
    pub fn toggle_multi_select_current(&mut self) {
        if let Some(current) = self.selected.clone() {
            log::debug!("Toggling multi-select for node: {}", current);
            self.toggle_multi_select(current);
            log::debug!("Multi-selected nodes: {:?}", self.multi_selected);
        } else {
            log::warn!("No node selected to toggle multi-select");
        }
    }

    /// Select range from anchor to end_node_id (Shift+Arrow)
    /// Adds all nodes between anchor and end to multi_selected
    pub fn select_range(&mut self, end_node_id: String) {
        let anchor = self
            .anchor_selection
            .clone()
            .or_else(|| self.selected.clone());

        if let Some(anchor) = anchor {
            // Find indices in visible order
            let start_idx = self.visible_order.iter().position(|id| id == &anchor);
            let end_idx = self.visible_order.iter().position(|id| id == &end_node_id);

            if let (Some(start), Some(end)) = (start_idx, end_idx) {
                let (from, to) = if start <= end {
                    (start, end)
                } else {
                    (end, start)
                };

                // Add all nodes in range to multi_selected
                for idx in from..=to {
                    self.multi_selected.insert(self.visible_order[idx].clone());
                }
            }
        }

        // Update anchor to end position
        self.anchor_selection = Some(end_node_id);
    }

    /// Extend selection up (Shift+Up) - select range to previous node
    pub fn extend_selection_up(&mut self) {
        if let Some(current) = &self.selected.clone() {
            if let Some(pos) = self.visible_order.iter().position(|id| id == current) {
                if pos > 0 {
                    let target = self.visible_order[pos - 1].clone();
                    self.select_range(target.clone());
                    self.selected = Some(target); // Move cursor

                    // Ensure the new selection is visible
                    if let Some(height) = self.viewport_height {
                        self.update_scroll(height);
                    }
                }
            }
        }
    }

    /// Extend selection down (Shift+Down) - select range to next node
    pub fn extend_selection_down(&mut self) {
        if let Some(current) = &self.selected.clone() {
            if let Some(pos) = self.visible_order.iter().position(|id| id == current) {
                if pos + 1 < self.visible_order.len() {
                    let target = self.visible_order[pos + 1].clone();
                    self.select_range(target.clone());
                    self.selected = Some(target); // Move cursor

                    // Ensure the new selection is visible
                    if let Some(height) = self.viewport_height {
                        self.update_scroll(height);
                    }
                }
            }
        }
    }

    /// Clear all multi-selections (Ctrl+D or Esc)
    pub fn clear_multi_selection(&mut self) {
        self.multi_selected.clear();
        self.anchor_selection = None;
    }

    /// Select all visible nodes (Ctrl+A)
    pub fn select_all_visible(&mut self) {
        self.multi_selected = self.visible_order.iter().cloned().collect();
        if let Some(first) = self.visible_order.first() {
            self.anchor_selection = Some(first.clone());
        }
    }

    /// Get all selected node IDs (primary selection + multi-selected)
    /// Returns a Vec with all unique selected nodes
    pub fn all_selected(&self) -> Vec<String> {
        let mut result = Vec::new();

        // Add primary selection first (if not in multi_selected)
        if let Some(primary) = &self.selected {
            if !self.multi_selected.contains(primary) {
                result.push(primary.clone());
            }
        }

        // Add all multi-selected nodes
        result.extend(self.multi_selected.iter().cloned());

        result
    }

    /// Check if a node is in the multi-selection set
    pub fn is_multi_selected(&self, node_id: &str) -> bool {
        self.multi_selected.contains(node_id)
    }

    /// Get count of multi-selected items (excludes primary selection)
    pub fn multi_select_count(&self) -> usize {
        self.multi_selected.len()
    }

    /// Get total selection count (primary + multi-selected, deduplicated)
    pub fn total_selection_count(&self) -> usize {
        let mut count = self.multi_selected.len();

        // Add 1 if primary selection exists and is not in multi_selected
        if let Some(primary) = &self.selected {
            if !self.multi_selected.contains(primary) {
                count += 1;
            }
        }

        count
    }

    // === End multi-selection methods ===

    /// Handle keyboard navigation (returns true if handled)
    pub fn handle_key(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Up => {
                self.navigate_prev();
                true
            }
            KeyCode::Down => {
                self.navigate_next();
                true
            }
            KeyCode::Right => {
                // Expand selected node
                if let Some(id) = &self.selected.clone() {
                    if !self.is_expanded(id) {
                        self.toggle(id);
                    }
                }
                true
            }
            KeyCode::Left => {
                // Collapse or jump to parent
                if let Some(id) = &self.selected.clone() {
                    if self.is_expanded(id) {
                        self.toggle(id);
                    } else {
                        self.navigate_to_parent();
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Update scroll offset based on selection and visible height
    pub fn update_scroll(&mut self, visible_height: usize) {
        if let Some(selected) = &self.selected {
            // Find index in visible order
            if let Some(sel_idx) = self.visible_order.iter().position(|id| id == selected) {
                let item_count = self.visible_order.len();

                // Don't scroll if all items fit on screen
                if item_count <= visible_height {
                    self.scroll_offset = 0;
                    return;
                }

                // Calculate ideal scroll range to keep selection visible with scrolloff
                let min_scroll =
                    sel_idx.saturating_sub(visible_height.saturating_sub(self.scroll_off + 1));
                let max_scroll = sel_idx.saturating_sub(self.scroll_off);

                if self.scroll_offset < min_scroll {
                    self.scroll_offset = min_scroll;
                } else if self.scroll_offset > max_scroll {
                    self.scroll_offset = max_scroll;
                }

                // Final clamp to valid range (prevents empty lines at bottom)
                let max_offset = item_count.saturating_sub(visible_height);
                self.scroll_offset = self.scroll_offset.min(max_offset);
            }
        }
    }

    /// Handle tree event (unified event pattern)
    /// Returns Some(selected_id) on Toggle event, None otherwise
    pub fn handle_event(
        &mut self,
        event: crate::tui::widgets::events::TreeEvent,
    ) -> Option<String> {
        use crate::tui::widgets::events::TreeEvent;

        log::debug!("TreeState::handle_event: {:?}", event);

        match event {
            TreeEvent::Navigate(key) => {
                self.handle_key(key);
                None
            }
            TreeEvent::Toggle => {
                // Toggle current selection
                if let Some(id) = self.selected.clone() {
                    self.toggle(&id);
                }
                None
            }
            TreeEvent::ToggleMultiSelect => {
                log::debug!("Handling ToggleMultiSelect event");
                self.toggle_multi_select_current();
                None
            }
            TreeEvent::SelectAll => {
                log::debug!("Handling SelectAll event");
                self.select_all_visible();
                log::debug!("Selected {} nodes", self.multi_selected.len());
                None
            }
            TreeEvent::ClearMultiSelection => {
                log::debug!("Handling ClearMultiSelection event");
                self.clear_multi_selection();
                None
            }
            TreeEvent::ExtendSelectionUp => {
                log::debug!("Handling ExtendSelectionUp event");
                self.extend_selection_up();
                None
            }
            TreeEvent::ExtendSelectionDown => {
                log::debug!("Handling ExtendSelectionDown event");
                self.extend_selection_down();
                None
            }
        }
    }

    /// Rebuild metadata cache from tree structure
    /// This is called internally when cache is invalid
    pub(crate) fn rebuild_metadata<T: TreeItem>(&mut self, root_items: &[T]) {
        self.node_parents.clear();
        self.node_depths.clear();
        self.visible_order.clear();

        for item in root_items {
            self.build_metadata_recursive(item, None, 0);
        }

        self.cache_valid = true;

        // Validate current selection - clear if it no longer exists
        if let Some(selected) = &self.selected {
            if !self.visible_order.contains(selected) {
                // Selected item no longer exists in tree, clear selection
                self.selected = None;
            }
        }

        // If no selection and there are items, select first
        if self.selected.is_none() && !self.visible_order.is_empty() {
            self.selected = Some(self.visible_order[0].clone());
        }
    }

    fn build_metadata_recursive<T: TreeItem>(
        &mut self,
        item: &T,
        parent_id: Option<String>,
        depth: usize,
    ) {
        let id = item.id();

        // Record parent relationship
        if let Some(parent) = parent_id {
            self.node_parents.insert(id.clone(), parent);
        }

        // Record depth
        self.node_depths.insert(id.clone(), depth);

        // Add to visible order
        self.visible_order.push(id.clone());

        // Recursively process children if expanded
        if self.is_expanded(&id) && item.has_children() {
            for child in item.children() {
                self.build_metadata_recursive(&child, Some(id.clone()), depth + 1);
            }
        }
    }
}

/// Internal structure for flattened tree nodes
pub(crate) struct FlatNode<Msg> {
    pub id: String,
    pub element: Element<Msg>,
    pub depth: usize,
}

/// Internal structure for flattened table tree nodes
#[derive(Clone)]
pub struct FlatTableNode {
    pub id: String,
    pub columns: Vec<String>,
    pub depth: usize,
    pub is_selected: bool,
    pub is_expanded: bool,
}

/// Flatten tree into displayable nodes based on expansion state
pub(crate) fn flatten_tree<T: TreeItem>(
    root_items: &[T],
    state: &mut TreeState,
) -> Vec<FlatNode<T::Msg>> {
    // Rebuild metadata cache if invalid
    if !state.cache_valid {
        state.rebuild_metadata(root_items);
    }

    let mut result = vec![];
    for item in root_items {
        flatten_recursive(item, state, 0, &mut result);
    }
    result
}

fn flatten_recursive<T: TreeItem>(
    item: &T,
    state: &TreeState,
    depth: usize,
    result: &mut Vec<FlatNode<T::Msg>>,
) {
    let id = item.id();
    let is_expanded = state.is_expanded(&id);
    let is_selected = state.selected() == Some(&id);
    let is_multi_selected = state.is_multi_selected(&id);
    let has_children = item.has_children();

    // Render node (delegates to TreeItem::to_element)
    let element = item.to_element(depth, is_selected, is_multi_selected, is_expanded);

    result.push(FlatNode {
        id: id.clone(),
        element,
        depth,
    });

    // Recursively flatten children if expanded
    if is_expanded && has_children {
        for child in item.children() {
            flatten_recursive(&child, state, depth + 1, result);
        }
    }
}

/// Flatten table tree into displayable rows based on expansion state
pub(crate) fn flatten_table_tree<T: TableTreeItem>(
    root_items: &[T],
    state: &mut TreeState,
) -> Vec<FlatTableNode> {
    let start = std::time::Instant::now();

    // Rebuild metadata cache if invalid
    if !state.cache_valid {
        state.rebuild_metadata(root_items);
    }

    let mut result = vec![];
    for item in root_items {
        flatten_table_recursive(item, state, 0, &mut result);
    }

    let elapsed = start.elapsed();
    if elapsed.as_micros() > 1000 {
        log::warn!(
            "PERF flatten_table_tree: {}us for {} items",
            elapsed.as_micros(),
            result.len()
        );
    }

    result
}

fn flatten_table_recursive<T: TableTreeItem>(
    item: &T,
    state: &TreeState,
    depth: usize,
    result: &mut Vec<FlatTableNode>,
) {
    let id = item.id();
    let is_expanded = state.is_expanded(&id);
    let is_selected = state.selected() == Some(&id);
    let has_children = item.has_children();

    // Get column data from item
    let columns = item.to_table_columns(depth, is_selected, is_expanded);

    result.push(FlatTableNode {
        id: id.clone(),
        columns,
        depth,
        is_selected,
        is_expanded,
    });

    // Recursively flatten children if expanded
    if is_expanded && has_children {
        for child in item.children() {
            flatten_table_recursive(&child, state, depth + 1, result);
        }
    }
}
