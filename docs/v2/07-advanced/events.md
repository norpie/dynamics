# Event Broadcast System

**Prerequisites:** [App and Context](../01-fundamentals/app-and-context.md)

## Overview

The event broadcast system provides **fire-and-forget pub/sub** for state change notifications. Multiple subscribers can listen to the same topic with best-effort delivery.

**Use for:** State change notifications, user actions, settings updates
**Don't use for:** Operations requiring guaranteed delivery (use [Work Queues](queues.md) instead)

---

## Publishing Events

Type-safe publishing with automatic serialization:

```rust
// Publish with any serializable type
ctx.broadcast("migration:selected", migration_id);  // migration_id: String
ctx.broadcast("theme:changed", new_theme);          // new_theme: Theme struct
ctx.broadcast("file:saved", SaveEvent {
    path: "/path/to/file",
    timestamp: now,
});
```

---

## Subscribing to Events

Apps declare subscriptions in `new()` and poll events in `update()`:

```rust
impl App for MyApp {
    fn new(ctx: &AppContext) -> Self {
        // Register interest in typed events
        ctx.subscribe::<String>("migration:selected");
        ctx.subscribe::<Theme>("theme:changed");
        ctx.subscribe::<SaveEvent>("file:saved");

        Self { /* ... */ }
    }

    fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
        // Type-safe polling - no manual deserialization!
        while let Some(id) = ctx.poll_event::<String>("migration:selected") {
            self.load_migration(id);
        }

        while let Some(theme) = ctx.poll_event::<Theme>("theme:changed") {
            self.theme = theme;
        }

        // ... render UI
    }
}
```

**No manual deserialization** - the type system handles serialization automatically.

---

## Persistent Subscriptions

By default, events are **dropped if the subscribing app is backgrounded**. For apps that need events while backgrounded:

```rust
ctx.subscribe::<String>("migration:selected")
    .persistent(true);  // Queues events while app is backgrounded
```

**Persistent subscription behavior:**
- Events are queued while app is backgrounded
- Delivered when app returns to foreground
- Warning toasts at 100/500/1000 queued events
- Unlimited queue size (for future app management UI to clear)

**When to use persistent subscriptions:**
- Backgrounded app needs to act on events when it returns to foreground
- Examples: operation queue receiving work while backgrounded, migration app tracking progress
- **Don't use** for transient UI updates that are irrelevant once outdated

---

## Event System Characteristics

| Property | Behavior |
|----------|----------|
| **Delivery** | Best-effort, multiple subscribers |
| **Ordering** | No guarantees |
| **Retry** | None |
| **Persistence** | Optional (only for persistent subscriptions) |
| **Use cases** | State change notifications, user actions, settings updates |

---

## Examples

### Example 1: Migration Events

```rust
// Migration app (event publisher)
impl App for MigrationApp {
    fn on_migration_selected(&mut self, ctx: &mut Context, migration_id: String) {
        self.selected_migration = Some(migration_id.clone());

        // Broadcast to all interested apps
        ctx.broadcast("migration:selected", migration_id);
    }
}

// Entity comparison app (event subscriber)
impl App for EntityComparisonApp {
    fn new(ctx: &AppContext) -> Self {
        ctx.subscribe::<String>("migration:selected")
            .persistent(true);  // Queue events while backgrounded

        Self { /* ... */ }
    }

    fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
        // Poll for migration selection events
        while let Some(migration_id) = ctx.poll_event::<String>("migration:selected") {
            self.load_migration_data(ctx, migration_id);
        }

        // ... render UI
    }
}
```

### Example 2: Theme Changes

```rust
// Settings app (event publisher)
impl App for SettingsApp {
    fn on_theme_changed(&mut self, ctx: &mut Context, new_theme: Theme) {
        // Save to config
        self.config.set_theme(new_theme.clone()).await;

        // Broadcast to all apps
        ctx.broadcast("theme:changed", new_theme);
    }
}

// Any app (event subscriber)
impl App for AnyApp {
    fn new(ctx: &AppContext) -> Self {
        ctx.subscribe::<Theme>("theme:changed");
        Self { /* ... */ }
    }

    fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
        // React to theme changes
        while let Some(theme) = ctx.poll_event::<Theme>("theme:changed") {
            self.theme = theme;
            // UI will re-render with new theme
        }

        // ... render UI
    }
}
```

---

## Type Safety Architecture

The event system uses **type erasure at runtime boundaries** to avoid generic explosion while providing type safety to apps:

```rust
// Runtime storage (type-erased)
pub struct EventBus {
    subscribers: HashMap<String, Vec<Box<dyn ErasedSubscriber>>>,
}

// Apps work with typed subscriptions
ctx.subscribe::<String>("topic");
ctx.poll_event::<String>("topic");  // Type-safe!
```

---

## See Also

- [Work Queues](queues.md) - Guaranteed delivery task processing
- [App and Context](../01-fundamentals/app-and-context.md) - Context API reference
- [Background Work](../03-state-management/background-work.md) - Async task patterns

---

**Next:** Learn about [Work Queues](queues.md) for guaranteed delivery, or explore [Background Work](../03-state-management/background-work.md).
