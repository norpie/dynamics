# Keybinds

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md), [Options System](../03-state-management/options.md)

## Overview

V2 keybinds are **single-definition** with **direct callbacks** and **layer-scoped activation**:

```rust
impl App for QueueApp {
    fn keybinds() -> KeybindMap<Self> {
        KeybindMap::new()
            // Global binds - work on all layers
            .bind("refresh", "Refresh queue", KeyCode::F(5), Self::refresh)

            // Layer-scoped - only when "confirm_delete" modal rendered
            .layer("confirm_delete")
            .bind_layer("yes", "Confirm", KeyCode::Char('y'), |app, _ctx| {
                app.delete_item();
                app.show_confirm = false;
            })
    }
}
```

**Key features:**
- Single definition (no duplication between registration and usage)
- Type-safe callbacks (`fn(&mut App, &mut Context)`)
- Layer-scoped activation (not per-frame subscriptions)
- Auto-registration to Options system
- User-customizable with aliases
- Stored in SQLite via Options

---

## KeybindMap Structure

```rust
pub struct KeybindMap<A> {
    global_binds: Vec<KeybindDef<A>>,
    layer_binds: IndexMap<String, Vec<KeybindDef<A>>>,
}

pub struct KeybindDef<A> {
    action: String,                      // For Options registration
    description: String,                 // For help menu
    default_key: KeyBinding,             // Default binding
    callback: fn(&mut A, &mut Context),  // Handler
}
```

**Builder pattern:**
```rust
impl<A> KeybindMap<A> {
    pub fn bind(
        self,
        action: &str,
        desc: &str,
        key: impl Into<KeyBinding>,
        callback: fn(&mut A, &mut Context)
    ) -> Self;

    pub fn layer(self, layer_id: &str) -> Self;

    pub fn bind_layer(
        self,
        action: &str,
        desc: &str,
        key: impl Into<KeyBinding>,
        callback: fn(&mut A, &mut Context)
    ) -> Self;
}
```

---

## App Usage

### Global Keybinds

Global keybinds work on **all layers** (base UI + modals):

```rust
impl App for MyApp {
    fn keybinds() -> KeybindMap<Self> {
        KeybindMap::new()
            // Method references
            .bind("save", "Save changes", KeyCode::Ctrl('s'), Self::save)
            .bind("refresh", "Refresh", KeyCode::F(5), Self::refresh)

            // Closures for simple actions
            .bind("back", "Back", KeyCode::Char('b'), |app, ctx| {
                ctx.navigate(AppId::Launcher);
            })
    }
}

impl MyApp {
    fn save(&mut self, ctx: &mut Context) {
        // Implementation
    }

    fn refresh(&mut self, ctx: &mut Context) {
        // Implementation
    }
}
```

### Layer-Scoped Keybinds

Layer-scoped keybinds only activate when their layer is rendered:

```rust
fn keybinds() -> KeybindMap<Self> {
    KeybindMap::new()
        // Global
        .bind("refresh", "Refresh", KeyCode::F(5), Self::refresh)

        // Modal layer binds - only when "confirm_delete" rendered
        .layer("confirm_delete")
        .bind_layer("yes", "Confirm deletion", KeyCode::Char('y'), |app, _ctx| {
            app.delete_item(app.selected);
            app.show_confirm = false;
        })
        .bind_layer("no", "Cancel", KeyCode::Char('n'), |app, _ctx| {
            app.show_confirm = false;
        })

        // Another modal's binds
        .layer("edit_modal")
        .bind_layer("submit", "Submit changes", KeyCode::Enter, Self::submit_edit)
        .bind_layer("cancel", "Cancel edit", KeyCode::Esc, |app, _ctx| {
            app.show_edit = false;
        })
}

fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    let mut layers = vec![Layer::fill(main_ui).id("main")];

    // When this renders, "confirm_delete" layer binds activate
    if self.show_confirm {
        layers.push(Layer::modal(confirm_ui).id("confirm_delete"));
    }

    // When this renders, "edit_modal" layer binds activate
    if self.show_edit {
        layers.push(Layer::modal(edit_ui).id("edit_modal"));
    }

    layers
}
```

