# Keybinds (First-Class)

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md)

## Declarative Definition

Apps define keybinds in the `keybinds()` method:

```rust
impl App for MyApp {
    fn keybinds() -> KeybindMap {
        KeybindMap::new()
            .action("save", "Save changes", default_key!("Ctrl+S"), Self::handle_save)
            .action("quit", "Quit app", default_key!("q"), Self::handle_quit)
            .action("refresh", "Refresh data", default_key!("r"), Self::handle_refresh)
    }
}
```

**Components:**
- **Action ID** - Unique identifier ("save", "quit")
- **Description** - Shown in help menu
- **Default key** - Fallback if user hasn't configured
- **Handler** - Method reference

## User Configuration

Users can override defaults via config file:

```toml
# ~/.config/dynamics/keybinds.toml
[global]
app_launcher = "Ctrl+Space"
help = "F1"
quit = "Ctrl+Q"

[app.EntityComparison]
save = "Ctrl+S"
refresh = "F5"
quit = "Escape"  # Override global for this app
```

## Automatic Widget Navigation

**Widgets handle their own keys automatically - zero boilerplate!**

```rust
fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    vec![Layer::fill(panel("Queue", |ui| {
        // List gets arrow keys automatically!
        ui.list(&mut self.list_state, &self.items);

        // Tree gets arrows + Space automatically!
        ui.tree(&mut self.tree_state, &self.nodes);

        // Text input gets typing automatically!
        ui.text_input(&mut self.input);
    }))]
}
```

**Framework routing priority:**
1. Focused widget - does it want this key?
2. Global keybinds (app_launcher, help, etc.)
3. App keybinds
4. Ignore if unbound

## Button Keybind Integration

Buttons and keybinds can call the same handler:

```rust
// Button calls handler on click
ui.button("Clear All").on_click(Self::handle_clear);

// Keybind calls same handler on key press
// Framework can show "Clear All (Ctrl+K)" by matching signatures
```

**See Also:**
- [Navigation](navigation.md) - Tab/Shift-Tab focus navigation
- [Focus System](focus.md) - Focus management details
- [Component Patterns](component-patterns.md) - Handler patterns
- [Help System](../06-system-features/help-system.md) - Auto-generated help from keybinds

---

**Next:** Learn about [Focus System](focus.md) for focus management.
