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
use std::collections::HashMap;
use once_cell::sync::Lazy;

/// Global progress state for analysis (allows async task to update UI)
static ANALYSIS_PROGRESS: Lazy<RwLock<AnalysisProgressState>> = Lazy::new(|| {
    RwLock::new(AnalysisProgressState::default())
});

/// Status of a single fetch operation
#[derive(Debug, Clone, PartialEq)]
pub enum FetchStatus {
    Pending,
    Fetching,
    Done,
    Failed(String),
}

impl FetchStatus {
    pub fn symbol(&self) -> &'static str {
        match self {
            FetchStatus::Pending => "○",
            FetchStatus::Fetching => "⋯",
            FetchStatus::Done => "✓",
            FetchStatus::Failed(_) => "✗",
        }
    }
}

/// Progress for a single entity
#[derive(Debug, Clone)]
pub struct EntityProgress {
    pub entity: String,
    pub display_name: Option<String>,
    pub schema_status: FetchStatus,
    pub records_status: FetchStatus,
    pub record_count: Option<usize>,
}

impl EntityProgress {
    pub fn new(entity: &str, display_name: Option<&str>) -> Self {
        Self {
            entity: entity.to_string(),
            display_name: display_name.map(|s| s.to_string()),
            schema_status: FetchStatus::Pending,
            records_status: FetchStatus::Pending,
            record_count: None,
        }
    }
}

/// Progress state updated during analysis
#[derive(Debug, Clone)]
pub struct AnalysisProgressState {
    /// Per-entity progress
    pub entities: HashMap<String, EntityProgress>,
    /// Order of entities (for consistent display)
    pub entity_order: Vec<String>,
    /// Overall phase message
    pub overall_phase: String,
    /// Whether analysis is complete
    pub complete: bool,
}

impl Default for AnalysisProgressState {
    fn default() -> Self {
        Self {
            entities: HashMap::new(),
            entity_order: Vec::new(),
            overall_phase: String::new(),
            complete: false,
        }
    }
}

/// Initialize progress tracking for a set of entities
pub fn init_analysis_progress(entities: &[(String, Option<String>)]) {
    if let Ok(mut progress) = ANALYSIS_PROGRESS.write() {
        progress.entities.clear();
        progress.entity_order.clear();
        progress.overall_phase = "Starting analysis...".to_string();
        progress.complete = false;

        for (entity, display_name) in entities {
            progress.entities.insert(
                entity.clone(),
                EntityProgress::new(entity, display_name.as_deref()),
            );
            progress.entity_order.push(entity.clone());
        }
    }
}

/// Update schema fetch status for an entity
pub fn set_entity_schema_status(entity: &str, status: FetchStatus) {
    if let Ok(mut progress) = ANALYSIS_PROGRESS.write() {
        if let Some(ep) = progress.entities.get_mut(entity) {
            ep.schema_status = status;
        }
    }
}

/// Update records fetch status for an entity
pub fn set_entity_records_status(entity: &str, status: FetchStatus, count: Option<usize>) {
    if let Ok(mut progress) = ANALYSIS_PROGRESS.write() {
        if let Some(ep) = progress.entities.get_mut(entity) {
            ep.records_status = status;
            if let Some(c) = count {
                ep.record_count = Some(c);
            }
        }
    }
}

/// Set overall phase message
pub fn set_analysis_phase(phase: &str) {
    if let Ok(mut progress) = ANALYSIS_PROGRESS.write() {
        progress.overall_phase = phase.to_string();
    }
}

/// Mark analysis as complete
pub fn set_analysis_complete() {
    if let Ok(mut progress) = ANALYSIS_PROGRESS.write() {
        progress.complete = true;
        progress.overall_phase = "Analysis complete".to_string();
    }
}

/// Get the current analysis progress (called from UI render)
pub fn get_analysis_progress() -> AnalysisProgressState {
    ANALYSIS_PROGRESS
        .read()
        .map(|p| p.clone())
        .unwrap_or_default()
}