**Layer activation:**
- Runtime tracks which layers are currently rendered
- Checks layer binds first (top to bottom), then global binds
- Layer IDs must match between `keybinds()` and `update()`

---

## Runtime Dispatch

Runtime dispatches keybinds by calling callbacks directly:

```rust
impl Runtime {
    fn handle_key_event(&mut self, event: KeyEvent) -> Result<()> {
        let binding = KeyBinding::from(event);

        // 1. Check runtime global keybinds (F1 help, Ctrl+Space launcher, Esc)
        if self.handle_runtime_keybind(&binding)? {
            return Ok(());
        }

        // 2. Check active layers (top to bottom)
        for layer_id in self.active_layers.iter().rev() {
            if let Some(callback) = self.find_keybind_for_layer(&binding, layer_id) {
                callback(&mut self.current_app, &mut self.ctx);  // Direct call!
                return Ok(());
            }
        }

        // 3. Check app global binds
        if let Some(callback) = self.find_global_keybind(&binding) {
            callback(&mut self.current_app, &mut self.ctx);  // Direct call!
            return Ok(());
        }

        Ok(())
    }
}
```

**No string matching** - callbacks are invoked directly.

---

## Auto-Registration to Options

When an app is registered, its keybinds are automatically added to the Options system:

```rust
impl Runtime {
    pub fn register_app<A: App>(&mut self) -> Result<()> {
        let app_id = A::id();
        let keybinds = A::keybinds();

        // Register global binds to Options
        for bind_def in &keybinds.global_binds {
            let option_key = format!("keybind.{}.{}", app_id, bind_def.action);

            self.registry.register(
                OptionDefBuilder::new("keybind", &option_key)
                    .display_name(&bind_def.description)
                    .description(&bind_def.description)
                    .array_type(
                        OptionType::String { max_length: Some(32) },
                        vec![OptionValue::String(bind_def.default_key.to_string())],
                        Some(1),  // min_length
                        None,     // max_length (unlimited aliases)
                    )
                    .build()?
            )?;
        }

        // Register layer-scoped binds
        for (layer_id, layer_binds) in &keybinds.layer_binds {
            for bind_def in layer_binds {
                let option_key = format!("keybind.{}.{}.{}", app_id, layer_id, bind_def.action);
                // ... same as above
            }
        }

        Ok(())
    }
}
```

**Keybind storage:**
```
keybind.queue.refresh = ["F5"]
keybind.queue.save = ["Ctrl+s"]
keybind.queue.confirm_delete.yes = ["y"]
keybind.queue.confirm_delete.no = ["n"]
```

---

## User Customization

Users can customize keybinds via the Settings UI or programmatically.

### Adding Aliases

Users can add multiple keybinds for the same action (non-destructive):

```rust
// Via Options API
options.set("keybind.queue.refresh", OptionValue::Array(vec![
    OptionValue::String("F5".into()),      // Primary (from default)
    OptionValue::String("r".into()),       // Alias 1
    OptionValue::String("Ctrl+r".into()),  // Alias 2
])).await?;
```

**All three bindings trigger the same callback.**

### Changing Primary Binding

```rust
options.set("keybind.queue.save", OptionValue::Array(vec![
    OptionValue::String("Ctrl+w".into()),  // New primary
])).await?;
```

### Vim Mode Preset

Settings UI can apply presets that add Vim-style navigation:

```rust
pub async fn apply_vim_nav_preset(options: &Options) -> Result<()> {
    // Add vim navigation as aliases (non-destructive)
    let nav_binds = vec![
        ("global.nav.up", vec!["Up", "k"]),
        ("global.nav.down", vec!["Down", "j"]),
        ("global.nav.left", vec!["Left", "h"]),
        ("global.nav.right", vec!["Right", "l"]),
    ];

    for (action, keys) in nav_binds {
        let values = keys.into_iter()
            .map(|k| OptionValue::String(k.into()))
            .collect();
        options.set(&format!("keybind.{}", action), OptionValue::Array(values)).await?;
    }

    Ok(())
}
```

