# App & Context API

**Prerequisites:** [Overview](../00-overview.md)

## App Trait

The App trait is the core interface for V2 applications:

```rust
trait App: 'static {
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

**Key difference from V1:** No separate `view()` method - `update()` handles events AND returns UI.

**See Also:**
- [Lifecycle Hooks](lifecycle.md) - Full lifecycle details
- [Keybinds](../04-user-interaction/keybinds.md) - Keybind system
- [Layers](../02-building-ui/layers.md) - Layer composition

## Context API

The Context provides access to framework services:

```rust
struct Context {
    // View routing (multi-view apps)
    router: Router,

    // Task spawning with auto-polling
    tasks: TaskManager,

    // Pub/sub with auto-routing
    pubsub: PubSub,

    // UI builder (immediate mode)
    ui: UiBuilder,
}
```

**Context Services:**
- **router** - Navigate between views in multi-view apps
- **tasks** - Spawn async tasks with automatic polling
- **pubsub** - Subscribe to and publish messages
- **ui** - Build UI elements (immediate mode)

**See Also:**
- [Multi-View Routing](../03-state-management/routing.md) - Router usage
- [Resource Pattern](../03-state-management/resource-pattern.md) - Async task management
- [Pub/Sub](../03-state-management/pubsub.md) - Message passing
- [Elements](elements.md) - UI builder usage

## Simple App Example

Minimal example showing the pattern:

```rust
struct QueueApp {
    list_state: ListState,
    items: Vec<QueueItem>,
}

impl App for QueueApp {
    fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
        vec![
            Layer::fill(panel("Queue", |ui| {
                ui.text(format!("{} items", self.items.len()));
                ui.list(&mut self.list_state, &self.items, |item, ui| {
                    ui.text(&item.name);
                });
            }))
        ]
    }

    // Handlers as separate methods (can be async!)
    async fn handle_clear(&mut self, ctx: &mut Context) {
        self.items.clear();
    }
}
```

**Key patterns:**
- Direct state mutation (`self.items.clear()`)
- Immediate-mode UI building (`ui.text()`, `ui.list()`)
- Async handlers supported
- Layers returned directly from `update()`

**See Also:**
- [Component Patterns](../04-user-interaction/component-patterns.md) - Callbacks and state
- [Layout](../02-building-ui/layout.md) - Panel and layout primitives

---

**Next:** Learn about [Lifecycle Hooks](lifecycle.md) or explore [Element Building](elements.md).
