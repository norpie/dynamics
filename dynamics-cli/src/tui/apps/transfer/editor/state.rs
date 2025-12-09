use crate::transfer::{TransferConfig, EntityMapping, FieldMapping, Transform};
use crate::tui::resource::Resource;
use crate::tui::widgets::{TreeState, AutocompleteField, TextInputField};
use crate::tui::widgets::events::{TreeEvent, AutocompleteEvent, TextInputEvent};

/// Parameters to initialize the editor
#[derive(Clone, Debug)]
pub struct EditorParams {
    pub config_name: String,
}

impl Default for EditorParams {
    fn default() -> Self {
        Self {
            config_name: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct State {
    pub config_name: String,
    pub config: Resource<TransferConfig>,
    pub tree_state: TreeState,
    pub dirty: bool,

    // Entity lists for autocomplete
    pub source_entities: Resource<Vec<String>>,
    pub target_entities: Resource<Vec<String>>,

    // Entity mapping modal
    pub show_entity_modal: bool,
    pub entity_form: EntityMappingForm,
    pub editing_entity_idx: Option<usize>,

    // Field mapping modal
    pub show_field_modal: bool,
    pub field_form: FieldMappingForm,
    pub editing_field: Option<(usize, usize)>, // (entity_idx, field_idx)

    // Delete confirmation
    pub show_delete_confirm: bool,
    pub delete_target: Option<DeleteTarget>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            config_name: String::new(),
            config: Resource::NotAsked,
            tree_state: TreeState::with_selection(),
            dirty: false,
            source_entities: Resource::NotAsked,
            target_entities: Resource::NotAsked,
            show_entity_modal: false,
            entity_form: EntityMappingForm::default(),
            editing_entity_idx: None,
            show_field_modal: false,
            field_form: FieldMappingForm::default(),
            editing_field: None,
            show_delete_confirm: false,
            delete_target: None,
        }
    }
}

#[derive(Clone)]
pub enum DeleteTarget {
    Entity(usize),
    Field(usize, usize),
}

#[derive(Clone, Default)]
pub struct EntityMappingForm {
    pub source_entity: AutocompleteField,
    pub target_entity: AutocompleteField,
    pub priority: TextInputField,
}

impl EntityMappingForm {
    pub fn is_valid(&self) -> bool {
        !self.source_entity.value.trim().is_empty()
            && !self.target_entity.value.trim().is_empty()
            && self.priority.value.trim().parse::<u32>().is_ok()
    }

    pub fn from_mapping(mapping: &EntityMapping) -> Self {
        let mut form = Self::default();
        form.source_entity.value = mapping.source_entity.clone();
        form.target_entity.value = mapping.target_entity.clone();
        form.priority.value = mapping.priority.to_string();
        form
    }

    pub fn to_mapping(&self) -> EntityMapping {
        EntityMapping {
            id: None,
            source_entity: self.source_entity.value.trim().to_string(),
            target_entity: self.target_entity.value.trim().to_string(),
            priority: self.priority.value.trim().parse().unwrap_or(0),
            field_mappings: vec![],
        }
    }
}

#[derive(Clone, Default)]
pub struct FieldMappingForm {
    pub target_field: TextInputField,
    pub transform_type: TransformType,
    pub source_path: TextInputField,
    pub constant_value: TextInputField,
}

#[derive(Clone, Default, PartialEq)]
pub enum TransformType {
    #[default]
    Copy,
    Constant,
}

impl FieldMappingForm {
    pub fn is_valid(&self) -> bool {
        let target_valid = !self.target_field.value.trim().is_empty();
        let source_valid = match self.transform_type {
            TransformType::Copy => !self.source_path.value.trim().is_empty(),
            TransformType::Constant => true, // constant can be empty (null)
        };
        target_valid && source_valid
    }

    pub fn from_mapping(mapping: &FieldMapping) -> Self {
        let mut form = Self::default();
        form.target_field.value = mapping.target_field.clone();

        match &mapping.transform {
            Transform::Copy { source_path } => {
                form.transform_type = TransformType::Copy;
                form.source_path.value = source_path.to_string();
            }
            Transform::Constant { value } => {
                form.transform_type = TransformType::Constant;
                form.constant_value.value = value.to_string();
            }
            _ => {
                // For complex transforms, show as copy for now
                form.transform_type = TransformType::Copy;
            }
        }
        form
    }

    pub fn to_mapping(&self) -> Option<FieldMapping> {
        use crate::transfer::FieldPath;
        use crate::transfer::Value;

        let target = self.target_field.value.trim().to_string();

        let transform = match self.transform_type {
            TransformType::Copy => {
                let path = FieldPath::parse(&self.source_path.value.trim())
                    .ok()?;
                Transform::Copy { source_path: path }
            }
            TransformType::Constant => {
                let val = self.constant_value.value.trim();
                let value = if val.is_empty() {
                    Value::Null
                } else if let Ok(n) = val.parse::<i64>() {
                    Value::Int(n)
                } else if let Ok(b) = val.parse::<bool>() {
                    Value::Bool(b)
                } else {
                    Value::String(val.to_string())
                };
                Transform::Constant { value }
            }
        };

        Some(FieldMapping {
            id: None,
            target_field: target,
            transform,
        })
    }
}

#[derive(Clone)]
pub enum Msg {
    // Data loading
    ConfigLoaded(Result<TransferConfig, String>),
    SourceEntitiesLoaded(Result<Vec<String>, String>),
    TargetEntitiesLoaded(Result<Vec<String>, String>),

    // Tree navigation
    TreeEvent(TreeEvent),
    TreeSelect(String),

    // Entity mapping actions
    AddEntity,
    EditEntity(usize),
    DeleteEntity(usize),
    CloseEntityModal,
    SaveEntity,
    EntityFormSource(AutocompleteEvent),
    EntityFormTarget(AutocompleteEvent),
    EntityFormPriority(TextInputEvent),

    // Field mapping actions
    AddField(usize), // entity_idx
    EditField(usize, usize), // entity_idx, field_idx
    DeleteField(usize, usize),
    CloseFieldModal,
    SaveField,
    FieldFormTarget(TextInputEvent),
    FieldFormSourcePath(TextInputEvent),
    FieldFormConstant(TextInputEvent),
    FieldFormToggleType,

    // Delete confirmation
    ConfirmDelete,
    CancelDelete,

    // Save
    Save,
    SaveCompleted(Result<(), String>),

    // Navigation
    Back,
    Preview,
}
