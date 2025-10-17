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

---

## Progress Tracking

V2 extends the Resource pattern with progress information for long-running async operations.

### Progress Types

```rust
pub enum Progress {
    /// No progress information (shows spinner, no percentage)
    Indeterminate,

    /// Step-based progress - X of Y units completed
    Steps {
        current: u64,
        total: u64,
        label: Option<String>,
    },

    /// Percentage-based (0.0 - 1.0)
    Percentage {
        value: f32,
        label: Option<String>,
    },

    /// Multi-phase operation with sub-progress
    Phase {
        current: usize,
        total: usize,
        name: String,
        phase_progress: Option<Box<Progress>>,
    },

    /// Elapsed time (unknown duration)
    Elapsed {
        elapsed: Duration,
    },

    /// Time-based with known duration
    Duration {
        elapsed: Duration,
        total: Duration,
    },
}

// Updated Resource enum
pub enum Resource<T, E = String> {
    NotAsked,
    Loading(Progress),  // Always has Progress
    Success(T),
    Failure(E),
}
```

### Helper Methods

```rust
// Create loading states
Resource::loading()                      // Indeterminate
Resource::loading_steps(5, 10)           // 5/10 complete
Resource::loading_percentage(0.45)       // 45%
Resource::loading_elapsed(duration)      // Show elapsed time

// Update progress
resource.update_progress(Progress::Steps { current: 7, total: 10, label: None });

// Get progress info
if let Some(pct) = resource.progress().and_then(|p| p.as_percentage()) {
    println!("{}% complete", pct * 100.0);
}
```

### Usage Examples

**Step-based progress:**
```rust
for (i, entity) in entities.iter().enumerate() {
    state.comparisons = Resource::Loading(Progress::Steps {
        current: i as u64,
        total: entities.len() as u64,
        label: Some(format!("Loading {}", entity.name)),
    });

    let comparison = fetch_comparison(entity).await;
}
state.comparisons = Resource::Success(all_comparisons);
```

**Multi-phase with sub-progress:**
```rust
// Phase 1: Downloading
state.file = Resource::Loading(Progress::Phase {
    current: 0,
    total: 3,
    name: "Downloading".to_string(),
    phase_progress: Some(Box::new(Progress::Steps {
        current: bytes_downloaded,
        total: total_bytes,
        label: Some(format!("{:.1} MB / {:.1} MB", /* ... */)),
    })),
});

// Phase 2: Parsing (indeterminate)
state.file = Resource::Loading(Progress::Phase {
    current: 1,
    total: 3,
    name: "Parsing".to_string(),
    phase_progress: Some(Box::new(Progress::Indeterminate)),
});
```

### Updating from Async Tasks

**Channel-based updates:**
```rust
let (tx, mut rx) = mpsc::channel(100);
ctx.spawn(async move {
    for (i, item) in items.iter().enumerate() {
        fetch(item).await;
        tx.send(Progress::Steps { current: i as u64, total: items.len() as u64, label: None }).await;
    }
});

// In update()
while let Ok(progress) = self.progress_rx.try_recv() {
    self.data.update_progress(progress);
}
```

### UI Rendering

```rust
match &self.data {
    Resource::Loading(progress) => {
        match progress {
            Progress::Indeterminate => ui.spinner("Loading..."),
            prog if prog.as_percentage().is_some() => {
                ui.progress_bar(prog.as_percentage().unwrap())
                    .label(prog.display());
            }
            prog => ui.spinner(prog.display()),
        }
    }
    Resource::Success(data) => { /* ... */ }
    Resource::Failure(err) => { /* ... */ }
    Resource::NotAsked => { /* ... */ }
}
```

### Migration from V1

**V1:**
```rust
state.data = Resource::Loading;
```

**V2:**
```rust
state.data = Resource::loading();  // Helper method
// Or explicit:
state.data = Resource::Loading(Progress::Indeterminate);
```

---

**See Also:**
- [Error Recovery](error-recovery.md) - Retry strategies and error handling
- [Background Work](background-work.md) - Background task patterns

---

**Next:** Learn about [Error Recovery](error-recovery.md) strategies.
