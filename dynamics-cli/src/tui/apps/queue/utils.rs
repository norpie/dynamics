//! Utility functions for queue calculations and formatting

use super::app::State;
use super::models::OperationStatus;
use std::collections::VecDeque;

/// Calculate average completion time from last N successful operations
pub fn calculate_avg_time(recent_times: &VecDeque<u64>, n: usize) -> Option<u64> {
    if recent_times.is_empty() {
        return None;
    }

    let count = recent_times.len().min(n);
    let sum: u64 = recent_times.iter().rev().take(count).sum();
    Some(sum / count as u64)
}

/// Estimate time remaining for pending operations
pub fn estimate_remaining_time(state: &State, n: usize) -> Option<String> {
    let avg_time = calculate_avg_time(&state.recent_completion_times, n)?;
    let pending_count = state
        .queue_items
        .iter()
        .filter(|item| item.status == OperationStatus::Pending)
        .count();

    if pending_count == 0 {
        return None;
    }

    // Account for concurrent execution
    let concurrent = state.max_concurrent.max(1);
    let estimated_ms = (avg_time * pending_count as u64) / concurrent as u64;

    Some(format_duration_estimate(estimated_ms))
}

/// Format duration estimate in a readable way
pub fn format_duration_estimate(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.0}s", ms as f64 / 1000.0)
    } else if ms < 3_600_000 {
        let minutes = ms / 60_000;
        let seconds = (ms % 60_000) / 1000;
        if seconds > 0 {
            format!("{}m{}s", minutes, seconds)
        } else {
            format!("{}m", minutes)
        }
    } else {
        let hours = ms / 3_600_000;
        let minutes = (ms % 3_600_000) / 60_000;
        if minutes > 0 {
            format!("{}h{}m", hours, minutes)
        } else {
            format!("{}h", hours)
        }
    }
}
