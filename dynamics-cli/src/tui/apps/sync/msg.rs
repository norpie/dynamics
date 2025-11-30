//! Message types for the Entity Sync App
//!
//! This module defines all the messages (events/actions) that can occur
//! in the sync wizard flow.

use std::collections::HashSet;
use crossterm::event::KeyCode;
use crate::api::models::Environment as ApiEnvironment;
use crate::tui::widgets::TextInputEvent;

use super::state::{EntityListItem, JunctionCandidate, AnalysisPhase, ExecutionPhase};
use super::types::SyncPlan;
use crate::tui::apps::queue::models::{QueueResult, QueueMetadata};

/// All messages for the Entity Sync App
#[derive(Clone)]
pub enum Msg {
    // === Navigation ===
    /// Go back to previous step (or quit on first step)
    Back,
    /// Proceed to next step
    Next,
    /// Confirm back action (when there are unsaved changes)
    ConfirmBack,
    /// Cancel back action
    CancelBack,

    // === Step 1: Environment Selection ===
    /// Environments loaded from database
    EnvironmentsLoaded(Result<Vec<ApiEnvironment>, String>),
    /// Navigate in origin environment list
    OriginListNavigate(KeyCode),
    /// Navigate in target environment list
    TargetListNavigate(KeyCode),
    /// Select origin environment at index
    OriginListSelect(usize),
    /// Select target environment at index
    TargetListSelect(usize),
    /// Switch focus between origin and target lists
    SwitchEnvFocus,
    /// Origin list clicked at index
    OriginListClicked(usize),
    /// Target list clicked at index
    TargetListClicked(usize),

    // === Step 2: Entity Selection ===
    /// Entities loaded from origin environment
    EntitiesLoaded(Result<Vec<EntityListItem>, String>),
    /// Navigate in entity list
    EntityListNavigate(KeyCode),
    /// Toggle selection of entity at index
    EntityListToggle(usize),
    /// Select all visible entities
    SelectAllEntities,
    /// Deselect all entities
    DeselectAllEntities,
    /// Filter text input event
    FilterInputEvent(TextInputEvent),
    /// Clear filter
    ClearFilter,
    /// Junction candidates detected
    JunctionCandidatesLoaded(Vec<JunctionCandidate>),
    /// Navigate in junction list
    JunctionListNavigate(KeyCode),
    /// Toggle inclusion of junction at index
    JunctionListToggle(usize),
    /// Toggle visibility of junction panel
    ToggleJunctionPanel,
    /// Switch focus between entity and junction lists
    SwitchEntityFocus,
    /// Include all junction candidates
    IncludeAllJunctions,
    /// Exclude all junction candidates
    ExcludeAllJunctions,
    /// Preset selector event (open/close/navigate/select)
    PresetSelectEvent(crate::tui::widgets::SelectEvent),

    // === Step 3: Analysis ===
    /// Start the analysis process
    StartAnalysis,
    /// Analysis phase changed
    AnalysisPhaseChanged(AnalysisPhase),
    /// Analysis progress update
    AnalysisProgress(u8, String),
    /// Analysis completed successfully
    AnalysisComplete(Box<SyncPlan>),
    /// Analysis failed
    AnalysisFailed(String),
    /// Cancel analysis
    CancelAnalysis,

    // === Step 4: Diff Review ===
    /// Navigate in entity list
    DiffEntityListNavigate(KeyCode),
    /// Select entity at index
    DiffEntityListSelect(usize),
    /// Navigate in field list
    DiffFieldListNavigate(KeyCode),
    /// Navigate in origin data records list
    DataListNavigate(KeyCode),
    /// Navigate in target data records list
    TargetDataListNavigate(KeyCode),
    /// Switch to next tab (Schema/Data/Lookups)
    DiffNextTab,
    /// Switch to previous tab
    DiffPrevTab,
    /// Toggle section expansion
    DiffToggleSection(String),
    /// Set viewport height for field list
    DiffSetViewportHeight(usize),

    // === Step 5: Confirm ===
    /// Toggle confirmation checkbox
    ToggleConfirm,
    /// Start execution (send to queue)
    Execute,
    /// Export report to Excel
    ExportReport,
    /// Report exported successfully
    ReportExported(Result<String, String>),
    /// Queue item completed (from subscription)
    QueueItemCompleted {
        id: String,
        result: QueueResult,
        metadata: QueueMetadata,
    },
    /// Execution phase changed
    ExecutionPhaseChanged(ExecutionPhase),
    /// Execution completed
    ExecutionComplete(Result<(), String>),

    // === General ===
    /// Dismiss error message
    DismissError,
    /// No-op message (for ignored events)
    Noop,
}

