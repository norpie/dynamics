# App & Context API

**Prerequisites:** [Overview](../00-overview.md)

## App Trait

The App trait is the core interface for V2 applications:

```rust
trait App: 'static {
    // REQUIRED: App's unique identifier (single segment)
    fn id() -> &'static str;

    // Called once on creation
    fn new(ctx: &AppContext) -> Self;

    // Called on every event or invalidation - returns UI + layers
    fn update(&mut self, ctx: &mut Context) -> Vec<Layer>;

    // Optional: Define app-specific keybinds
    fn keybinds() -> KeybindMap {
        KeybindMap::new()
    }

    // Optional lifecycle
    fn on_background(&mut self) {}
    fn on_foreground(&mut self) {}
}
```

**Key differences from V1:**
- No separate `view()` method - `update()` handles events AND returns UI
- Mandatory `id()` method for hierarchical path construction

### App ID Requirements

Each app declares its own **single segment** ID - the runtime composes the full path:

```rust
impl App for MigrationApp {
    fn id() -> &'static str {
        "migration"  // Just the app name, NOT "apps.migration"!
    }
}

// Runtime composes: "apps.migration.*"
```

**Validation rules:**
- Lowercase letters, numbers, `_`, and `-` only
- No dots (`.`) - these are path separators
- No `_` prefix (reserved for auto-generated IDs)
- No `system` keyword (reserved for runtime)

**See Also:**
- [Lifecycle Hooks](lifecycle.md) - Full lifecycle details
- [Keybinds](../04-user-interaction/keybinds.md) - Keybind system
- [Layers](../02-building-ui/layers.md) - Layer composition

## Context API

The Context provides access to framework services:

```rust
struct Context {
    // Hierarchical ID path stack (managed by runtime)
    id_stack: Vec<String>,  // e.g., ["apps", "migration"]

    // View routing (multi-view apps)
    router: Router,

    // Task spawning with auto-polling
    tasks: TaskManager,

    // Pub/sub with auto-routing
    pubsub: PubSub,

    // UI builder (immediate mode)
    ui: UiBuilder,

    // Focus management
    focus: FocusManager,
}
```

**Context Services:**
- **id_stack** - Hierarchical path for ID composition (read-only for apps)
- **router** - Navigate between views in multi-view apps
- **tasks** - Spawn async tasks with automatic polling
- **pubsub** - Subscribe to and publish messages
- **ui** - Build UI elements (immediate mode)
- **focus** - Programmatic focus control

### Path Management

The context maintains an ID stack for hierarchical path construction:

```rust
impl Context {
    // Get current full path
    pub fn full_path(&self) -> String {
        self.id_stack.join(".")
    }

    // Create nested context for sub-components
    pub fn with_segment<F, R>(&mut self, segment: &str, f: F) -> R
    where F: FnOnce(&mut Context) -> R
    {
        self.id_stack.push(segment.to_string());
        let result = f(self);
        self.id_stack.pop();
        result
    }
}
```

**Example usage:**
```rust
// ctx.id_stack = ["apps", "migration"]

// Nested context for a view
ctx.with_segment("environment", |ctx| {
    // ctx.id_stack = ["apps", "migration", "environment"]
    Layer::fill(ui).id("main")  // → "apps.migration.environment.main"
})

// ctx.id_stack back to ["apps", "migration"]
```

**See Also:**
- [Multi-View Routing](../03-state-management/routing.md) - Router usage
- [Resource Pattern](../03-state-management/resource-pattern.md) - Async task management
- [Pub/Sub](../03-state-management/pubsub.md) - Message passing
- [Elements](elements.md) - UI builder usage
- [Plugin System](../06-system-features/plugin-system.md) - Plugins use extended PluginContext

## Simple App Example

Minimal example showing the pattern:

```rust
struct QueueApp {
    list_state: ListState,
    items: Vec<QueueItem>,
}

impl App for QueueApp {
    fn id() -> &'static str {
        "queue"  // App declares only its segment
    }

    fn new(ctx: &AppContext) -> Self {
        Self {
            list_state: ListState::default(),
            items: Vec::new(),
        }
    }

    fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
        // ctx.id_stack = ["apps", "queue"]
        vec![
            Layer::fill(panel("Queue", |ui| {
                ui.text(format!("{} items", self.items.len()));
                ui.list(&mut self.list_state, &self.items, |item, ui| {
                    ui.text(&item.name);
                });
            }))
            .id("main")  // → "apps.queue.main"
        ]
    }

    // Handlers as separate methods (can be async!)
    async fn handle_clear(&mut self, ctx: &mut Context) {
        self.items.clear();
    }
}
```

**Key patterns:**
- **Mandatory ID** - Each app declares its segment via `id()`
- **Hierarchical paths** - Runtime composes "apps.queue.main"
- **Direct state mutation** - `self.items.clear()`
- **Immediate-mode UI** - `ui.text()`, `ui.list()`
- **Async handlers** - Methods can be async

**See Also:**
- [Component Patterns](../04-user-interaction/component-patterns.md) - Callbacks and state
- [Layout](../02-building-ui/layout.md) - Panel and layout primitives

---

**Next:** Learn about [Lifecycle Hooks](lifecycle.md) or explore [Element Building](elements.md).
