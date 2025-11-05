# Context-Aware Help (F1)

**Prerequisites:** [Keybinds](../04-user-interaction/keybinds.md), [Options System](../03-state-management/options.md)

## Overview

**F1** displays a context-aware help modal showing currently active keybinds based on rendered layers. Help content is **automatically generated** from the keybind system, ensuring accuracy and eliminating stale documentation.

**Key features:**
- Auto-generated from KeybindMap definitions
- Layer-aware (shows different binds when modals open)
- Displays all aliases from Options storage
- Component navigation keybinds included
- Always accurate (no manual maintenance)

---

## Help Generation

Runtime generates help by querying active layers and their keybinds:

```rust
struct HelpSection {
    title: String,
    binds: Vec<(Vec<KeyBinding>, String)>,  // (all keys, description)
}

impl Runtime {
    fn generate_help(&self) -> Vec<HelpSection> {
        let mut sections = vec![];

        // 1. Global runtime keybinds
        sections.push(self.generate_global_section());

        // 2. Current app global binds
        sections.push(self.generate_app_global_section());

        // 3. Active layer binds (context-dependent!)
        for layer_id in &self.active_layers {
            if let Some(section) = self.generate_layer_section(layer_id) {
                sections.push(section);
            }
        }

        // 4. Navigation keybinds (always shown)
        sections.push(self.generate_navigation_section());

        sections
    }
}
```

---

## Global Runtime Keybinds

Always shown at the top:

```rust
fn generate_global_section(&self) -> HelpSection {
    let mut binds = vec![];

    // Get all keys (primary + aliases) from Options
    if let Some(keys) = self.get_all_keybinds("global.help") {
        binds.push((keys, "Toggle help menu".to_string()));
    }

    if let Some(keys) = self.get_all_keybinds("global.launcher") {
        binds.push((keys, "Open app launcher".to_string()));
    }

    if let Some(keys) = self.get_all_keybinds("global.escape") {
        binds.push((keys, "Close modal / Go back".to_string()));
    }

    HelpSection {
        title: "Global".to_string(),
        binds,
    }
}

/// Get all keybindings (primary + aliases) from Options
fn get_all_keybinds(&self, action: &str) -> Option<Vec<KeyBinding>> {
    let key = format!("keybind.{}", action);

    // Options stores as array: ["F1", "?"]
    if let Some(arr) = self.config.keybinds.get(&key) {
        Some(arr.clone())  // Already Vec<KeyBinding> in RuntimeConfig
    } else {
        None
    }
}
```

---

## App Global Keybinds

App's global binds (work on all layers):

```rust
fn generate_app_global_section(&self) -> HelpSection {
    let app_id = self.current_app.id();
    let keybinds = self.current_app.get_keybinds();

    let mut binds = vec![];

    for bind_def in &keybinds.global_binds {
        let action_key = format!("keybind.{}.{}", app_id, bind_def.action);

        // Get all bindings (primary + aliases) from Options
        if let Some(keys) = self.config.keybinds.get(&action_key) {
            binds.push((keys.clone(), bind_def.description.clone()));
        }
    }

    HelpSection {
        title: format!("{} - Global", self.current_app.get_title()),
        binds,
    }
}
```

---

## Layer-Specific Keybinds

Only shown when layer is active (context-aware!):

```rust
fn generate_layer_section(&self, layer_id: &str) -> Option<HelpSection> {
    let app_id = self.current_app.id();
    let keybinds = self.current_app.get_keybinds();

    let layer_binds = keybinds.layer_binds.get(layer_id)?;

    let mut binds = vec![];

    for bind_def in layer_binds {
        let action_key = format!("keybind.{}.{}.{}", app_id, layer_id, bind_def.action);

        // Get all bindings (primary + aliases)
        if let Some(keys) = self.config.keybinds.get(&action_key) {
            binds.push((keys.clone(), bind_def.description.clone()));
        }
    }

    if binds.is_empty() {
        return None;
    }

    Some(HelpSection {
        title: format!("{} - {}", self.current_app.get_title(), layer_id),
        binds,
    })
}
```

**Context awareness:**
- Main view active → Shows main layer binds
- Modal open → Shows modal layer binds instead
- No manual state tracking needed!

---

## Navigation Keybinds

Framework navigation keys (always shown):

```rust
fn generate_navigation_section(&self) -> HelpSection {
    let nav_actions = vec![
        ("global.nav.up", "Move selection/cursor up"),
        ("global.nav.down", "Move selection/cursor down"),
        ("global.nav.left", "Move left (collapse tree nodes)"),
        ("global.nav.right", "Move right (expand tree nodes)"),
        ("global.nav.page_up", "Navigate up one page"),
        ("global.nav.page_down", "Navigate down one page"),
        ("global.nav.home", "Jump to start"),
        ("global.nav.end", "Jump to end"),
        ("global.nav.activate", "Activate/confirm"),
        ("global.nav.next", "Next tab/item"),
        ("global.nav.previous", "Previous tab/item"),
    ];

    let mut binds = vec![];

    for (action, desc) in nav_actions {
        let key = format!("keybind.{}", action);
        if let Some(keys) = self.config.keybinds.get(&key) {
            binds.push((keys.clone(), desc.to_string()));
        }
    }

    HelpSection {
        title: "Navigation".to_string(),
        binds,
    }
}
```