---

## Navigation Bindings

Framework provides global navigation bindings for widgets:

```rust
pub enum NavAction {
    Up, Down, Left, Right,
    PageUp, PageDown,
    Home, End,
    Activate,  // Enter
    Cancel,    // Esc
    Next,      // Tab
    Previous,  // Shift+Tab
}
```

**Widgets handle navigation automatically:**
```rust
fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    vec![Layer::fill(panel("Queue", |ui| {
        // List handles Up/Down/Activate internally
        ui.list(&mut self.list_state, &self.items, |item, ui| {
            ui.text(&item.name);
        });

        // Tree handles Up/Down/Left (collapse)/Right (expand)
        ui.tree(&mut self.tree_state, &self.nodes);
    }))]
}
```

**Apps never see navigation keys** - they're consumed by focused widgets.

**Dispatch priority:**
1. Navigation bindings → Focused widget
2. Runtime global keybinds → Runtime actions
3. Layer-scoped keybinds → App handlers
4. App global keybinds → App handlers

---

## Complete Example

```rust
struct QueueApp {
    items: Vec<QueueItem>,
    selected: usize,
    show_confirm: bool,
}

impl App for QueueApp {
    fn id() -> &'static str { "queue" }

    fn keybinds() -> KeybindMap<Self> {
        KeybindMap::new()
            // Global binds (work on all layers)
            .bind("refresh", "Refresh queue", KeyCode::F(5), Self::refresh)
            .bind("back", "Back to launcher", KeyCode::Char('b'), |app, ctx| {
                ctx.navigate(AppId::Launcher);
            })

            // Main layer binds
            .layer("main")
            .bind_layer("process", "Process item", KeyCode::Char('p'), Self::process_item)
            .bind_layer("delete", "Delete item", KeyCode::Char('d'), |app, _ctx| {
                app.show_confirm = true;  // Open modal
            })

            // Modal layer binds
            .layer("confirm_delete")
            .bind_layer("yes", "Confirm deletion", KeyCode::Char('y'), |app, _ctx| {
                app.items.remove(app.selected);
                app.show_confirm = false;
            })
            .bind_layer("no", "Cancel", KeyCode::Char('n'), |app, _ctx| {
                app.show_confirm = false;
            })
    }

    fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
        let main_ui = panel("Queue", |ui| {
            for (i, item) in self.items.iter().enumerate() {
                ui.text(&item.name);
            }
        });

        let mut layers = vec![Layer::fill(main_ui).id("main")];

        // Confirmation modal (activates "confirm_delete" layer binds)
        if self.show_confirm {
            layers.push(
                Layer::centered(50, 10, panel("Confirm", |ui| {
                    ui.text("Delete item?");
                    ui.text("Press 'y' to confirm, 'n' to cancel");
                }))
                .id("confirm_delete")
                .dim_below(true)
                .blocks_input(true)
            );
        }

        layers
    }
}

impl QueueApp {
    fn refresh(&mut self, ctx: &mut Context) {
        // Reload items
    }

    fn process_item(&mut self, ctx: &mut Context) {
        // Process selected item
    }
}
```

**Resulting keybind registrations:**
```
keybind.queue.refresh = ["F5"]
keybind.queue.back = ["b"]
keybind.queue.main.process = ["p"]
keybind.queue.main.delete = ["d"]
keybind.queue.confirm_delete.yes = ["y"]
keybind.queue.confirm_delete.no = ["n"]
```

**User experience:**
- On main screen: F5, b, p, d work
- Modal open: F5, b still work (global), y, n work (modal), p, d don't work (main layer inactive)

---

## See Also

- **[Options System](../03-state-management/options.md)** - Storage backend for keybinds
- **[Settings UI](../06-system-features/settings-ui.md)** - UI for customizing keybinds
- **[Help System](../06-system-features/help-system.md)** - Auto-generated help menu
- **[Layers](../02-building-ui/layers.md)** - Layer system for scoping

---

**Next:** Learn about [Focus Management](focus.md) or explore [Mouse Interaction](mouse.md).
