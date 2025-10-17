# Context-Aware Help (F1)

**Prerequisites:** [Keybinds](../04-user-interaction/keybinds.md), [App Launcher](app-launcher.md)

## Overview

**F1** displays a context-aware help modal showing currently active keybinds based on app state. Help content is **automatically generated** from the app's `keybinds()` method, ensuring accuracy and eliminating stale documentation.

## Core Concept

**Apps return different keybind sets based on state** - the help menu displays whatever the app returned from its most recent `keybinds()` call:

- Modal showing? Help shows modal keybinds
- Main view active? Help shows main view keybinds
- Component focused? Help shows component navigation keybinds + app actions

No custom help content - just show what's currently bound.

## Help Entry Structure

```rust
pub struct HelpEntry {
    /// All keys bound to this action (primary + aliases)
    pub keys: Vec<KeyBinding>,

    /// Human-readable description
    pub description: String,

    /// Category for grouping in UI
    pub category: HelpCategory,
}

pub enum HelpCategory {
    Global,      // F1, Ctrl+Space, Ctrl+Q, etc.
    Navigation,  // Arrow keys, Page Up/Down, etc.
    App,         // App-specific actions
    Component,   // Currently focused component's navigation
}
```

## Help Generation

**Runtime generates help entries** by querying config and current app state:

```rust
impl Runtime {
    fn generate_help(&self) -> Vec<HelpEntry> {
        let mut entries = vec![];

        // 1. Global keybinds (always shown)
        entries.extend(self.generate_global_help());

        // 2. Navigation keybinds (always shown)
        entries.extend(self.generate_navigation_help());

        // 3. App keybinds (state-dependent)
        entries.extend(self.generate_app_help());

        // 4. Focused component keybinds
        if let Some(focused) = self.focused_component() {
            entries.extend(self.generate_component_help(focused));
        }

        entries
    }
}
```

### Collecting All Bindings (Primary + Aliases)

```rust
/// Collect all bindings (primary + all aliases) for an action
fn collect_all_bindings(&self, action_key: &str) -> Vec<KeyBinding> {
    let mut keys = vec![];
    let options = &self.config.options;

    // Primary binding
    let primary_key = format!("keybind.{}", action_key);
    if let Ok(key_str) = options.get_string(&primary_key).await {
        if let Ok(key) = KeyBinding::from_str(&key_str) {
            keys.push(key);
        }
    }

    // All aliases (up to 10)
    for i in 1..=10 {
        let alias_key = format!("keybind.{}.alias{}", action_key, i);
        if let Ok(key_str) = options.get_string(&alias_key).await {
            if let Ok(key) = KeyBinding::from_str(&key_str) {
                keys.push(key);
            }
        }
    }

    keys
}
```

## Component Nav Action Reporting

**Components report which navigation actions they handle:**

```rust
pub trait Component {
    /// Which navigation actions does this component handle?
    fn handled_nav_actions(&self) -> Vec<NavActionInfo> {
        vec![]  // Default: handles nothing
    }
}

pub struct NavActionInfo {
    pub action: NavAction,
    pub description: String,
}

// Example: ListComponent
impl ListComponent {
    fn handled_nav_actions(&self) -> Vec<NavActionInfo> {
        vec![
            NavActionInfo {
                action: NavAction::Up,
                description: "Navigate up in list".to_string()
            },
            NavActionInfo {
                action: NavAction::Down,
                description: "Navigate down in list".to_string()
            },
            NavActionInfo {
                action: NavAction::PageUp,
                description: "Scroll up one page".to_string()
            },
            // ... more actions
        ]
    }
}
```

## Help Modal Rendering

**Modal with categorized keybind listing:**

