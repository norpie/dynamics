# Keybinds (First-Class)

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md)

> **TODO:** Alternative keybind system design exists (v2.md L3988-4500) with:
> - Three categories (Navigation/Global/App) instead of two
> - NavAction enum for semantic navigation (Up/Down/Left/Right/etc)
> - Alias system (`.alias1`, `.alias2`) for multiple keys per action
> - OptionsRegistry registration instead of KeybindMap
> - SQLite database storage (not TOML)
> - Vim mode presets, conflict detection, settings UI
>
> Need to resolve which design is final before implementation.

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

---

## Alternative Keybind System Design (v2.md L3988-4500)

> **Note:** This section documents an alternative keybind system design found in v2.md. Decision needed on which approach to implement.

### Three Binding Categories

Instead of two categories (Global, App), this design uses three:

1. **Navigation Bindings** (`keybind.global.nav.*`)
   - Semantic navigation actions (up, down, activate, cancel, etc.)
   - Sent directly to focused components as NavActions
   - Components handle internally (no app code needed)
   - User can add vim-style or custom aliases

2. **Global Bindings** (`keybind.global.*`)
   - Runtime-level actions (help menu, app launcher, quit, etc.)
   - Checked after navigation but before app bindings

3. **App Bindings** (`keybind.{app}.*`)
   - App-specific actions
   - Lowest priority in key handling

### NavAction Enum

Semantic navigation actions handled by components:

```rust
pub enum NavAction {
    Up,           // Move selection/cursor up
    Down,         // Move selection/cursor down
    Left,         // Move left (tree collapse, etc.)
    Right,        // Move right (tree expand, etc.)
    PageUp,       // Navigate by page
    PageDown,     // Navigate by page
    Home,         // Jump to start
    End,          // Jump to end
    Activate,     // Confirm/select (Enter, Space)
    Cancel,       // Close/back (Esc)
    Next,         // Next item/tab (Tab)
    Previous,     // Previous item/tab (Shift+Tab)
}

impl NavAction {
    fn from_option_key(key: &str) -> Option<Self> {
        match key {
            "global.nav.up" => Some(NavAction::Up),
            "global.nav.down" => Some(NavAction::Down),
            // ... etc
            _ => None,
        }
    }
}
```

### Alias System

**Multiple keys per action** using `.alias1`, `.alias2`, etc. suffix pattern:

```rust
// Registration (in keybinds.rs)
registry.register(
    OptionDefBuilder::new("keybind", "global.nav.up")
        .display_name("Navigate Up")
        .description("Move selection/cursor up in focused component")
        .keybind_type(KeyCode::Up)  // Primary binding
        .build()?
)?;

// Optional aliases (user can add via settings)
registry.register(
    OptionDefBuilder::new("keybind", "global.nav.up.alias1")
        .display_name("Navigate Up (Vim)")
        .keybind_type(KeyCode::Char('k'))
        .build()?
)?;
```

**In SQLite database:**
```
keybind.global.nav.up = "Up"           # Primary (default)
keybind.global.nav.up.alias1 = "k"     # Vim style
keybind.global.nav.up.alias2 = "w"     # Custom user binding
```

All three keys trigger the same `NavAction::Up` - non-destructive customization.

### Keybind Registration

Using OptionsRegistry instead of KeybindMap:

```rust
pub fn register_navigation(registry: &OptionsRegistry) -> Result<()> {
    // Directional navigation
    registry.register(
        OptionDefBuilder::new("keybind", "global.nav.up")
            .display_name("Navigate Up")
            .description("Move selection/cursor up")
            .keybind_type(KeyCode::Up)
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "global.nav.down")
            .display_name("Navigate Down")
            .description("Move selection/cursor down")
            .keybind_type(KeyCode::Down)
            .build()?
    )?;

    // ... more registrations

    Ok(())
}
```

### Runtime Key Handling

