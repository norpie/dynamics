# Modal System

**Prerequisites:** [Layers](layers.md)

## Modals Are Just Layers

**Core concept:** Modals are not a special framework type - they're layers (see [Layer System](layers.md)) with `dim_below: true` and `blocks_input: true`.

**Apps control visibility** - framework doesn't auto-show/hide modals:

```rust
struct MyApp {
    show_confirm: bool,  // App owns this flag
}

fn handle_yes(&mut self, ctx: &mut Context) {
    self.delete_file();
    self.show_confirm = false;  // App hides modal
}
```

## Hybrid Approach: Pattern + Optional Helpers

**Modals are a pattern, not a framework concept.**

Apps can choose:
1. **Raw layers** - Maximum flexibility for custom modals
2. **Builder helpers** - Convenience for common patterns

## Raw Layers (Maximum Flexibility)

For custom modals, just use layers directly:

```rust
if self.show_settings {
    layers.push(
        Layer::centered(80, 30, panel("Settings", |ui| {
            ui.text("Theme:");
            ui.select(&mut self.theme_select, &["Mocha", "Latte"]);

            if ui.button("Save").clicked() {
                self.save_settings();
                self.show_settings = false;
            }
        }))
        .dim_below(true)
        .blocks_input(true)
    );
}
```

## Builder Helpers (Convenience)

For common patterns, use builder types:

```rust
pub struct ConfirmationModal {
    title: String,
    message: String,
    on_yes: Option<fn()>,
    on_no: Option<fn()>,
    on_cancel: Option<fn()>,  // Called on Esc
    width: u16,
    height: u16,
}

impl ConfirmationModal {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self;
    pub fn on_yes(mut self, handler: fn()) -> Self;
    pub fn on_no(mut self, handler: fn()) -> Self;
    pub fn on_cancel(mut self, handler: fn()) -> Self;
    pub fn build(self) -> Layer;
}
```

**App usage:**
```rust
if self.show_delete_confirm {
    layers.push(
        ConfirmationModal::new("Delete File?", "This cannot be undone")
            .on_yes(Self::handle_confirm_delete)
            .on_no(Self::handle_cancel_delete)
            .on_cancel(Self::handle_cancel_delete)  // Esc = same as No
            .width(60)
            .build()
    );
}
```

## Built-in Modal Helpers

### ConfirmationModal

Yes/No dialogs.

### ErrorModal

Error display with dismiss button:

```rust
ErrorModal::new("Failed to Save", &error_message)
    .on_dismiss(Self::handle_error_dismiss)
    .build()
```

### LoadingModal

Loading spinner with optional progress:

```rust
// Simple spinner
LoadingModal::new("Loading...").build()

// With progress
LoadingModal::new("Processing files...")
    .progress(self.completed, self.total)
    .build()
```

### HelpModal

Keybinding viewer (auto-populated from app keybinds):

```rust
HelpModal::new()
    .app_keybinds(MyApp::keybinds())
    .build()
```

## Modal Dismissal (Esc Behavior)

**Apps control when modals close** - framework doesn't auto-dismiss.

Modal builders register Esc handler:

```rust
ConfirmationModal::new("Delete?", "Sure?")
    .on_cancel(Self::handle_cancel)  // Called when Esc pressed
    .build()

fn handle_cancel(&mut self, ctx: &mut Context) {
    self.show_confirm = false;  // App controls dismissal
}
```

Apps can also handle Esc globally via keybinds.

## Context Menu Pattern

Right-click context menus are just layers positioned at mouse:

```rust
if let Some(Event::RightClick(idx)) = ui.events().list() {
    self.show_context_menu = true;
    self.context_menu_pos = ctx.mouse.position();
}

if self.show_context_menu {
    layers.push(
        Layer::at(self.context_menu_pos, panel("", |ui| {
            if ui.button("Open").clicked() { /* ... */ }
            if ui.button("Delete").clicked() { /* ... */ }
        }))
        .dim_below(false)     // Context menus don't dim
        .blocks_input(true)   // But they do block clicks
    );
}
```

**Optional ContextMenu helper available** for common patterns.

## Benefits

- **Simple mental model** - Modals are just layers with flags
- **Maximum flexibility** - Raw layers available for custom modals
- **Convenience helpers** - Builders for common patterns
- **App-controlled** - Apps decide when to show/hide modals
- **Consistent UI** - Builders provide standard styling (but customizable)
- **Easy to extend** - Adding new modal types is just adding a builder

**See Also:**
- [Layers](layers.md) - Layer system details
- [Focus System](../04-user-interaction/focus.md) - Layer-scoped focus restoration
- [Mouse Support](../04-user-interaction/mouse.md) - Context menu interactions

---

**Next:** Explore [Layout](layout.md) or [Components](components.md).
