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

Components compose `NavigableState` for consistent navigation behavior across 1D (List, Tree) and 2D (Table) widgets. NavigableState provides unified scrolling, vim-style scrolloff, and selection management.

**See [NavigableState](../07-advanced/navigable-state.md) for complete details on:**
- 1D vs 2D constructors
- Navigation methods (up/down/left/right)
- Scrolloff logic
- Selection accessors

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