```rust
fn render_help_modal(entries: &[HelpEntry], theme: &Theme) -> Layer {
    Layer::centered(80, 40, panel("Help - Keyboard Shortcuts", |ui| {
        // Group by category
        let global = entries.iter().filter(|e| matches!(e.category, HelpCategory::Global));
        let navigation = entries.iter().filter(|e| matches!(e.category, HelpCategory::Navigation));
        let app = entries.iter().filter(|e| matches!(e.category, HelpCategory::App));
        let component = entries.iter().filter(|e| matches!(e.category, HelpCategory::Component));

        // Global actions section
        if global.clone().count() > 0 {
            ui.text("Global").style(theme.text_primary.bold());
            ui.spacer(1);

            for entry in global {
                render_help_entry(ui, entry, theme);
            }
            ui.spacer(1);
        }

        // ... repeat for other categories

        // Footer
        ui.text("Press F1 or Esc to close")
            .style(theme.text_dim)
            .align(Align::Center);
    }))
    .dim_below(true)
    .blocks_input(true)
}

fn render_help_entry(ui: &mut UiBuilder, entry: &HelpEntry, theme: &Theme) {
    ui.row(|ui| {
        // Keys column (show all aliases inline)
        let keys_str = entry.keys.iter()
            .map(|k| k.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        ui.text(keys_str)
            .width(Length(25))
            .style(theme.accent_1);

        // Description column
        ui.text(&entry.description)
            .width(Fill(1))
            .style(theme.text);
    });
}
```

## Example Output

**Main view with list focused:**

```
╭─ Help - Keyboard Shortcuts ────────────────────────────────────────────────╮
│ Global                                                                      │
│                                                                             │
│ F1                       Toggle help menu                                   │
│ Ctrl+A                   Open app launcher                                  │
│ Ctrl+Q                   Quit application                                   │
│                                                                             │
│ Navigation                                                                  │
│                                                                             │
│ ↑, k                     Move selection/cursor up                           │
│ ↓, j                     Move selection/cursor down                         │
│ ←, h                     Move left (collapse tree nodes)                    │
│ →, l                     Move right (expand tree nodes)                     │
│ Page Up, Ctrl+U          Navigate up one page                               │
│ Page Down, Ctrl+D        Navigate down one page                             │
│ Home, g                  Jump to start                                      │
│ End, G                   Jump to end                                        │
│ Enter                    Activate/confirm selected item                     │
│ Esc                      Close modal/go back                                │
│ Tab                      Next tab/item                                      │
│ Shift+Tab                Previous tab/item                                  │
│                                                                             │
│ Current View                                                                │
│                                                                             │
│ n                        Create new migration environment                   │
│ d                        Delete selected migration                          │
│ r                        Rename selected migration                          │
│                                                                             │
│ Focused Component                                                           │
│                                                                             │
│ ↑, k                     Navigate up in list                                │
│ ↓, j                     Navigate down in list                              │
│ Page Up, Ctrl+U          Scroll up one page                                 │
│ Page Down, Ctrl+D        Scroll down one page                               │
│ Home, g                  Jump to first item                                 │
│ End, G                   Jump to last item                                  │
│ Enter                    Activate selected item                             │
│                                                                             │
│                     Press F1 or Esc to close                                │
╰─────────────────────────────────────────────────────────────────────────────╯
```

**Notice**: "Current View" section changes based on app state (modal vs main view).

## Runtime Help Toggle

**Global keybind handler:**

```rust
impl Runtime {
    fn handle_f1(&mut self) {
        self.showing_help = !self.showing_help;
        self.invalidate();
    }

    fn render(&mut self) -> Vec<Layer> {
        let mut layers = vec![];

        layers.push(Layer::dock_top(3, self.render_header()));
        layers.extend(self.active_app.update(&mut self.ctx));

        // Help modal (if F1 pressed)
        if self.showing_help {
            let help_entries = self.generate_help();
            layers.push(render_help_modal(&help_entries, &self.ctx.theme));
        }

        layers
    }
}
```

## No Custom Help Content

**Dropped feature**: Apps cannot provide custom help content beyond keybind descriptions.

**Reasoning:**
- Keybind descriptions already explain what each action does
- Custom content would become stale (same problem as before)
- Keybinds are the primary help need in TUI apps
- Keeps implementation simple
- Apps can provide tooltips/hints inline in UI if needed

## Benefits

✅ **Always accurate** - Generated from current keybinds, never stale
✅ **State-aware** - Shows different keys based on modal/view state
✅ **Shows all aliases** - Vim users see both arrow keys and hjkl
✅ **Component-specific** - Focused component's navigation shown separately
✅ **Zero maintenance** - No custom help content to keep updated
✅ **Simple implementation** - Queries registry + calls app.keybinds()
✅ **Consistent formatting** - All apps look the same

**See Also:**
- [Keybinds](../04-user-interaction/keybinds.md) - Keybind system details
- [Focus System](../04-user-interaction/focus.md) - Component focus tracking
- [App Launcher](app-launcher.md) - Global launcher system

---

**Next:** Learn about [Settings](settings.md) or explore [Background Apps](background-apps.md).
