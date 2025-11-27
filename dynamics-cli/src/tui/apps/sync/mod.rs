//! Entity Sync App
//!
//! TUI application for syncing "settings" entities between Dynamics 365
//! environments (e.g., dev → pre-prod). Provides a wizard-style flow:
//!
//! 1. Environment Selection - Choose origin and target environments
//! 2. Entity Selection - Multi-select entities to sync
//! 3. Analysis - Fetch schemas, build dependency graph, detect junctions
//! 4. Diff Review - Review schema diff and data preview
//! 5. Confirm & Execute - Generate operations and send to queue
//!
//! Key features:
//! - Preserves GUIDs for relationship integrity
//! - Auto-detects junction entities
//! - Dependency-aware ordering (delete reverse, insert forward)
//! - Additive schema sync (no deletions - report for manual review)
//! - Excel report for manual follow-up tasks

pub mod types;
pub mod state;
pub mod msg;
pub mod logic;
pub mod steps;
pub mod app;

pub use types::*;
pub use state::State;
pub use msg::Msg;
pub use steps::*;
pub use app::EntitySyncApp;

use std::sync::RwLock;

/// Global progress state for analysis (allows async task to update UI)
static ANALYSIS_PROGRESS: RwLock<AnalysisProgressState> = RwLock::new(AnalysisProgressState::new());

/// Progress state updated during analysis
#[derive(Debug, Clone)]
pub struct AnalysisProgressState {
    pub message: String,
    pub entity: Option<String>,
    pub step: Option<String>,
}

impl AnalysisProgressState {
    const fn new() -> Self {
        Self {
            message: String::new(),
            entity: None,
            step: None,
        }
    }
}

/// Update the global analysis progress (called from async task)
pub fn set_analysis_progress(message: &str, entity: Option<&str>, step: Option<&str>) {
    if let Ok(mut progress) = ANALYSIS_PROGRESS.write() {
        progress.message = message.to_string();
        progress.entity = entity.map(|s| s.to_string());
        progress.step = step.map(|s| s.to_string());
    }
}

/// Get the current analysis progress (called from UI render)
pub fn get_analysis_progress() -> AnalysisProgressState {
    ANALYSIS_PROGRESS
        .read()
        .map(|p| p.clone())
        .unwrap_or_else(|_| AnalysisProgressState::new())
}
