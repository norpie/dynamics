# Background Work + Invalidation

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md), [Event Loop](../01-fundamentals/event-loop.md)

## Problem

Apps need to do work while user is idle and trigger UI updates when progress happens.

## Solution: Invalidator API

Apps spawn background tasks and use `ctx.invalidator()` to trigger re-renders:

```rust
async fn process_batch(&mut self, ctx: &mut Context) {
    let invalidator = ctx.invalidator();
    let progress = Arc::new(AtomicU32::new(0));
    let progress_clone = progress.clone();

    ctx.spawn(async move {
        for i in 0..100 {
            process_step(i).await;
            progress_clone.store(i, Ordering::Relaxed);
            invalidator.invalidate();  // Trigger re-render
        }
    });

    self.progress = progress;
}
```

## Common Patterns

### Pattern 1: Progress Updates

Background task updates shared state and triggers re-render:

```rust
fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    let progress = self.progress.load(Ordering::Relaxed);

    vec![
        Layer::fill(panel("Processing", |ui| {
            ui.progress_bar(progress, 100);
        }))
    ]
}
```

### Pattern 2: File Watching

Monitor filesystem and invalidate on changes:

```rust
async fn watch_config(&mut self, ctx: &mut Context) {
    let invalidator = ctx.invalidator();

    ctx.spawn(async move {
        let mut watcher = notify::watcher(...);
        while watcher.changed().await {
            invalidator.invalidate();  // File changed, re-render
        }
    });
}
```

### Pattern 3: Periodic Polling

Poll API periodically and trigger updates:

```rust
async fn poll_api(&mut self, ctx: &mut Context) {
    let invalidator = ctx.invalidator();

    ctx.spawn(async move {
        loop {
            fetch_and_update().await;
            invalidator.invalidate();
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}
```

## Event Sources

See [Event-Driven Rendering](../01-fundamentals/event-loop.md#event-sources) for complete details on event sources and latency characteristics.

## Benefits

✅ **Simple API** - Just call `invalidate()` when something changes
✅ **Event-driven** - Runtime only wakes when needed
✅ **Low latency** - Updates trigger immediately
✅ **Composable** - Works with async tasks, file watchers, timers

**See Also:**
- [Resource Pattern](resource-pattern.md) - Async state management
- [Event Loop](../01-fundamentals/event-loop.md) - Event-driven rendering
- [Background Apps](../06-system-features/background-apps.md) - Apps that run in background

---

**Next:** Learn about [Resource Pattern](resource-pattern.md) or explore [Events & Queues](../07-advanced/events-and-queues.md).
