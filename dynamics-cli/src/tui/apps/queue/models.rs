//! Data models for the operation queue

use crate::api::operations::{OperationResult, Operations};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Item in the operation queue
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueueItem {
    /// Unique identifier for this queue item
    pub id: String,
    /// The operations to execute
    pub operations: Operations,
    /// Metadata about where this came from
    pub metadata: QueueMetadata,
    /// Current execution status
    pub status: OperationStatus,
    /// Priority (lower number = higher priority)
    pub priority: u8,
    /// Result after execution (if completed)
    pub result: Option<QueueResult>,
    /// When execution started (None if not started yet)
    #[serde(skip)]
    pub started_at: Option<Instant>,
    /// True if this item was Running when the app exited
    #[serde(default)]
    pub was_interrupted: bool,
    /// When the interruption occurred (app exit timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interrupted_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Indices of operations that have already succeeded (for partial retry)
    /// On retry, only operations NOT in this set will be executed
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub succeeded_indices: Vec<usize>,
    /// Cached column data for rendering (to avoid recomputing every frame)
    #[serde(skip)]
    pub cached_columns: Option<Vec<String>>,
}

impl QueueItem {
    /// Create a new queue item with auto-generated ID and Pending status
    pub fn new(operations: Operations, metadata: QueueMetadata, priority: u8) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            operations,
            metadata,
            status: OperationStatus::Pending,
            priority,
            result: None,
            started_at: None,
            was_interrupted: false,
            interrupted_at: None,
            succeeded_indices: Vec::new(),
            cached_columns: None,
        }
    }

    /// Invalidate cached columns (call when item state changes)
    pub fn invalidate_cache(&mut self) {
        self.cached_columns = None;
    }

    /// Update cached columns
    pub fn update_cache(&mut self, columns: Vec<String>) {
        self.cached_columns = Some(columns);
    }

    /// Get the number of operations that still need to be executed
    pub fn pending_operation_count(&self) -> usize {
        self.operations
            .len()
            .saturating_sub(self.succeeded_indices.len())
    }

    /// Check if this item has partial success (some ops succeeded, some failed)
    pub fn is_partially_succeeded(&self) -> bool {
        !self.succeeded_indices.is_empty() && self.succeeded_indices.len() < self.operations.len()
    }
}

/// Metadata about where a queue item came from
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueueMetadata {
    /// Source application/module (e.g., "Deadlines Excel", "Migration")
    pub source: String,
    /// Entity type being operated on (e.g., "cgk_deadline")
    pub entity_type: String,
    /// Human-readable description (e.g., "Row 5: Q1 Report")
    pub description: String,
    /// Source row number (if applicable)
    pub row_number: Option<usize>,
    /// Environment name for client lookup
    pub environment_name: String,
}

/// Status of a queue item
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationStatus {
    /// Waiting to execute
    Pending,
    /// Currently executing
    Running,
    /// User paused this item
    Paused,
    /// Completed successfully
    Done,
    /// Execution failed (all operations failed or fatal error)
    Failed,
    /// Some operations succeeded, some failed (retryable)
    PartiallyFailed,
}

impl OperationStatus {
    /// Get the display symbol for this status
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Pending => "·",
            Self::Running => "▶",
            Self::Paused => "⏸",
            Self::Done => "✓",
            Self::Failed => "⚠",
            Self::PartiallyFailed => "◐",
        }
    }

    /// Get the display word for this status
    pub fn word(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Running => "Running",
            Self::Paused => "Paused",
            Self::Done => "Done",
            Self::Failed => "Failed",
            Self::PartiallyFailed => "Partial",
        }
    }
}

/// Result of executing a queue item
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueueResult {
    /// Whether all operations succeeded
    pub success: bool,
    /// Results from individual operations
    pub operation_results: Vec<OperationResult>,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
}

/// Filter for displaying queue items
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueueFilter {
    /// Show all items
    #[default]
    All,
    /// Show only pending items
    Pending,
    /// Show only running items
    Running,
    /// Show only paused items
    Paused,
    /// Show only failed items
    Failed,
}

impl QueueFilter {
    /// Get display label for this filter
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Pending => "Pending",
            Self::Running => "Running",
            Self::Paused => "Paused",
            Self::Failed => "Failed",
        }
    }

    /// Check if an item matches this filter
    pub fn matches(&self, item: &QueueItem) -> bool {
        match self {
            Self::All => true,
            Self::Pending => item.status == OperationStatus::Pending,
            Self::Running => item.status == OperationStatus::Running,
            Self::Paused => item.status == OperationStatus::Paused,
            Self::Failed => item.status == OperationStatus::Failed,
        }
    }
}

/// Sort mode for queue items
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMode {
    /// Sort by priority (lower first)
    #[default]
    Priority,
    /// Sort by status
    Status,
    /// Sort by source
    Source,
}

impl SortMode {
    /// Get display label for this sort mode
    pub fn label(&self) -> &'static str {
        match self {
            Self::Priority => "Priority",
            Self::Status => "Status",
            Self::Source => "Source",
        }
    }
}