**Priority order when key is pressed:**

```rust
impl Runtime {
    fn handle_key(&mut self, key_event: KeyEvent) {
        // 1. Check if key is bound to a navigation action
        if let Some((nav_key, nav_action)) = self.lookup_navigation(key_event) {
            if let Some(focused) = self.focused_component() {
                if focused.handle_nav(nav_action) {
                    return;  // Component consumed the navigation action
                }
            }
        }

        // 2. Check global keybinds
        if let Some(global_action) = self.lookup_global_keybind(key_event) {
            self.handle_global_action(global_action);
            return;
        }

        // 3. Check app keybinds
        if let Some(app_action) = self.active_app.lookup_keybind(key_event) {
            app_action.call(&mut self.active_app);
            return;
        }

        // 4. Ignored - key not bound to anything
    }

    /// Lookup which navigation action this key triggers (checks primary + all aliases)
    fn lookup_navigation(&self, key: KeyEvent) -> Option<(&str, NavAction)> {
        for nav_key in &["up", "down", "left", "right", "activate", "cancel",
                         "page_up", "page_down", "home", "end", "next", "previous"] {
            let base_key = format!("global.nav.{}", nav_key);

            // Check primary binding
            let primary_option_key = format!("keybind.{}", base_key);
            if let Ok(bound_str) = self.config.options.get_string(&primary_option_key) {
                if let Ok(bound_key) = KeyBinding::from_str(&bound_str) {
                    if bound_key.matches(&key) {
                        if let Some(action) = NavAction::from_option_key(&base_key) {
                            return Some((nav_key, action));
                        }
                    }
                }
            }

            // Check aliases (alias1, alias2, ..., alias10)
            for i in 1..=10 {
                let alias_option_key = format!("keybind.{}.alias{}", base_key, i);
                if let Ok(bound_str) = self.config.options.get_string(&alias_option_key) {
                    if let Ok(bound_key) = KeyBinding::from_str(&bound_str) {
                        if bound_key.matches(&key) {
                            if let Some(action) = NavAction::from_option_key(&base_key) {
                                return Some((nav_key, action));
                            }
                        }
                    }
                }
            }
        }
        None
    }
}
```

### Component Navigation Handling

**Components handle NavActions internally** - apps never see navigation keys:

```rust
// Internal to framework - apps don't touch this
impl ListComponent {
    fn handle_nav(&mut self, action: NavAction) -> bool {
        match action {
            NavAction::Up => {
                self.state.navigate_up();
                true  // Consumed
            }
            NavAction::Down => {
                self.state.navigate_down();
                true  // Consumed
            }
            NavAction::PageUp => {
                self.state.navigate_page_up();
                true
            }
            NavAction::Activate => {
                // Emit semantic event to app
                self.emit(ListEvent::Activated(self.state.selected()));
                true
            }
            _ => false  // Doesn't handle this action
        }
    }
}
```

### Keybind Presets

**Settings app provides preset buttons** - applies multiple aliases at once:

```rust
/// Apply Vim navigation preset (non-destructive - adds aliases)
pub async fn apply_vim_preset(options: &Options) -> Result<()> {
    options.set("keybind.global.nav.up.alias1", "k").await?;
    options.set("keybind.global.nav.down.alias1", "j").await?;
    options.set("keybind.global.nav.left.alias1", "h").await?;
    options.set("keybind.global.nav.right.alias1", "l").await?;
    options.set("keybind.global.nav.page_up.alias1", "Ctrl+u").await?;
    options.set("keybind.global.nav.page_down.alias1", "Ctrl+d").await?;
    options.set("keybind.global.nav.home.alias1", "g").await?;
    options.set("keybind.global.nav.end.alias1", "G").await?;
    Ok(())
}

/// Remove Vim preset (delete aliases)
pub async fn remove_vim_preset(options: &Options) -> Result<()> {
    options.delete("keybind.global.nav.up.alias1").await?;
    options.delete("keybind.global.nav.down.alias1").await?;
    // ... delete all vim aliases
    Ok(())
}
```