// Implement Debug manually since some fields can't derive it
impl std::fmt::Debug for Msg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Back => write!(f, "Back"),
            Self::Next => write!(f, "Next"),
            Self::ConfirmBack => write!(f, "ConfirmBack"),
            Self::CancelBack => write!(f, "CancelBack"),
            Self::EnvironmentsLoaded(r) => write!(f, "EnvironmentsLoaded({:?})", r.is_ok()),
            Self::OriginListNavigate(k) => write!(f, "OriginListNavigate({:?})", k),
            Self::TargetListNavigate(k) => write!(f, "TargetListNavigate({:?})", k),
            Self::OriginListSelect(i) => write!(f, "OriginListSelect({})", i),
            Self::TargetListSelect(i) => write!(f, "TargetListSelect({})", i),
            Self::SwitchEnvFocus => write!(f, "SwitchEnvFocus"),
            Self::OriginListClicked(i) => write!(f, "OriginListClicked({})", i),
            Self::TargetListClicked(i) => write!(f, "TargetListClicked({})", i),
            Self::EntitiesLoaded(r) => write!(f, "EntitiesLoaded({:?})", r.is_ok()),
            Self::EntityListNavigate(k) => write!(f, "EntityListNavigate({:?})", k),
            Self::EntityListToggle(i) => write!(f, "EntityListToggle({})", i),
            Self::SelectAllEntities => write!(f, "SelectAllEntities"),
            Self::DeselectAllEntities => write!(f, "DeselectAllEntities"),
            Self::FilterInputEvent(_) => write!(f, "FilterInputEvent"),
            Self::ClearFilter => write!(f, "ClearFilter"),
            Self::JunctionCandidatesLoaded(j) => write!(f, "JunctionCandidatesLoaded({})", j.len()),
            Self::JunctionListNavigate(k) => write!(f, "JunctionListNavigate({:?})", k),
            Self::JunctionListToggle(i) => write!(f, "JunctionListToggle({})", i),
            Self::ToggleJunctionPanel => write!(f, "ToggleJunctionPanel"),
            Self::SwitchEntityFocus => write!(f, "SwitchEntityFocus"),
            Self::IncludeAllJunctions => write!(f, "IncludeAllJunctions"),
            Self::ExcludeAllJunctions => write!(f, "ExcludeAllJunctions"),
            Self::PresetSelectEvent(e) => write!(f, "PresetSelectEvent({:?})", e),
            Self::StartAnalysis => write!(f, "StartAnalysis"),
            Self::AnalysisPhaseChanged(p) => write!(f, "AnalysisPhaseChanged({:?})", p),
            Self::AnalysisProgress(p, s) => write!(f, "AnalysisProgress({}, {})", p, s),
            Self::AnalysisComplete(_) => write!(f, "AnalysisComplete"),
            Self::AnalysisFailed(e) => write!(f, "AnalysisFailed({})", e),
            Self::CancelAnalysis => write!(f, "CancelAnalysis"),
            Self::DiffEntityListNavigate(k) => write!(f, "DiffEntityListNavigate({:?})", k),
            Self::DiffEntityListSelect(i) => write!(f, "DiffEntityListSelect({})", i),
            Self::DiffFieldListNavigate(k) => write!(f, "DiffFieldListNavigate({:?})", k),
            Self::DataListNavigate(k) => write!(f, "DataListNavigate({:?})", k),
            Self::TargetDataListNavigate(k) => write!(f, "TargetDataListNavigate({:?})", k),
            Self::DiffNextTab => write!(f, "DiffNextTab"),
            Self::DiffPrevTab => write!(f, "DiffPrevTab"),
            Self::DiffToggleSection(s) => write!(f, "DiffToggleSection({})", s),
            Self::DiffSetViewportHeight(h) => write!(f, "DiffSetViewportHeight({})", h),
            Self::ToggleConfirm => write!(f, "ToggleConfirm"),
            Self::Execute => write!(f, "Execute"),
            Self::ExportReport => write!(f, "ExportReport"),
            Self::ReportExported(r) => write!(f, "ReportExported({:?})", r.is_ok()),
            Self::QueueItemCompleted { id, result, .. } => {
                write!(f, "QueueItemCompleted({}, success={})", id, result.success)
            }
            Self::ExecutionPhaseChanged(p) => write!(f, "ExecutionPhaseChanged({:?})", p),
            Self::ExecutionComplete(r) => write!(f, "ExecutionComplete({:?})", r.is_ok()),
            Self::DismissError => write!(f, "DismissError"),
            Self::Noop => write!(f, "Noop"),
        }
    }
}
