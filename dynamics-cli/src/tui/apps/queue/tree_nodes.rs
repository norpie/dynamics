//! Tree node implementations for the operation queue

use crate::api::operations::Operation;
use crate::tui::{Element, Theme};
use crate::tui::widgets::{TreeItem, TableTreeItem};
use ratatui::layout::Constraint;
use super::models::QueueItem;

/// Message type for queue app
pub type Msg = super::app::Msg;

/// Node for table tree display
#[derive(Clone)]
pub enum QueueTreeNode {
    /// Parent node representing a queue item
    Parent(QueueItem),
    /// Child node representing an individual operation
    Child {
        operation: Operation,
        parent_id: String,
        index: usize,
    },
}

impl TreeItem for QueueTreeNode {
    type Msg = Msg;

    fn id(&self) -> String {
        match self {
            Self::Parent(item) => item.id.clone(),
            Self::Child { parent_id, index, .. } => format!("{}_{}", parent_id, index),
        }
    }

    fn has_children(&self) -> bool {
        match self {
            Self::Parent(item) => item.operations.len() > 1,
            Self::Child { .. } => false,
        }
    }

    fn children(&self) -> Vec<Self> {
        match self {
            Self::Parent(item) => {
                // Convert operations to child nodes
                // First operation is shown in the parent, rest are children
                item.operations
                    .operations()
                    .iter()
                    .skip(1) // Skip first operation (shown in parent)
                    .enumerate()
                    .map(|(idx, op)| Self::Child {
                        operation: op.clone(),
                        parent_id: item.id.clone(),
                        index: idx + 1, // +1 because we skipped first
                    })
                    .collect()
            }
            Self::Child { .. } => vec![],
        }
    }

    fn to_element(
        &self,
        _depth: usize,
        _is_selected: bool,
        _is_expanded: bool,
    ) -> Element<Self::Msg> {
        // Not used for table trees
        Element::None
    }
}

impl TableTreeItem for QueueTreeNode {
    fn to_table_columns(
        &self,
        _depth: usize,
        _is_selected: bool,
        _is_expanded: bool,
    ) -> Vec<String> {
        match self {
            Self::Parent(item) => {
                let status_word = item.status.word();

                // Get first operation for display
                let first_op = item.operations.operations().first();
                let op_entity = first_op.map(|op| op.entity()).unwrap_or("unknown");

                // Determine type: single operation or batch
                let op_type = if item.operations.len() == 1 {
                    first_op.map(|op| op.operation_type().to_string()).unwrap_or_else(|| "Unknown".to_string())
                } else {
                    "BATCH".to_string()
                };

                // Calculate duration/time
                let time_display = if let Some(result) = &item.result {
                    // Completed - show duration from result
                    format_duration(result.duration_ms)
                } else if let Some(started) = item.started_at {
                    // Running - show elapsed time
                    let elapsed = started.elapsed().as_millis() as u64;
                    format_duration(elapsed)
                } else {
                    // Not started
                    "-".to_string()
                };

                // Add warning indicator if interrupted
                let description = if item.was_interrupted {
                    format!("{} ({}) ⚠", item.metadata.description, op_entity)
                } else {
                    format!("{} ({})", item.metadata.description, op_entity)
                };

                vec![
                    item.priority.to_string(),
                    status_word.to_string(),
                    description,
                    op_type,
                    time_display,
                ]
            }
            Self::Child { operation, .. } => {
                let op_type = operation.operation_type();
                let entity = operation.entity();

                vec![
                    "".to_string(),           // No priority for children
                    "".to_string(),           // No status for children
                    format!("└─ {}", entity), // Indented entity name
                    op_type.to_string(),
                    "".to_string(),           // No time for children
                ]
            }
        }
    }

    fn column_widths() -> Vec<Constraint> {
        vec![
            Constraint::Length(4),  // Priority
            Constraint::Length(8),  // Status word
            Constraint::Fill(1),    // Operation description (expandable)
            Constraint::Length(10), // Type
            Constraint::Length(10), // Time/Duration
        ]
    }

    fn column_headers() -> Vec<String> {
        vec![
            "Pri".to_string(),
            "Status".to_string(),
            "Operation".to_string(),
            "Type".to_string(),
            "Time".to_string(),
        ]
    }
}

/// Format duration in milliseconds to human-readable format
fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else if ms < 3_600_000 {
        let minutes = ms / 60_000;
        let seconds = (ms % 60_000) / 1000;
        format!("{}m{}s", minutes, seconds)
    } else {
        let hours = ms / 3_600_000;
        let minutes = (ms % 3_600_000) / 60_000;
        format!("{}h{}m", hours, minutes)
    }
}
