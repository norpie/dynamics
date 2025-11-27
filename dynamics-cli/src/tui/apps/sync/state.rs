//! State types for the Entity Sync App
//!
//! This module contains the main application state and step-specific
//! state types used throughout the sync wizard flow.

use std::collections::HashSet;
use crate::config::models::DbEnvironment;
use crate::tui::app::AppState;
use crate::tui::resource::Resource;
use crate::tui::widgets::{ListState, TextInputState};

use super::types::{SyncStep, SyncPlan, EntitySyncPlan};

/// Main application state for the Entity Sync App
#[derive(Debug, Default)]
pub struct State {
    /// Current step in the wizard
    pub step: SyncStep,

    /// Step 1: Environment selection state
    pub env_select: EnvironmentSelectState,

    /// Step 2: Entity selection state
    pub entity_select: EntitySelectState,

    /// Step 3: Analysis state (loading)
    pub analysis: AnalysisState,

    /// Step 4: Diff review state
    pub diff_review: DiffReviewState,

    /// Step 5: Confirm state
    pub confirm: ConfirmState,

    /// The complete sync plan (populated after analysis)
    pub sync_plan: Option<SyncPlan>,

    /// Error message to display (if any)
    pub error: Option<String>,

    /// Whether we're currently loading something
    pub is_loading: bool,

    /// Loading message
    pub loading_message: Option<String>,
}

impl AppState for State {
    // Default implementation - no auto-dispatch
}

/// State for Step 1: Environment Selection
#[derive(Debug, Default)]
pub struct EnvironmentSelectState {
    /// Available environments
    pub environments: Resource<Vec<DbEnvironment>>,

    /// Origin environment list state
    pub origin_list: ListState,

    /// Target environment list state
    pub target_list: ListState,

    /// Selected origin environment name
    pub origin_env: Option<String>,

    /// Selected target environment name
    pub target_env: Option<String>,

    /// Which side is focused (true = origin, false = target)
    pub origin_focused: bool,
}

impl EnvironmentSelectState {
    /// Check if we can proceed to the next step
    pub fn can_proceed(&self) -> bool {
        self.origin_env.is_some()
            && self.target_env.is_some()
            && self.origin_env != self.target_env
    }

    /// Get validation error message
    pub fn validation_error(&self) -> Option<&'static str> {
        if self.origin_env.is_none() {
            Some("Select an origin environment")
        } else if self.target_env.is_none() {
            Some("Select a target environment")
        } else if self.origin_env == self.target_env {
            Some("Origin and target must be different")
        } else {
            None
        }
    }
}

/// State for Step 2: Entity Selection
#[derive(Debug, Default)]
pub struct EntitySelectState {
    /// Available entities from the origin environment
    pub available_entities: Resource<Vec<EntityListItem>>,

    /// Entity list state (with multi-select)
    pub entity_list: ListState,

    /// Filter/search text input
    pub filter_input: TextInputState,

    /// Current filter text
    pub filter_text: String,

    /// Selected entity logical names
    pub selected_entities: HashSet<String>,

    /// Junction entity candidates (auto-detected)
    pub junction_candidates: Vec<JunctionCandidate>,

    /// Junction candidates list state
    pub junction_list: ListState,

    /// Which junctions are included
    pub included_junctions: HashSet<String>,

    /// Whether the junction panel is visible
    pub show_junctions: bool,

    /// Which panel is focused (true = entities, false = junctions)
    pub entities_focused: bool,
}

impl EntitySelectState {
    /// Check if we can proceed to the next step
    pub fn can_proceed(&self) -> bool {
        !self.selected_entities.is_empty()
    }

    /// Get all entities to sync (selected + included junctions)
    pub fn entities_to_sync(&self) -> Vec<String> {
        let mut entities: Vec<_> = self.selected_entities.iter().cloned().collect();
        entities.extend(self.included_junctions.iter().cloned());
        entities
    }

