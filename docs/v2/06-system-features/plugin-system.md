# Plugin System

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md), [Layers](../02-building-ui/layers.md)

## Overview

Plugins are **runtime extensions** with system-level access. Unlike user apps, plugins:

- Run alongside all apps (not mutually exclusive)
- Can intercept and modify events before apps see them
- Can inject/modify/remove layers from any app
- Have access to runtime internals (queues, focus state, etc.)
- Auto-register via `inventory` crate (no central registry)
- Configure via `Options` derive (type-safe, validated)

**Examples**: Header, help menu, app launcher, debug overlay, metrics, notifications.

---

## Plugin Trait

```rust
trait Plugin: 'static {
    // ===== Identity =====
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str { self.id() }
    fn create(config: &Config) -> Box<dyn Plugin> where Self: Sized;

    // ===== Keybinds (Automatic Registration) =====
    fn keybinds(&self) -> KeybindMap {
        KeybindMap::new()
    }

    // ===== Lifecycle Hooks =====

    /// Called once on plugin load
    fn on_init(&mut self, ctx: &mut PluginContext) {}

    /// Called before app sees event (can filter/transform/consume)
    fn on_event(&mut self, event: &Event, ctx: &mut PluginContext) -> EventFlow {
        EventFlow::Continue
    }

    /// Called after apps return layers, before rendering (can add/modify/remove)
    fn on_layers(&mut self, layers: &mut Vec<Layer>, ctx: &mut PluginContext) {}

    /// Called after render completes
    fn on_post_render(&mut self, ctx: &mut PluginContext) {}

    /// Called when switching apps
    fn on_app_switch(&mut self, from: &str, to: &str, ctx: &mut PluginContext) {}

    /// Called when focus changes
    fn on_focus_change(&mut self, event: &FocusEvent, ctx: &mut PluginContext) {}

    /// Called on layer lifecycle events
    fn on_layer_event(&mut self, event: &LayerEvent, ctx: &mut PluginContext) {}

    /// Called on shutdown
    fn on_shutdown(&mut self, ctx: &mut PluginContext) {}
}

enum EventFlow {
    Continue,  // Pass event to next plugin/app
    Consume,   // Don't pass event further (handled)
}
```

---

## Auto-Registration (No Central Registry)

Plugins register themselves using the `inventory` crate:

```rust
use inventory;

// Registration struct (collected automatically)
inventory::collect!(PluginRegistration);

pub struct PluginRegistration {
    pub id: &'static str,
    pub name: &'static str,
    pub create: fn(&Config) -> Box<dyn Plugin>,
}

// Convenience macro
#[macro_export]
macro_rules! register_plugin {
    ($ty:ty) => {
        inventory::submit! {
            PluginRegistration {
                id: <$ty>::ID,
                name: <$ty>::NAME,
                create: |config| Box::new(<$ty>::from_config(config)),
            }
        }
    };
}
```

**Runtime discovers plugins automatically:**

```rust
impl Runtime {
    fn load_plugins(&mut self) {
        // Iterate ALL registered plugins (via inventory)
        for registration in inventory::iter::<PluginRegistration> {
            // Check if enabled in config
            if !self.config.plugins.enabled.contains(&registration.id.to_string()) {
                continue;
            }

            // Create plugin
            let plugin = (registration.create)(&self.config);
            self.plugins.push(plugin);
        }

        // Sort by priority, register keybinds
        self.sort_and_register_plugins();
    }
}
```

**No central match statement!** Add a plugin = create a file, that's it.

---

## PluginContext: System-Level Access

Plugins receive full runtime access:

