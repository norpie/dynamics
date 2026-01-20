use crossterm::event::KeyCode;

use crate::config::repository::transfer::TransferConfigSummary;
use crate::transfer::TransferMode;
use crate::tui::resource::Resource;
use crate::tui::widgets::events::{AutocompleteEvent, ListEvent, TextInputEvent};
use crate::tui::widgets::{AutocompleteField, ListState, TextInputField};

#[derive(Default)]
pub struct State {
    pub configs: Resource<Vec<TransferConfigSummary>>,
    pub list_state: ListState,

    // Delete confirmation
    pub show_delete_confirm: bool,
    pub selected_for_delete: Option<String>,

    // Create modal
    pub show_create_modal: bool,
    pub create_form: CreateConfigForm,
    pub environments: Resource<Vec<String>>,

    // Clone modal
    pub show_clone_modal: bool,
    pub clone_form: CloneConfigForm,
    pub selected_for_clone: Option<String>,

    // Merge modal
    pub show_merge_modal: bool,
    pub merge_form: MergeConfigForm,
    pub merge_error: Option<String>,
}

#[derive(Clone)]
pub struct CreateConfigForm {
    pub name: TextInputField,
    pub source_env: AutocompleteField,
    pub target_env: AutocompleteField,
    pub mode: TransferMode,
}

impl Default for CreateConfigForm {
    fn default() -> Self {
        Self {
            name: TextInputField::default(),
            source_env: AutocompleteField::default(),
            target_env: AutocompleteField::default(),
            mode: TransferMode::Declarative,
        }
    }
}

impl CreateConfigForm {
    pub fn is_valid(&self) -> bool {
        !self.name.value.trim().is_empty()
            && !self.source_env.value.trim().is_empty()
            && !self.target_env.value.trim().is_empty()
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            TransferMode::Declarative => TransferMode::Lua,
            TransferMode::Lua => TransferMode::Declarative,
        };
    }
}

#[derive(Clone, Default)]
pub struct CloneConfigForm {
    pub name: TextInputField,
}

impl CloneConfigForm {
    pub fn is_valid(&self) -> bool {
        !self.name.value.trim().is_empty()
    }
}

#[derive(Clone, Default)]
pub struct MergeConfigForm {
    pub name: TextInputField,
}

impl MergeConfigForm {
    pub fn is_valid(&self) -> bool {
        !self.name.value.trim().is_empty()
    }
}

#[derive(Clone)]
pub enum Msg {
    // Data loading
    ConfigsLoaded(Result<Vec<TransferConfigSummary>, String>),
    EnvironmentsLoaded(Result<Vec<String>, String>),

    // List navigation
    ListNavigate(KeyCode),
    SelectConfig(usize),

    // Actions
    CreateNew,
    EditSelected,
    DeleteSelected,
    ConfirmDelete,
    CancelDelete,
    Refresh,

    // Delete result
    DeleteResult(Result<(), String>),

    // Create modal
    CloseCreateModal,
    CreateFormName(TextInputEvent),
    CreateFormSourceEnv(AutocompleteEvent),
    CreateFormTargetEnv(AutocompleteEvent),
    CreateFormToggleMode,
    SaveNewConfig,
    ConfigCreated(Result<(String, TransferMode), String>),

    // Clone modal
    CloneSelected,
    CloseCloneModal,
    CloneFormName(TextInputEvent),
    SaveClone,
    CloneResult(Result<(String, TransferMode), String>),

    // Multi-select
    ListMultiSelect(ListEvent),

    // Merge modal
    MergeSelected,
    CloseMergeModal,
    MergeFormName(TextInputEvent),
    SaveMerge,
    MergeResult(Result<String, String>),
    CloseErrorModal,
}
