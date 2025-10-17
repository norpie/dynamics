# Pub/Sub (Auto-Managed)

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md)

## Overview

V2 provides automatic pub/sub messaging between apps. The framework handles all synchronization (Arc/Mutex/RwLock) internally.

**Key feature:** No manual polling - framework calls handler methods automatically.

## Subscribing to Messages

Subscribe in `new()` with a handler method reference:

```rust
impl App for OperationQueue {
    fn new(ctx: &AppContext) -> Self {
        ctx.subscribe("operations:add", Self::on_operations_received);
        Self { queue: Vec::new() }
    }

    // Called automatically when message arrives (even if app is in background!)
    fn on_operations_received(&mut self, msg: Message) {
        let ops: Vec<Operation> = msg.parse();
        self.queue.extend(ops);
    }

    fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
        // No manual polling needed!
        vec![Layer::fill(panel("Queue", |ui| {
            ui.list(&mut self.queue_state, &self.queue);
        }))]
    }
}
```

## Publishing Messages

Publish from any handler via `ctx.pubsub`:

```rust
async fn handle_save(&mut self, ctx: &mut Context) {
    ctx.pubsub.publish("operations:add", &self.data);
}
```

**See Also:**
- [Background Apps](../06-system-features/background-apps.md) - Apps that run in background
- [Events & Queues](../07-advanced/events-and-queues.md) - Type-safe event system
- [Lifecycle](../01-fundamentals/lifecycle.md) - App lifecycle and background state

---

**Next:** Learn about [Resource Pattern](resource-pattern.md) for async state.
