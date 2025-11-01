# Resource Pattern & Error Handling

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
pub enum Resource<T> {
    NotAsked,
    Loading(Progress),  // Always has Progress
    Success(T),
    Failure {
        error: ResourceError,
        retry_count: usize,
    },
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
    Resource::Failure { error, retry_count } => { /* ... */ }
    Resource::NotAsked => { /* ... */ }
}
```

---

## Error Recovery

V2 provides structured error handling with automatic retry capabilities for async operations.

### Error Types

```rust
pub struct ResourceError {
    pub message: String,
    pub kind: ErrorKind,
    pub retry_strategy: RetryStrategy,
    pub context: HashMap<String, String>,
}

pub enum ErrorKind {
    Transient,        // Temporary network/infrastructure issues
    RateLimit,        // API rate limits, throttling
    Authentication,   // Expired tokens, invalid credentials
    Validation,       // Invalid input, constraint violations
    NotFound,         // Resource doesn't exist
    Permission,       // Insufficient permissions
    Fatal,            // Unrecoverable errors
}

pub enum RetryStrategy {
    None,             // Should not be retried
    Immediate,        // Retry immediately
    After(Duration),  // Retry after specific delay
    Exponential {     // Exponential backoff
        base: Duration,
        max_delay: Duration,
        max_attempts: Option<usize>,
    },
}
```

### Creating Errors

#### Constructor Methods

```rust
// Transient errors (auto-retry with exponential backoff)
ResourceError::transient("Network timeout")

// Rate limit (retry after specific delay)
ResourceError::rate_limit("Too many requests", Duration::from_secs(60))

// Authentication (requires user action, no retry)
ResourceError::authentication("Token expired")

// Validation (requires user to fix input)
ResourceError::validation("Invalid email address")

// Not found (won't help to retry)
ResourceError::not_found("Entity not found")

// Permission (requires permission change)
ResourceError::permission("Access denied")

// Fatal (unrecoverable)
ResourceError::fatal("Disk full")
```

#### Adding Context

```rust
ResourceError::transient("Failed to fetch entity")
    .with_context("entity_id", entity_id)
    .with_context("timestamp", Utc::now().to_string())
```

### Automatic Conversions

#### String Errors

```rust
// Plain strings default to Fatal
state.data = Resource::failure("Something went wrong");  // Fatal
```

#### std::io::Error

Smart mapping based on error kind:

```rust
// NotFound -> ResourceError::not_found
// PermissionDenied -> ResourceError::permission
// ConnectionRefused/Reset/TimedOut -> ResourceError::transient
// Others -> ResourceError::fatal
```

#### reqwest::Error

Smart mapping based on HTTP status:

```rust
// Timeout/connection errors -> Transient
// 401/403 -> Authentication
// 404 -> NotFound
// 429 -> RateLimit
// 500-599 -> Transient
// Others -> Fatal
```

**Example:**
```rust
async fn fetch_data() -> Result<Data, ResourceError> {
    // Automatic conversion with smart error mapping
    let response = reqwest::get("https://api.example.com/data").await?;
    let data = response.json().await?;
    Ok(data)
}

// In app
match fetch_data().await {
    Ok(data) => state.data = Resource::Success(data),
    Err(err) => state.data = Resource::failure(err),  // Auto-mapped
}
```

### Retry Strategies

#### Manual Retry

```rust
fn handle_retry(&mut self, ctx: &mut Context) {
    if let Resource::Failure { .. } = &self.data {
        self.data.increment_retry();

        ctx.spawn_into(&mut self.data, async {
            fetch_data().await
        });
    }
}
```

#### Automatic Retry with Exponential Backoff

Framework helper for automatic retries:

```rust
ctx.spawn_with_retry(&mut self.data, || async {
    fetch_data().await  // Automatically retries with exponential backoff
});
```

**Behavior:**
- Retries transient errors automatically
- Uses exponential backoff (1s, 2s, 4s, 8s, ...)
- Respects max_attempts limit
- Stops retrying for non-retryable errors

#### Rate Limiting with Countdown

```rust
// When rate limited
state.data = Resource::failure(
    ResourceError::rate_limit("Too many requests", Duration::from_secs(60))
);

// In update(), auto-retry after delay
if let Resource::Failure { error, .. } = &self.data {
    if error.kind == ErrorKind::RateLimit {
        if let Some(delay) = error.next_retry_delay(self.data.retry_count()) {
            ctx.timer(delay, Msg::AutoRetry);
        }
    }
}

