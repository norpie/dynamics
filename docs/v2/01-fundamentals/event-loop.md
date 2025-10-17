# Event-Driven Rendering

**Prerequisites:** [App & Context API](app-and-context.md)

## Overview

V2 uses **event-driven rendering** - no continuous rendering loop!

**Framework only calls `update()` when:**
- User input event (keyboard, mouse)
- Resource finishes loading
- Pub/sub message received
- Timer fires
- Explicit invalidation (`ctx.invalidate()`)

**Benefits:**
- **CPU efficient** - Only renders when needed
- **Battery friendly** - No wasted cycles
- **Responsive** - Immediate response to user input

## Explicit Invalidation

Apps can request re-render via `ctx.invalidate()`:

```rust
async fn handle_save(&mut self, ctx: &mut Context) {
    self.save_status = "Saving...";
    ctx.invalidate();  // Update UI immediately

    save_to_disk(&self.data).await;

    self.save_status = "Saved!";
    // Framework auto-invalidates when async completes
}
```

**Note:** Most cases don't need explicit invalidation - the framework handles it automatically (user input, resource completion, pub/sub messages).

**See Also:**
- [Resource Pattern](../03-state-management/resource-pattern.md) - Auto-invalidation on async completion
- [Pub/Sub](../03-state-management/pubsub.md) - Auto-invalidation on message receipt
- [Background Work](../07-advanced/background-work.md) - Invalidation patterns

---

**Next:** Learn about [Lifecycle Hooks](lifecycle.md) or explore [Elements](elements.md).