**After applying Vim preset:**
- Arrow keys still work (primary bindings unchanged)
- Vim keys also work (added as aliases)
- User can further customize either

### Conflict Detection

**Check for key conflicts across all categories:**

```rust
pub fn find_conflicts(key: KeyBinding, registry: &OptionsRegistry) -> Vec<String> {
    let mut conflicts = vec![];

    // Check all registered keybinds (primary + aliases)
    for option_def in registry.list_namespace("keybind") {
        if let Ok(bound_str) = option_def.default.as_string() {
            if let Ok(bound_key) = KeyBinding::from_str(&bound_str) {
                if bound_key == key {
                    conflicts.push(option_def.key.clone());
                }
            }
        }
    }

    conflicts
}
```

**Settings UI warns about conflicts:**
```
⚠️ Warning: Key 'k' is bound to multiple actions:
  - keybind.global.nav.up.alias1 (Navigate Up)
  - keybind.entity_comparison.create_mapping (Create Mapping)

Navigation binding takes precedence when component is focused.
App binding only triggers when no component is focused.
```

### Settings UI

**Per-action keybind editor:**
```
╭─ Navigate Up ─────────────────────────────────╮
│ Primary:  ↑                      [Edit]       │
│ Alias 1:  k                      [Remove]     │
│ Alias 2:  w                      [Remove]     │
│                                  [Add Alias]  │
├───────────────────────────────────────────────┤
│ Description: Move selection/cursor up in      │
│              focused component                │
╰───────────────────────────────────────────────╯

[Apply Vim Preset]  [Remove Vim Preset]  [Reset to Defaults]
```

**Keybind listing grouped by category:**
```
╭─ Global Navigation ───────────────────────────╮
│ Navigate Up       ↑, k, w                     │
│ Navigate Down     ↓, j                        │
│ Activate          Enter                       │
│ Cancel            Esc                         │
├─ Global Actions ──────────────────────────────┤
│ Help Menu         F1                          │
│ App Launcher      Ctrl+A                      │
│ Quit              Ctrl+Q                      │
├─ App: Entity Comparison ──────────────────────┤
│ Create Mapping    m                           │
│ Delete Mapping    d                           │
│ Refresh Metadata  F5                          │
│ Export to Excel   F10                         │
╰───────────────────────────────────────────────╯
```

### Benefits of Alternative Approach

✅ **Component-agnostic navigation** - One keybind set works across all components
✅ **Vim mode support** - Non-destructive aliases preserve arrow keys
✅ **Unlimited aliases** - User can bind as many keys as they want per action
✅ **No app boilerplate** - Apps never handle navigation keys
✅ **Conflict detection** - Warns about overlapping bindings
✅ **User customizable** - Full editor with presets
✅ **Uses existing infrastructure** - Options system + suffix pattern

### Comparison: Current vs Alternative

| Aspect | Current (keybinds.md) | Alternative (L3988-4500) |
|--------|----------------------|------------------------|
| **Storage** | TOML files | SQLite database via Options system |
| **Navigation** | Mentioned but not detailed | Full `NavAction` enum with semantic actions |
| **Aliases** | Not mentioned | Full alias system (`.alias1`, `.alias2`, etc.) |
| **Categories** | 2 (Global, App) | 3 (Navigation, Global, App) |
| **API** | `KeybindMap::action()` | `OptionsRegistry.register()` |
| **Presets** | Not mentioned | Vim mode preset system |
| **Conflict Detection** | Not mentioned | Built-in conflict warnings |
| **Settings UI** | Not mentioned | Detailed settings UI spec |
| **Vim Support** | Not mentioned | Full vim mode via non-destructive aliases |

---

**Next:** Learn about [Focus System](focus.md) for focus management.