```rust
struct PluginContext<'a> {
    // ===== Runtime State (Read/Write) =====

    /// Active app metadata
    active_app: &'a AppInfo,

    /// Complete layer stack (inspect/modify)
    layers: &'a mut Vec<Layer>,

    /// Focus state (read/write)
    focus: &'a mut FocusState,

    /// Request queues (inspect/modify)
    focus_queue: &'a mut VecDeque<FocusRequest>,
    event_queue: &'a mut VecDeque<Event>,

    /// Timing information
    frame_time: Duration,
    last_render: Instant,
}

impl PluginContext<'_> {
    // ===== Cross-Plugin Communication =====
    pub fn publish(&mut self, topic: &str, data: Value);
    pub fn poll_topic(&mut self, topic: &str) -> Vec<Value>;
    pub fn subscribe(&mut self, topic: &str);

    // ===== App Management =====
    pub fn available_apps(&self) -> &[AppInfo];
    pub fn navigate_to(&mut self, app_id: &str);
    pub fn active_app_id(&self) -> &str;

    // ===== Layer Management =====
    pub fn add_layer(&mut self, layer: Layer);
    pub fn remove_layer(&mut self, id: &str) -> Option<Layer>;
    pub fn find_layer(&self, id: &str) -> Option<&Layer>;

    // ===== Focus Management =====
    pub fn focus_request(&mut self, target: &str);
    pub fn focused_layer(&self) -> Option<&str>;
    pub fn focus_history(&self) -> &[String];

    // ===== Introspection =====
    pub fn active_app_keybinds(&self) -> &KeybindMap;
    pub fn plugin_keybinds(&self) -> Vec<(&str, &KeybindMap)>;
    pub fn global_keybinds(&self) -> &KeybindMap;
    pub fn user_app_layout_mode(&self) -> LayoutMode;

    // ===== Plugin State (Persistent) =====
    pub fn state<T: DeserializeOwned>(&self) -> Option<T>;
    pub fn set_state<T: Serialize>(&mut self, state: &T);
}
```

---

## Example: Help Plugin

```rust
// File: src/plugins/help.rs

use crate::plugin::{Plugin, PluginContext, register_plugin};

pub struct HelpPlugin {
    showing: bool,
    config: HelpConfig,
}

impl HelpPlugin {
    pub const ID: &'static str = "help";
    pub const NAME: &'static str = "Help Menu";

    fn from_config(config: &Config) -> Self {
        Self {
            showing: false,
            config: config.plugins.help.clone(),
        }
    }
}

impl Plugin for HelpPlugin {
    fn id(&self) -> &'static str { Self::ID }
    fn name(&self) -> &'static str { Self::NAME }
    fn create(config: &Config) -> Box<dyn Plugin> {
        Box::new(Self::from_config(config))
    }

    // Keybinds auto-registered by runtime
    fn keybinds(&self) -> KeybindMap {
        KeybindMap::new()
            .bind(KeyCode::F(1), "Toggle help", |plugin: &mut Self| {
                plugin.showing = !plugin.showing;
            })
    }

    // Inject help modal layer
    fn on_layers(&mut self, layers: &mut Vec<Layer>, ctx: &mut PluginContext) {
        if !self.showing { return; }

        // Gather all keybinds
        let app_keybinds = ctx.active_app_keybinds();
        let plugin_keybinds = ctx.plugin_keybinds();
        let global_keybinds = ctx.global_keybinds();

        layers.push(
            Layer::centered(80, 30, |ui| {
                ui.text("=== Help (F1 to close) ===");

                ui.section("Global", |ui| {
                    for (key, desc) in global_keybinds.iter() {
                        ui.text(format!("  {} - {}", key, desc));
                    }
                });

                for (plugin_id, keybinds) in plugin_keybinds {
                    ui.section(&format!("Plugin: {}", plugin_id), |ui| {
                        for (key, desc) in keybinds.iter() {
                            ui.text(format!("  {} - {}", key, desc));
                        }
                    });
                }

                ui.section(&format!("App: {}", ctx.active_app.name), |ui| {
                    for (key, desc) in app_keybinds.iter() {
                        ui.text(format!("  {} - {}", key, desc));
                    }
                });
            })
            .id("modal")  // → "system.help.modal"
            .name("Help Menu")
            .dim_below(true)
            .blocks_input(true)
        );
    }
}

// AUTO-REGISTRATION (just by existing!)
register_plugin!(HelpPlugin);

// Type-safe config via Options derive
#[derive(Debug, Clone, Options)]
pub struct HelpConfig {
    /// Show keybind cheatsheet
    #[options(default = "true")]
    pub show_keybinds: bool,

    /// Group keybinds by app
    #[options(default = "true")]
    pub group_by_app: bool,
}
```

