# Background Apps

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md), [Pub/Sub](../03-state-management/pubsub.md)

## Overview

Apps can continue processing while not visible, useful for background tasks like operation queues or file watchers.

## Basic Usage

```rust
impl App for OperationQueue {
    fn new(ctx: &AppContext) -> Self {
        // Mark this app as always active
        ctx.set_lifecycle(Lifecycle::AlwaysActive);

        // Subscribe to messages (works even when in background!)
        ctx.subscribe("operations:add", Self::on_operations_received);

        Self { queue: Vec::new() }
    }

    fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
        // Process queue even when user is viewing other apps
        if self.current.is_none() && !self.queue.is_empty() {
            let op = self.queue.remove(0);
            self.current = Some(op.clone());
            self.progress.load(ctx, execute_operation(op));
        }

        vec![Layer::fill(self.render_queue_ui())]
    }
}
```

## Lifecycle States

- **Lifecycle::Normal** - Paused when not visible (default)
- **Lifecycle::AlwaysActive** - Continues running in background

## Use Cases

**Good for:**
- Operation queues
- File watchers
- Background sync tasks
- System monitors

**Avoid for:**
- Heavy computation (blocks UI thread)
- Apps that don't need background processing

**See Also:**
- [Pub/Sub](../03-state-management/pubsub.md) - Background message handling
- [Lifecycle](../01-fundamentals/lifecycle.md) - Lifecycle hooks and states
- [Background Work](../07-advanced/background-work.md) - Background task patterns

---

**Next:** Explore [App Launcher](app-launcher.md) or [Help System](help-system.md).