// Handle auto-retry
Msg::AutoRetry => {
    self.data.increment_retry();
    ctx.spawn_into(&mut self.data, async { fetch_data().await });
}
```

### Error UI Rendering

#### Comprehensive Error Display

```rust
match &self.data {
    Resource::Failure { error, retry_count } => {
        ui.col(|ui| {
            ui.text(format!("Error: {}", error.message))
                .style(theme.error);

            // Show context details
            if !error.context.is_empty() {
                ui.text("Details:").style(theme.text_dim);
                for (key, value) in &error.context {
                    ui.text(format!("  {}: {}", key, value))
                        .style(theme.text_muted);
                }
            }

            // Retry information
            if error.is_retryable() {
                ui.text(format!("Retry attempt: {}", retry_count));

                if let Some(delay) = error.next_retry_delay(*retry_count) {
                    if delay > Duration::ZERO {
                        ui.text(format!("Next retry in {:.1}s", delay.as_secs_f32()));
                    }
                    ui.button("Retry Now").on_click(Self::handle_retry);
                } else {
                    ui.text("Max retry attempts exceeded");
                }
            } else {
                // Not retryable - show appropriate action
                match error.kind {
                    ErrorKind::Authentication => {
                        ui.button("Re-authenticate").on_click(Self::handle_reauth);
                    }
                    ErrorKind::Validation => {
                        ui.text("Please fix the input and try again");
                    }
                    _ => {
                        ui.text("This error cannot be automatically retried");
                    }
                }
            }
        });
    }
    // ... other states
}
```

#### Simple Retry Button

```rust
match &self.data {
    Resource::Failure { error, .. } if error.is_retryable() => {
        ui.row(|ui| {
            ui.text(error.message).style(theme.error);
            ui.button("Retry").on_click(Self::handle_retry);
        });
    }
    Resource::Failure { error, .. } => {
        ui.text(format!("Error: {}", error.message))
            .style(theme.error);
    }
    // ... other states
}
```

### Error Kind Guidelines

**When to use each error kind:**

- **Transient** - Network timeouts, temporary service unavailability, connection issues
- **RateLimit** - API rate limits, throttling (include retry_after duration)
- **Authentication** - Expired tokens, invalid credentials, missing auth
- **Validation** - Invalid user input, malformed requests, constraint violations
- **NotFound** - Resource doesn't exist, 404 errors
- **Permission** - Insufficient permissions, 403 errors, access denied
- **Fatal** - Unrecoverable errors, disk full, invalid state, programming errors

### Helper Methods

```rust
impl<T> Resource<T> {
    /// Check if resource failed
    pub fn is_failure(&self) -> bool;

    /// Get error if failed
    pub fn error(&self) -> Option<&ResourceError>;

    /// Get retry count
    pub fn retry_count(&self) -> usize;

    /// Check if retryable
    pub fn is_retryable(&self) -> bool;

    /// Get next retry delay
    pub fn next_retry_delay(&self) -> Option<Duration>;

    /// Increment retry count (for manual retry)
    pub fn increment_retry(&mut self);
}

impl ResourceError {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool;

    /// Calculate next retry delay based on retry count
    pub fn next_retry_delay(&self, retry_count: usize) -> Option<Duration>;

    /// Add context to error
    pub fn with_context(self, key: impl Into<String>, value: impl Into<String>) -> Self;
}
```

---

## Migration from V1

**V1:**
```rust
enum Resource<T, E = String> {
    NotAsked,
    Loading,
    Success(T),
    Failure(E),
}

// Set error
state.data = Resource::Failure("Network timeout".to_string());
```

**V2:**
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

// Set error - automatic conversion from String (defaults to Fatal)
state.data = Resource::failure("Network timeout");

// Or be explicit about error kind
state.data = Resource::failure(ResourceError::transient("Network timeout"));

// reqwest errors auto-convert with smart mapping
state.data = Resource::failure(reqwest_error);

// Progress required for Loading
state.data = Resource::loading();  // Helper method (Indeterminate)
// Or explicit:
state.data = Resource::Loading(Progress::Indeterminate);
```

---

## See Also

- [Background Work](background-work.md) - Background task patterns
- [Pub/Sub](../07-advanced/events-and-queues.md) - Event-driven communication

---

**Next:** Learn about [Multi-View Routing](routing.md) or [Event Broadcasting](../07-advanced/events-and-queues.md).