**That's the entire plugin!** No runtime changes needed.

---

## Example: App Launcher Plugin

```rust
// File: src/plugins/launcher.rs

pub struct LauncherPlugin {
    showing: bool,
    search: String,
    selected: usize,
    config: LauncherConfig,
}

impl LauncherPlugin {
    pub const ID: &'static str = "launcher";
    pub const NAME: &'static str = "App Launcher";

    fn from_config(config: &Config) -> Self {
        Self {
            showing: false,
            search: String::new(),
            selected: 0,
            config: config.plugins.launcher.clone(),
        }
    }
}

impl Plugin for LauncherPlugin {
    fn id(&self) -> &'static str { Self::ID }
    fn name(&self) -> &'static str { Self::NAME }
    fn create(config: &Config) -> Box<dyn Plugin> {
        Box::new(Self::from_config(config))
    }

    fn keybinds(&self) -> KeybindMap {
        KeybindMap::new()
            .bind(
                KeyChord::ctrl(KeyCode::Char(' ')),
                "Open app launcher",
                |plugin: &mut Self| {
                    plugin.showing = true;
                    plugin.search.clear();
                    plugin.selected = 0;
                }
            )
    }

    // Intercept events when showing (consume Esc/Enter)
    fn on_event(&mut self, event: &Event, ctx: &mut PluginContext) -> EventFlow {
        if !self.showing {
            return EventFlow::Continue;
        }

        match event {
            Event::Key(KeyCode::Esc) => {
                self.showing = false;
                EventFlow::Consume  // Don't pass Esc to app
            }
            Event::Key(KeyCode::Enter) => {
                let apps = ctx.available_apps();
                let filtered = self.filter_apps(apps);
                if let Some(app) = filtered.get(self.selected) {
                    ctx.navigate_to(app.id);
                    self.showing = false;
                }
                EventFlow::Consume
            }
            _ => EventFlow::Continue
        }
    }

    fn on_layers(&mut self, layers: &mut Vec<Layer>, ctx: &mut PluginContext) {
        if !self.showing { return; }

        let apps = ctx.available_apps();
        let filtered = self.filter_apps(apps);

        layers.push(
            Layer::centered(60, 20, |ui| {
                ui.text("Launch App (Ctrl+Space)");

                ui.text_input(&mut self.search)
                    .id("search")
                    .auto_focus(true);

                ui.list(&filtered, self.selected)
                    .id("app_list")
                    .max_items(self.config.max_results);
            })
            .id("modal")  // → "system.launcher.modal"
            .dim_below(true)
            .blocks_input(true)
        );
    }
}

impl LauncherPlugin {
    fn filter_apps(&self, apps: &[AppInfo]) -> Vec<&AppInfo> {
        apps.iter()
            .filter(|app| {
                fuzzy_match(&app.name, &self.search) >= self.config.fuzzy_threshold
            })
            .take(self.config.max_results)
            .collect()
    }
}

register_plugin!(LauncherPlugin);

#[derive(Debug, Clone, Options)]
pub struct LauncherConfig {
    /// Maximum search results
    #[options(default = "10")]
    pub max_results: usize,

    /// Fuzzy search threshold (0.0-1.0)
    #[options(default = "0.7")]
    pub fuzzy_threshold: f64,
}
```

**Event interception** - plugin consumes events before app sees them.

---

## Example: Debug Overlay Plugin

