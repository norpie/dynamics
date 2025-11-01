# Events & Queue System

**Prerequisites:**
- [App and Context](../01-fundamentals/app-and-context.md) - Understanding the Context API

V2 provides **two parallel communication systems** for inter-app communication:

1. **Event Broadcast** - Fire-and-forget notifications for state changes (pub-sub)
2. **Work Queues** - Guaranteed delivery task processing with persistence and priorities

**Core principle**: Events are for notifications, queues are for work items. Don't use pub-sub for operations that need guaranteed delivery.

---

## Event Broadcast System

**Purpose:** Notify interested apps about state changes. Multiple subscribers, best-effort delivery.

### Publishing Events

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

### Subscribing to Events

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

### Persistent Subscriptions

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

### Event System Characteristics

| Property | Behavior |
|----------|----------|
| **Delivery** | Best-effort, multiple subscribers |
| **Ordering** | No guarantees |
| **Retry** | None |
| **Persistence** | Optional (only for persistent subscriptions) |
| **Use cases** | State change notifications, user actions, settings updates |

---

## Work Queue System

**Purpose:** Process work items with exactly-once delivery, persistence, priorities, and recovery.

### Queue Registration

Any app can create a work queue. Runtime warns if duplicate queue name is registered (toast notification).

```rust
impl App for OperationQueue {
    fn new(ctx: &AppContext) -> Self {
        // Register as queue owner (warns if duplicate)
        ctx.register_work_queue::<Operation>("operation_queue")
            .expect("Queue registration failed");

        Self {
            queue: WorkQueue::new("operation_queue", ctx),
            // ... other state
        }
    }
}
```

### Sending Work to Queues

Any app can send typed work items to a registered queue:

```rust
// Send typed work to a queue
ctx.send_work("operation_queue", Operation {
    endpoint: "/api/contacts",
    method: "POST",
    body: contact_data,
}, Priority::Normal);

// Priority enum
pub enum Priority {
    Critical = 0,      // Highest priority
    High = 64,
    Normal = 128,      // Default
    Low = 192,
    Background = 255,  // Lowest priority
}

// Or custom u8 priority (0 = highest, 255 = lowest)
ctx.send_work_priority("operation_queue", item, 42);
```

### Processing Queue Items

Queue owner processes items at its own pace:

```rust
impl App for OperationQueue {
    fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
        // Process items from queue with concurrency control
        while self.can_run_more() {
            if let Some(item) = self.queue.pop() {
                // Type-safe - no manual deserialization!
                ctx.spawn(async move {
                    self.execute_operation(item).await
                });
            } else {
                break;  // Queue empty
            }
        }

        // ... render UI showing queue status
    }
}
```

### WorkQueue API

```rust
pub struct WorkQueue<T> {
    name: String,
    items: BTreeMap<u8, VecDeque<T>>,  // Priority -> items
    storage: QueueStorage,
}

impl<T: Serialize + DeserializeOwned> WorkQueue<T> {
    /// Create queue (auto-loads from disk)
    pub fn new(name: &str, ctx: &AppContext) -> Self;

    /// Push item with priority (0 = highest, 255 = lowest)
    pub fn push(&mut self, item: T, priority: u8);

    /// Pop highest-priority item (FIFO within priority)
    pub fn pop(&mut self) -> Option<T>;

    /// Peek at next item without removing
    pub fn peek(&self) -> Option<(&T, u8)>;

    /// Count items at specific priority
    pub fn count(&self, priority: u8) -> usize;

    /// Total items across all priorities
    pub fn len(&self) -> usize;

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool;
}
```

### Queue Persistence

Work queues automatically persist to SQLite:

- **Write-through**: Items are written to disk immediately on push
- **Auto-load**: Queue items are loaded from disk on `WorkQueue::new()`
- **Crash recovery**: All queued items survive app crashes/restarts

```sql
-- SQLite schema (internal)
CREATE TABLE queue_items (
    id TEXT PRIMARY KEY,
    queue_name TEXT NOT NULL,
    priority INTEGER NOT NULL,
    data_json TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_queue_priority (queue_name, priority, created_at)
);
```

### Queue System Characteristics