    /// Get filtered entity list
    pub fn filtered_entities(&self) -> Vec<&EntityListItem> {
        match &self.available_entities {
            Resource::Success(entities) => {
                if self.filter_text.is_empty() {
                    entities.iter().collect()
                } else {
                    let filter_lower = self.filter_text.to_lowercase();
                    entities
                        .iter()
                        .filter(|e| {
                            e.logical_name.to_lowercase().contains(&filter_lower)
                                || e.display_name
                                    .as_ref()
                                    .map(|d| d.to_lowercase().contains(&filter_lower))
                                    .unwrap_or(false)
                        })
                        .collect()
                }
            }
            _ => Vec::new(),
        }
    }
}

/// Item in the entity list
#[derive(Debug, Clone)]
pub struct EntityListItem {
    pub logical_name: String,
    pub display_name: Option<String>,
    pub record_count: Option<usize>,
}

impl EntityListItem {
    pub fn display_text(&self) -> String {
        if let Some(display) = &self.display_name {
            format!("{} ({})", display, self.logical_name)
        } else {
            self.logical_name.clone()
        }
    }
}

/// Junction entity candidate
#[derive(Debug, Clone)]
pub struct JunctionCandidate {
    pub logical_name: String,
    pub display_name: Option<String>,
    /// The entities this junction connects
    pub connects: Vec<String>,
}

impl JunctionCandidate {
    pub fn display_text(&self) -> String {
        let name = self.display_name.as_ref().unwrap_or(&self.logical_name);
        format!("{} ({})", name, self.connects.join(" ↔ "))
    }
}

/// State for Step 3: Analysis (Loading)
#[derive(Debug, Default)]
pub struct AnalysisState {
    /// Current analysis phase
    pub phase: AnalysisPhase,

    /// Progress percentage (0-100)
    pub progress: u8,

    /// Current status message
    pub status_message: String,

    /// Entities being processed
    pub entities_processing: Vec<String>,

    /// Entity currently being processed
    pub current_entity: Option<String>,
}

/// Analysis phases
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnalysisPhase {
    #[default]
    FetchingOriginSchema,
    FetchingTargetSchema,
    FetchingRecordCounts,
    BuildingDependencyGraph,
    DetectingJunctions,
    ComputingDiff,
    Complete,
}

impl AnalysisPhase {
    pub fn label(&self) -> &'static str {
        match self {
            Self::FetchingOriginSchema => "Fetching origin schema...",
            Self::FetchingTargetSchema => "Fetching target schema...",
            Self::FetchingRecordCounts => "Counting records...",
            Self::BuildingDependencyGraph => "Building dependency graph...",
            Self::DetectingJunctions => "Detecting junction entities...",
            Self::ComputingDiff => "Computing schema diff...",
            Self::Complete => "Analysis complete",
        }
    }
}

/// State for Step 4: Diff Review
#[derive(Debug, Default)]
pub struct DiffReviewState {
    /// List of entity plans
    pub entity_list: ListState,

    /// Currently selected entity index
    pub selected_entity_idx: usize,

    /// Field list for current entity
    pub field_list: ListState,

    /// Active tab (Schema / Data / Lookups)
    pub active_tab: DiffTab,

    /// Expanded sections in the detail view
    pub expanded_sections: HashSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffTab {
    #[default]
    Schema,
    Data,
    Lookups,
}

impl DiffTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Schema => "Schema",
            Self::Data => "Data",
            Self::Lookups => "Lookups",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Schema => Self::Data,
            Self::Data => Self::Lookups,
            Self::Lookups => Self::Schema,
        }
    }
}

impl DiffReviewState {
    /// Get the currently selected entity plan
    pub fn selected_entity<'a>(&self, plan: &'a SyncPlan) -> Option<&'a EntitySyncPlan> {
        plan.entity_plans.get(self.selected_entity_idx)
    }
}

/// State for Step 5: Confirm & Execute
#[derive(Debug, Default)]
pub struct ConfirmState {
    /// Summary list state
    pub summary_list: ListState,

    /// Whether user has confirmed
    pub confirmed: bool,

    /// Export path for Excel report
    pub export_path: Option<String>,

    /// Whether execution has started
    pub executing: bool,

    /// Execution progress (0-100)
    pub execution_progress: u8,

    /// Current execution status
    pub execution_status: String,
}

impl ConfirmState {
    pub fn can_execute(&self) -> bool {
        !self.executing && !self.confirmed
    }
}