---

## Help Modal Rendering

```rust
fn render_help_modal(sections: &[HelpSection], theme: &Theme) -> Layer {
    Layer::centered(80, 40, panel("Help - Keyboard Shortcuts", |ui| {
        for section in sections {
            // Section header
            ui.text(&section.title)
                .style(theme.text_primary.bold());
            ui.spacer(1);

            // Section binds
            for (keys, desc) in &section.binds {
                render_help_entry(ui, keys, desc, theme);
            }

            ui.spacer(1);
        }

        // Footer
        ui.text("Press F1 or Esc to close")
            .style(theme.text_dim)
            .align(Align::Center);
    }))
    .id("help")
    .dim_below(true)
    .blocks_input(true)
}

fn render_help_entry(
    ui: &mut Ui,
    keys: &[KeyBinding],
    desc: &str,
    theme: &Theme
) {
    ui.row(|ui| {
        // Keys column (show all aliases)
        let keys_str = keys.iter()
            .map(|k| k.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        ui.text(keys_str)
            .width(Length(25))
            .style(theme.accent_1);

        // Description column
        ui.text(desc)
            .width(Fill(1))
            .style(theme.text);
    });
}
```

---

## Example Output

### Main View Active

```
╭─ Help - Keyboard Shortcuts ────────────────────────────────────────────────╮
│ Global                                                                      │
│                                                                             │
│ F1, ?                    Toggle help menu                                   │
│ Ctrl+Space               Open app launcher                                  │
│ Esc                      Close modal / Go back                              │
│                                                                             │
│ Queue - Global                                                              │
│                                                                             │
│ F5, r                    Refresh queue                                      │
│ b                        Back to launcher                                   │
│                                                                             │
│ Queue - main                                                                │
│                                                                             │
│ p                        Process selected item                              │
│ d                        Delete selected item                               │
│                                                                             │
│ Navigation                                                                  │
│                                                                             │
│ ↑, k                     Move selection/cursor up                           │
│ ↓, j                     Move selection/cursor down                         │
│ Enter                    Activate/confirm                                   │
│                                                                             │
│                     Press F1 or Esc to close                                │
╰─────────────────────────────────────────────────────────────────────────────╯
```

### Modal Open

```
╭─ Help - Keyboard Shortcuts ────────────────────────────────────────────────╮
│ Global                                                                      │
│                                                                             │
│ F1, ?                    Toggle help menu                                   │
│ Ctrl+Space               Open app launcher                                  │
│ Esc                      Close modal / Go back                              │
│                                                                             │
│ Queue - Global                                                              │
│                                                                             │
│ F5, r                    Refresh queue                                      │
│ b                        Back to launcher                                   │
│                                                                             │
│ Queue - confirm_delete                  ← Changed!                         │
│                                                                             │
│ y                        Confirm deletion                                   │
│ n                        Cancel                                             │
│                                                                             │
│ Navigation                                                                  │
│                                                                             │
│ ↑, k                     Move selection/cursor up                           │
│ ↓, j                     Move selection/cursor down                         │
│ Enter                    Activate/confirm                                   │
│                                                                             │
│                     Press F1 or Esc to close                                │
╰─────────────────────────────────────────────────────────────────────────────╯
```

**Notice**: "Queue - main" changed to "Queue - confirm_delete" when modal opened!

---

## Alias Display

Users who enabled Vim mode see all aliases:

```
│ ↑, k, j                  Move selection up                                  │
│ ↓, j                     Move selection down                                │
│ F5, r, Ctrl+r            Refresh queue                                      │
```

**All keys trigger the same action** - help just displays them all.

---

## Runtime Integration

```rust
impl Runtime {
    fn handle_key_event(&mut self, event: KeyEvent) -> Result<()> {
        let binding = KeyBinding::from(event);

        // F1 toggles help
        if self.is_help_keybind(&binding) {
            self.showing_help = !self.showing_help;
            return Ok(());
        }

        // Esc closes help if showing
        if self.showing_help && matches!(binding.key, KeyCode::Esc) {
            self.showing_help = false;
            return Ok(());
        }

        // ... normal keybind dispatch
    }

    fn render(&mut self) -> Vec<Layer> {
        let mut layers = vec![];

        // App layers
        layers.extend(self.active_app.update(&mut self.ctx));

        // Help modal (if F1 pressed)
        if self.showing_help {
            let sections = self.generate_help();
            layers.push(render_help_modal(&sections, &self.ctx.theme));
        }

        layers
    }
}
```

---

## Benefits

✅ **Always accurate** - Generated from KeybindMap, never stale
✅ **Layer-aware** - Automatically shows different binds based on active layers
✅ **Shows all aliases** - Vim users see both arrow keys and hjkl
✅ **Zero maintenance** - No custom help content to keep updated
✅ **Simple implementation** - Just query KeybindMap + RuntimeConfig
✅ **Consistent formatting** - All apps look the same

**See Also:**
- **[Keybinds](../04-user-interaction/keybinds.md)** - Keybind system details
- **[Options System](../03-state-management/options.md)** - Storage backend
- **[Settings UI](settings-ui.md)** - Customizing keybinds

---

**Next:** Learn about [Settings UI](settings-ui.md) or explore [App Launcher](app-launcher.md).
