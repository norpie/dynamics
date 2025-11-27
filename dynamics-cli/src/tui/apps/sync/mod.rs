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

// Future modules (Phase 4+):
// pub mod app;           // Main app implementation

pub use types::*;
pub use state::State;
pub use msg::Msg;
pub use steps::*;

// Will be added when app.rs is implemented:
// pub use app::EntitySyncApp;
