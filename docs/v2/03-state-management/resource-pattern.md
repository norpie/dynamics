# Resource Pattern

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md)

## Overview

The Resource pattern provides typed async state management with automatic progress tracking and structured error recovery.

```rust
enum Resource<T> {
    NotAsked,
    Loading(Progress),
    Success(T),
    Failure {
        error: ResourceError,
        retry_count: usize,
    },
}
```

**Benefits:**
- No manual `is_loading` flags
- Type-safe state representation
- Framework handles spawning, polling, and invalidation
- Built-in progress tracking and error recovery
- Automatic retry with exponential backoff

---

## Basic Usage

```rust
struct MyApp {
    data: Resource<Data>,
}

fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
    vec![Layer::fill(panel("Data", |ui| {
        if ui.button("Load").clicked() {
            // Framework handles spawning, polling, invalidation
            self.data.load(ctx, async {
                fetch_data().await
            });
        }

        // Resource has built-in render method
        self.data.render(ui,
            || spinner(),           // Loading
            |data| text(data),      // Success
            |err| error(err),       // Failure
        );
    }))]
}
```

## How It Works

`ctx.spawn_into()` (called by `resource.load()`):
1. Spawns async task
2. Wraps in Arc/Mutex
3. Updates Resource when complete
4. Auto-invalidates UI

**No manual polling or state tracking needed!**

---

## Advanced Topics

This document covers basic Resource usage. For detailed information on specific features:

### Progress Tracking

See [Resource Progress](resource-progress.md) for:
- Progress types (Indeterminate, Steps, Percentage, Phase, Duration)
- Helper methods for creating loading states
- Step-based and multi-phase progress examples
- Channel-based progress updates from async tasks
- UI rendering patterns

### Error Handling & Retry

See [Resource Errors](resource-errors.md) for:
- ErrorKind types (Transient, RateLimit, Authentication, etc.)
- RetryStrategy configuration
- Automatic error conversions (String, io::Error, reqwest::Error)
- Manual and automatic retry patterns
- Error UI rendering
- Migration from V1

---

## See Also

- [Resource Progress](resource-progress.md) - Progress tracking
- [Resource Errors](resource-errors.md) - Error handling and retry
- [Background Work](background-work.md) - Background task patterns
- [Pub/Sub](../07-advanced/events-and-queues.md) - Event-driven communication

---

**Next:** Learn about [Progress Tracking](resource-progress.md), [Error Recovery](resource-errors.md), or [Multi-View Routing](routing.md).