```rust
// File: src/plugins/debug.rs

pub struct DebugPlugin {
    showing: bool,
    fps_history: VecDeque<f64>,
    event_log: VecDeque<String>,
    config: DebugConfig,
}

impl Plugin for DebugPlugin {
    fn id(&self) -> &'static str { "debug" }
    fn name(&self) -> &'static str { "Debug Overlay" }
    fn create(config: &Config) -> Box<dyn Plugin> {
        Box::new(Self::from_config(config))
    }

    fn keybinds(&self) -> KeybindMap {
        KeybindMap::new()
            .bind(KeyCode::F(12), "Toggle debug overlay", |plugin: &mut Self| {
                plugin.showing = !plugin.showing;
            })
    }

    // Log all events
    fn on_event(&mut self, event: &Event, ctx: &mut PluginContext) -> EventFlow {
        if self.showing {
            self.event_log.push_back(format!("{:?}", event));
            if self.event_log.len() > self.config.max_event_history {
                self.event_log.pop_front();
            }
        }
        EventFlow::Continue  // Always pass through
    }

    // Inject debug overlay
    fn on_layers(&mut self, layers: &mut Vec<Layer>, ctx: &mut PluginContext) {
        if !self.showing { return; }

        layers.push(
            Layer::anchor(Anchor::TopRight, 40, 20, |ui| {
                ui.text("=== Debug (F12) ===");

                if self.config.show_fps {
                    ui.text(format!("FPS: {:.1}", self.current_fps()));
                }

                if self.config.show_layers {
                    ui.text("Layers:");
                    for layer in layers.iter() {
                        ui.text(format!("  {}", layer.id()));
                    }
                }

                if self.config.show_focus {
                    ui.text("Focus:");
                    ui.text(format!("  Layer: {:?}", ctx.focused_layer()));
                    ui.text("  History:");
                    for layer in ctx.focus_history().iter().rev().take(3) {
                        ui.text(format!("    {}", layer));
                    }
                }

                ui.text("Recent Events:");
                for event in self.event_log.iter().rev().take(5) {
                    ui.text(format!("  {}", event));
                }
            })
            .id("overlay")  // → "system.debug.overlay"
            .blocks_input(false)  // Can see but not interact
        );
    }

    // Track FPS
    fn on_post_render(&mut self, ctx: &mut PluginContext) {
        let fps = 1.0 / ctx.frame_time.as_secs_f64();
        self.fps_history.push_back(fps);
        if self.fps_history.len() > 60 {
            self.fps_history.pop_front();
        }
    }
}

register_plugin!(DebugPlugin);

#[derive(Debug, Clone, Options)]
pub struct DebugConfig {
    /// Show FPS counter
    #[options(default = "true")]
    pub show_fps: bool,

    /// Show layer stack
    #[options(default = "true")]
    pub show_layers: bool,

    /// Show focus state
    #[options(default = "false")]
    pub show_focus: bool,

    /// Max event history entries
    #[options(default = "50")]
    pub max_event_history: usize,
}
```

**Multi-hook usage** - logs events, injects UI, monitors performance.

---

## Example: Header Plugin

```rust
// File: src/plugins/header.rs

pub struct HeaderPlugin {
    config: HeaderConfig,
}

impl Plugin for HeaderPlugin {
    fn id(&self) -> &'static str { "header" }
    fn name(&self) -> &'static str { "Header Bar" }
    fn create(config: &Config) -> Box<dyn Plugin> {
        Box::new(Self::from_config(config))
    }

    fn on_layers(&mut self, layers: &mut Vec<Layer>, ctx: &mut PluginContext) {
        // Don't render if app wants fullscreen
        if ctx.user_app_layout_mode() == LayoutMode::Fullscreen {
            return;
        }

        // Prepend header layer (appears at bottom of stack)
        layers.insert(0,
            Layer::dock_top(3, |ui| {
                if self.config.show_app_name {
                    ui.text(format!("App: {}", ctx.active_app.name));
                }
                if self.config.show_clock {
                    let time = chrono::Local::now()
                        .format(&self.config.clock_format);
                    ui.text(format!("Time: {}", time));
                }
            })
            .id("main")  // → "system.header.main"
            .blocks_input(false)
        );
    }
}

register_plugin!(HeaderPlugin);

#[derive(Debug, Clone, Options)]
pub struct HeaderConfig {
    /// Show clock in header
    #[options(default = "true")]
    pub show_clock: bool,

    /// Clock format string
    #[options(default = "\"%H:%M:%S\"")]
    pub clock_format: String,

    /// Show active app name
    #[options(default = "true")]
    pub show_app_name: bool,
}
```

**Layer prepending** - header appears under app layers.

---

## Configuration

Top-level config structure:

