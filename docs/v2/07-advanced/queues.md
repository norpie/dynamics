# Work Queue System

**Prerequisites:** [App and Context](../01-fundamentals/app-and-context.md)

## Overview

The work queue system provides **exactly-once delivery** with persistence, priorities, and crash recovery. Work items are guaranteed to be processed by a single consumer.

**Use for:** API operations, batch jobs, background tasks requiring guaranteed delivery
**Don't use for:** Notifications to multiple apps (use [Event Broadcast](events.md) instead)

---

## Queue Registration

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

---

## Sending Work to Queues

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

---

## Processing Queue Items

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

---

## WorkQueue API

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

---

## Queue Persistence

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

---

## Queue System Characteristics

| Property | Behavior |
|----------|----------|
| **Delivery** | Exactly-once, single consumer (queue owner) |
| **Ordering** | FIFO within priority, priority-ordered globally |
| **Retry** | App-defined (queue owner decides) |
| **Persistence** | Always (SQLite backed) |
| **Use cases** | API operations, batch jobs, background tasks |

---

## Type Safety Architecture

Work queues use **type erasure at runtime boundaries** to avoid generic explosion while providing type safety to apps:

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

**Result:** Apps work with typed `WorkQueue<Operation>`, while runtime stores everything as `serde_json::Value`. No generic explosion.

---

## Example: Operation Queue

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

---

## Usage Guidelines

| Scenario | System | Priority |
|----------|--------|----------|
| Notify multiple apps of state change | [Event broadcast](events.md) | N/A |
| Send work item to specific app | Work queue | Normal |
| Critical API operations that can't be lost | Work queue | Critical |
| Background cleanup tasks | Work queue | Background |
| User triggered UI event | [Event broadcast](events.md) | N/A |
| Theme/settings changed | [Event broadcast](events.md) | Persistent if needed |
| Batch operations (Excel import) | Work queue | Normal/Low |
| File watcher notifications | [Event broadcast](events.md) | N/A |
| Migration progress updates | [Event broadcast](events.md) | N/A |

---

## See Also

- [Event Broadcast](events.md) - Fire-and-forget pub/sub
- [App and Context](../01-fundamentals/app-and-context.md) - Context API reference
- [Background Work](../03-state-management/background-work.md) - Async task patterns

---

**Next:** Explore [Event Broadcast](events.md) or [Background Work](../03-state-management/background-work.md).
