# Component System

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md)

## Terminology

**Unified "Component" naming** - no more "widget" vs "element" confusion:

- **`Component<Msg>`** - UI tree enum (internal to framework)
- **`XxxState`** - Persistent component state (ButtonState, ListState, etc.)
- **"Component"** in all documentation (not "widget" or "element")

**Users never see "Component" directly:**
```rust
fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    vec![Layer::fill(panel("Items", |ui| {
        ui.list(&mut self.list_state, &self.items);  // Clean API
        ui.button("Save");
    }))]
}
```

## Component State Composition

Components compose `NavigableState` for consistent navigation behavior:

```rust
// List (1D - only vertical navigation)
pub struct ListState {
    nav: NavigableState,
    last_click: Option<(usize, Instant)>,
}

impl ListState {
    pub fn new() -> Self {
        Self {
            nav: NavigableState::new_1d(),
            last_click: None,
        }
    }

    // Delegate navigation to shared state
    pub fn navigate_up(&mut self, count: usize) {
        self.nav.navigate_up(count);
    }

    pub fn selected(&self) -> Option<usize> {
        self.nav.selected_index()
    }
}

// Table (2D - both vertical and horizontal navigation)
pub struct TableState {
    nav: NavigableState,
    rows: usize,
    cols: usize,
}

impl TableState {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            nav: NavigableState::new_2d(),  // selected_col = Some(0)
            rows,
            cols,
        }
    }

    pub fn navigate_left(&mut self) {
        self.nav.navigate_left(self.cols);
    }

    pub fn selected(&self) -> Option<(usize, usize)> {
        self.nav.selected_cell()
    }
}
```

## Shared Styling Helpers

Consistent visual feedback across all focusable components:

```rust
pub fn apply_focus_style(base: Style, is_focused: bool, theme: &Theme) -> Style {
    if is_focused {
        base.fg(theme.accent_primary).bg(theme.bg_surface)
    } else {
        base
    }
}
```

**Note:** No separate hover styling. Hover is only used for click targeting, tooltips, and FocusMode behavior. Visual feedback is focus-only - cleaner and less noisy.

## Benefits

- **Consistent navigation** - List, Tree, Table, FileBrowser all use same logic
- **Scrolloff works everywhere** - vim-style scrolling behavior unified
- **2D support built-in** - Table gets full keyboard navigation
- **Less duplication** - Navigation written once, tested once
- **Easy to extend** - New navigable components just compose NavigableState

**See Also:**
- [NavigableState](../07-advanced/navigable-state.md) - Unified 2D navigation implementation
- [Component Patterns](../04-user-interaction/component-patterns.md) - Interaction patterns
- [Focus System](../04-user-interaction/focus.md) - Focus management

---

**Next:** Learn about [Modals](modals.md) or explore [NavigableState](../07-advanced/navigable-state.md).
