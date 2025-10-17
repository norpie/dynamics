# NavigableState: Unified 2D Navigation

**Prerequisites:** [Component System](../02-building-ui/components.md)

## Problem

Current navigation is inconsistent:
- List/Tree: 1D vertical only
- Table: needs 2D (row + column)
- Scrollable: has horizontal but disconnected from navigation

## Solution

Single `NavigableState` supporting both 1D and 2D navigation:

```rust
pub struct NavigableState {
    // Selection (None = nothing selected)
    selected_row: Option<usize>,
    selected_col: Option<usize>,  // None for 1D components

    // Scrolling (0-based offsets)
    scroll_row: usize,
    scroll_col: usize,

    // Scrolloff (vim-style, lines from edge before scrolling)
    scroll_off: usize,

    // Viewport dimensions (set by framework during render)
    viewport_rows: usize,
    viewport_cols: usize,
}
```

## Constructors

```rust
impl NavigableState {
    // 1D constructor (List, Tree, FileBrowser)
    pub fn new_1d() -> Self {
        Self {
            selected_row: None,
            selected_col: None,  // Always None for 1D
            scroll_off: 5,
            // ... other fields
        }
    }

    // 2D constructor (Table, Grid)
    pub fn new_2d() -> Self {
        Self {
            selected_row: None,
            selected_col: Some(0),  // Column matters for 2D
            scroll_off: 5,
            // ... other fields
        }
    }
}
```

## Navigation Methods

### Vertical Navigation

```rust
pub fn navigate_up(&mut self, row_count: usize);
pub fn navigate_down(&mut self, row_count: usize);
pub fn navigate_page_up(&mut self, row_count: usize);
pub fn navigate_page_down(&mut self, row_count: usize);
```

### Horizontal Navigation (2D only)

```rust
pub fn navigate_left(&mut self, col_count: usize);
pub fn navigate_right(&mut self, col_count: usize);
pub fn navigate_home(&mut self);  // Column 0
pub fn navigate_end(&mut self, col_count: usize);  // Last column
```

### Accessors

```rust
// For 1D components
pub fn selected_index(&self) -> Option<usize>;

// For 2D components
pub fn selected_cell(&self) -> Option<(usize, usize)>;

pub fn scroll_offset(&self) -> (usize, usize);
```

## Scrolloff Logic (vim-style)

Keeps selection N rows from edge before scrolling:

```rust
fn adjust_scroll_vertical(&mut self, total_rows: usize) {
    // Don't scroll if all rows fit
    if total_rows <= self.viewport_rows {
        self.scroll_row = 0;
        return;
    }

    // Keep selection scroll_off rows from edge
    let min_scroll = row.saturating_sub(
        self.viewport_rows.saturating_sub(self.scroll_off + 1)
    );
    let max_scroll = row.saturating_sub(self.scroll_off);

    // Adjust scroll if selection too close to edge
    // ...
}
```

## Usage Examples

See [Component System](../02-building-ui/components.md) for ListState, TableState, and TreeState composition examples.

**See Also:**
- [Component System](../02-building-ui/components.md) - Component composition patterns
- [Navigation](../04-user-interaction/navigation.md) - Tab/arrow key navigation

---

**Next:** Explore [Events & Queues](events-and-queues.md) or [Background Work](background-work.md).