```rust
#[derive(Debug, Options)]
pub struct Config {
    #[options(nested)]
    pub plugins: PluginConfigs,
}

#[derive(Debug, Options)]
pub struct PluginConfigs {
    /// List of enabled plugins
    #[options(default = "default_enabled_plugins()")]
    pub enabled: Vec<String>,

    /// Plugin priority (lower = earlier execution)
    #[options(default = "default_priorities()")]
    pub priority: HashMap<String, u32>,

    // Per-plugin configs
    #[options(nested)]
    pub header: HeaderConfig,

    #[options(nested)]
    pub help: HelpConfig,

    #[options(nested)]
    pub launcher: LauncherConfig,

    #[options(nested)]
    pub debug: DebugConfig,
}

fn default_enabled_plugins() -> Vec<String> {
    vec!["header".into(), "help".into(), "launcher".into()]
}

fn default_priorities() -> HashMap<String, u32> {
    [
        ("header".into(), 10),
        ("help".into(), 20),
        ("launcher".into(), 20),
        ("debug".into(), 30),
    ].into()
}
```

**Config file** (`~/.config/dynamics/config.toml`):

```toml
[plugins]
enabled = ["header", "help", "launcher", "debug"]

[plugins.priority]
header = 10
help = 20
launcher = 20
debug = 30

[plugins.header]
show_clock = true
clock_format = "%H:%M:%S"
show_app_name = true

[plugins.help]
show_keybinds = true
group_by_app = true

[plugins.launcher]
max_results = 10
fuzzy_threshold = 0.7

[plugins.debug]
show_fps = true
show_layers = true
show_focus = false
max_event_history = 50
```

**Benefits:**
- ✅ Type-safe (Options derive validates)
- ✅ Automatic defaults
- ✅ Self-documenting (from doc comments)
- ✅ CLI integration (Options generates arg parser)

---

## Hook Execution Order

Runtime executes plugin hooks in phases:

```
┌─────────────────────────────────────┐
│ 1. EVENT PHASE                      │
│    Plugin.on_event() → EventFlow    │
│    (can consume event)              │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 2. APP UPDATE                       │
│    App.update() → Vec<Layer>        │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 3. LAYER INJECTION PHASE            │
│    Plugin.on_layers()               │
│    (can add/modify/remove layers)   │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 4. RENDER PHASE                     │
│    Renderer.render(layers)          │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 5. POST-RENDER PHASE                │
│    Plugin.on_post_render()          │
│    (metrics, cleanup)               │
└─────────────────────────────────────┘
```

**Plugin order determined by priority** (configured in `[plugins.priority]`).

---

## Adding a New Plugin

### Step 1: Create plugin file

```rust
// File: src/plugins/notifications.rs

use crate::plugin::{Plugin, PluginContext, register_plugin};

pub struct NotificationsPlugin {
    queue: VecDeque<Notification>,
    config: NotificationsConfig,
}

impl NotificationsPlugin {
    pub const ID: &'static str = "notifications";
    pub const NAME: &'static str = "Notifications";

    fn from_config(config: &Config) -> Self {
        Self {
            queue: VecDeque::new(),
            config: config.plugins.notifications.clone(),
        }
    }
}

impl Plugin for NotificationsPlugin {
    fn id(&self) -> &'static str { Self::ID }
    fn name(&self) -> &'static str { Self::NAME }
    fn create(config: &Config) -> Box<dyn Plugin> {
        Box::new(Self::from_config(config))
    }

    fn on_init(&mut self, ctx: &mut PluginContext) {
        ctx.subscribe("notification.show");
    }

    fn on_layers(&mut self, layers: &mut Vec<Layer>, ctx: &mut PluginContext) {
        // Poll for new notifications
        for msg in ctx.poll_topic("notification.show") {
            if let Ok(notif) = serde_json::from_value::<Notification>(msg) {
                self.queue.push_back(notif);
            }
        }

        // Remove expired
        let duration = Duration::from_secs(self.config.duration_secs);
        self.queue.retain(|n| n.created.elapsed() < duration);

        // Render toasts
        for (idx, notif) in self.queue.iter().take(self.config.max_concurrent).enumerate() {
            layers.push(
                Layer::anchor(Anchor::BottomRight, 40, 3, |ui| {
                    ui.text(&notif.message);
                })
                .id(&format!("toast_{}", idx))
                .blocks_input(false)
            );
        }
    }
}

register_plugin!(NotificationsPlugin);

#[derive(Debug, Clone, Options)]
pub struct NotificationsConfig {
    /// Toast duration in seconds
    #[options(default = "5")]
    pub duration_secs: u64,

    /// Maximum concurrent toasts
    #[options(default = "3")]
    pub max_concurrent: usize,
}

struct Notification {
    message: String,
    created: Instant,
}
```

