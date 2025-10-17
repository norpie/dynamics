# Resource Pattern (Auto-Managed Async)

**Prerequisites:** [App & Context API](../01-fundamentals/app-and-context.md)

## Overview

The Resource pattern replaces manual loading flags with a typed enum that represents async state:

```rust
enum Resource<T, E = String> {
    NotAsked,
    Loading,
    Success(T),
    Failure(E),
}
```

**Benefits:**
- No manual `is_loading` flags
- Type-safe state representation
- Framework handles spawning, polling, and invalidation
- Built-in UI rendering helpers

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

**See Also:**
- [Resource Progress](resource-pattern.md#progress-tracking) - Progress tracking (to be added)
- [Error Recovery](error-recovery.md) - Retry strategies
- [Background Work](../07-advanced/background-work.md) - Background task patterns

---

**Next:** Learn about [Error Recovery](error-recovery.md) strategies.