| Property | Behavior |
|----------|----------|
| **Delivery** | Exactly-once, single consumer (queue owner) |
| **Ordering** | FIFO within priority, priority-ordered globally |
| **Retry** | App-defined (queue owner decides) |
| **Persistence** | Always (SQLite backed) |
| **Use cases** | API operations, batch jobs, background tasks |

---

## Type Safety Architecture

Both systems use **type erasure at runtime boundaries** to avoid generic explosion while providing type safety to apps.

### Implementation Pattern

```rust
// Runtime storage (type-erased)
pub struct QueueRegistry {
    queues: HashMap<String, Box<dyn ErasedQueue>>,
}

trait ErasedQueue {
    fn push_value(&mut self, value: Value, priority: u8);
    fn pop_value(&mut self) -> Option<Value>;
    fn len(&self) -> usize;
}

impl<T: Serialize + DeserializeOwned> ErasedQueue for WorkQueue<T> {
    fn push_value(&mut self, value: Value, priority: u8) {
        let item: T = serde_json::from_value(value)
            .expect("Failed to deserialize queue item");
        self.push(item, priority);
    }

    fn pop_value(&mut self) -> Option<Value> {
        self.pop().map(|item|
            serde_json::to_value(item).expect("Failed to serialize queue item")
        )
    }

    fn len(&self) -> usize {
        self.len()
    }
}

// Context methods use type erasure
impl AppContext {
    pub fn register_work_queue<T: Serialize + DeserializeOwned + 'static>(
        &mut self,
        name: &str
    ) -> Result<(), QueueError> {
        let queue = WorkQueue::<T>::new(name, self);
        self.registry.register(name, Box::new(queue))
    }

    pub fn send_work<T: Serialize>(
        &mut self,
        queue_name: &str,
        item: T,
        priority: Priority,
    ) {
        let value = serde_json::to_value(item)
            .expect("Failed to serialize work item");
        self.registry.send(queue_name, value, priority as u8);
    }
}
```

**Result:** Apps work with typed `WorkQueue<Operation>` and `poll_event::<String>()`, while runtime stores everything as `serde_json::Value`. No generic explosion.

---

## Usage Guidelines

| Scenario | System | Priority/Persistent |
|----------|--------|---------------------|
| Notify multiple apps of state change | Event broadcast | N/A |
| Send work item to specific app | Work queue | Normal |
| Critical API operations that can't be lost | Work queue | Critical |
| Background cleanup tasks | Work queue | Background |
| User triggered UI event | Event broadcast | N/A |
| Theme/settings changed | Event broadcast | Persistent if needed |
| Batch operations (Excel import) | Work queue | Normal/Low |
| File watcher notifications | Event broadcast | N/A |
| Migration progress updates | Event broadcast | N/A |

---

## Examples

### Example 1: Operation Queue

```rust
// OperationQueue app (work consumer)
impl App for OperationQueue {
    fn new(ctx: &AppContext) -> Self {
        ctx.register_work_queue::<QueueItem>("operation_queue")
            .expect("Queue registration");

        Self {
            queue: WorkQueue::new("operation_queue", ctx),
            max_concurrent: 3,
            currently_running: HashSet::new(),
        }
    }

    fn update(&mut self, ctx: &mut Context) -> Vec<Layer> {
        // Execute items with concurrency limit
        while self.currently_running.len() < self.max_concurrent {
            if let Some(item) = self.queue.pop() {
                let id = item.id.clone();
                self.currently_running.insert(id.clone());

                ctx.spawn(async move {
                    let result = self.execute_item(item).await;
                    // Notify self when done
                    ctx.send_message(ExecutionCompleted { id, result });
                });
            } else {
                break;
            }
        }

        // Render queue UI...
    }
}

// Deadlines app (work producer)
impl App for DeadlinesApp {
    fn on_import_complete(&mut self, ctx: &mut Context, operations: Vec<Operation>) {
        // Send each operation to queue with priority
        for op in operations {
            let priority = if op.is_urgent {
                Priority::High
            } else {
                Priority::Normal
            };

            ctx.send_work("operation_queue", QueueItem::new(op), priority);
        }
    }
}
```

### Example 2: Migration Events

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

### Example 3: Theme Changes

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

## See Also

- [App and Context](../01-fundamentals/app-and-context.md) - Context API reference
- [Background Work](../03-state-management/background-work.md) - Async task patterns