### Step 2: Add config to main config struct

```rust
// File: src/config.rs

#[derive(Debug, Options)]
pub struct PluginConfigs {
    // ... existing fields ...

    #[options(nested)]
    pub notifications: NotificationsConfig,  // Add this
}
```

### Step 3: Enable in config file

```toml
[plugins]
enabled = ["header", "help", "launcher", "notifications"]

[plugins.notifications]
duration_secs = 5
max_concurrent = 3
```

**That's it!** Plugin auto-registers and loads. No runtime changes needed.

---

## Cross-Plugin Communication

Plugins communicate via pubsub:

```rust
// Producer plugin
impl MetricsPlugin {
    fn on_post_render(&mut self, ctx: &mut PluginContext) {
        ctx.publish("metrics.fps", json!(self.current_fps()));
    }
}

// Consumer plugin
impl DebugPlugin {
    fn on_init(&mut self, ctx: &mut PluginContext) {
        ctx.subscribe("metrics.fps");
    }

    fn on_post_render(&mut self, ctx: &mut PluginContext) {
        for msg in ctx.poll_topic("metrics.fps") {
            if let Some(fps) = msg.as_f64() {
                self.external_fps = fps;
            }
        }
    }
}
```

**Topics are strings** - no registration needed, just publish/subscribe.

---

## Plugin Priority

Execution order controlled by priority (lower = earlier):

```toml
[plugins.priority]
header = 10       # First (prepends layers at bottom)
metrics = 15      # Early (collects data)
help = 20         # Middle (normal modals)
launcher = 20     # Middle
debug = 30        # Last (overlays everything)
```

**Hook execution:**
- `on_event`: Priority order (header can't consume events debug needs)
- `on_layers`: Priority order (header prepends, debug appends)
- `on_post_render`: Priority order (metrics before debug)

---

## Best Practices

### ✅ Keep Plugins Focused
Each plugin does one thing well. Don't create monolithic plugins.

### ✅ Use EventFlow::Consume Sparingly
Only consume events you truly handle. Most plugins should return `Continue`.

### ✅ Respect blocks_input
Non-interactive overlays should use `.blocks_input(false)`.

### ✅ Use Pubsub for Cross-Plugin Communication
Don't depend on plugin load order - use topics.

### ✅ Validate Config
Use `#[options(validator = "validate_threshold")]` for complex validation.

### ✅ Document Hooks
Comment which hooks your plugin uses and why.

---

## Directory Structure

```
src/
├── main.rs
├── runtime.rs           # Runtime core (plugin-agnostic)
├── config.rs            # Config with Options derives
├── plugin/
│   ├── mod.rs           # Plugin trait + registration macro
│   └── context.rs       # PluginContext
└── plugins/
    ├── mod.rs           # Re-exports (for convenience)
    ├── header.rs        # Self-registering
    ├── help.rs          # Self-registering
    ├── launcher.rs      # Self-registering
    ├── debug.rs         # Self-registering
    └── notifications.rs # Self-registering
```

**Each plugin is independent** - add/remove files without touching runtime.

---

## See Also

- [Layers](../02-building-ui/layers.md) - Layer system (plugins inject layers)
- [Focus System](../04-user-interaction/focus.md) - Focus management (plugins can request focus)
- [Keybinds](../04-user-interaction/keybinds.md) - Keybind system (plugins declare keybinds)
- [Pub/Sub](../03-state-management/pubsub.md) - Cross-plugin communication

---

**Next:** Explore [App Launcher](app-launcher.md) or [Help System](help-system.md) plugin implementations.
