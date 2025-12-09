use crate::transfer::{TransferConfig, EntityMapping, FieldMapping, Transform};
use crate::tui::resource::Resource;
use crate::tui::widgets::{TreeState, AutocompleteField, TextInputField};
use crate::tui::widgets::events::{TreeEvent, AutocompleteEvent, TextInputEvent};
use crate::api::FieldMetadata;

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

    // Field metadata for autocomplete (loaded when opening field modal)
    pub source_fields: Resource<Vec<FieldMetadata>>,
    pub target_fields: Resource<Vec<FieldMetadata>>,
    pub current_field_entity_idx: Option<usize>, // Which entity mapping the fields are for

    // Pending field modal open (entity_idx, field_idx) - None field_idx means "add new"
    pub pending_field_modal: Option<(usize, Option<usize>)>,

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
            source_entities: Resource::NotAsked,
            target_entities: Resource::NotAsked,
            show_entity_modal: false,
            entity_form: EntityMappingForm::default(),
            editing_entity_idx: None,
            show_field_modal: false,
            field_form: FieldMappingForm::default(),
            editing_field: None,
            source_fields: Resource::NotAsked,
            target_fields: Resource::NotAsked,
            current_field_entity_idx: None,
            pending_field_modal: None,
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
    pub target_field: AutocompleteField,
    pub transform_type: TransformType,

    // Copy transform fields
    pub source_path: AutocompleteField,

    // Constant transform fields
    pub constant_value: TextInputField,

    // Conditional transform fields
    pub condition_source: AutocompleteField,
    pub condition_type: ConditionType,
    pub condition_value: TextInputField,
    pub then_value: TextInputField,
    pub else_value: TextInputField,

    // ValueMap transform fields
    pub value_map_source: AutocompleteField,
    pub value_map_fallback: FallbackType,
    pub value_map_default: TextInputField,
    pub value_map_entries: Vec<ValueMapEntry>,
    pub value_map_selected: Option<usize>,
}

#[derive(Clone, Default)]
pub struct ValueMapEntry {
    pub source_value: TextInputField,
    pub target_value: TextInputField,
}

#[derive(Clone, Default, PartialEq, Copy)]
pub enum TransformType {
    #[default]
    Copy,
    Constant,
    Conditional,
    ValueMap,
}

impl TransformType {
    pub fn next(&self) -> Self {
        match self {
            TransformType::Copy => TransformType::Constant,
            TransformType::Constant => TransformType::Conditional,
            TransformType::Conditional => TransformType::ValueMap,
            TransformType::ValueMap => TransformType::Copy,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            TransformType::Copy => "Copy",
            TransformType::Constant => "Constant",
            TransformType::Conditional => "Conditional",
            TransformType::ValueMap => "Value Map",
        }
    }
}

#[derive(Clone, Default, PartialEq, Copy)]
pub enum ConditionType {
    #[default]
    Equals,
    NotEquals,
    IsNull,
    IsNotNull,
}

impl ConditionType {
    pub fn next(&self) -> Self {
        match self {
            ConditionType::Equals => ConditionType::NotEquals,
            ConditionType::NotEquals => ConditionType::IsNull,
            ConditionType::IsNull => ConditionType::IsNotNull,
            ConditionType::IsNotNull => ConditionType::Equals,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ConditionType::Equals => "equals",
            ConditionType::NotEquals => "not equals",
            ConditionType::IsNull => "is null",
            ConditionType::IsNotNull => "is not null",
        }
    }

    pub fn needs_value(&self) -> bool {
        matches!(self, ConditionType::Equals | ConditionType::NotEquals)
    }
}

#[derive(Clone, Default, PartialEq, Copy)]
pub enum FallbackType {
    #[default]
    Error,
    Default,
    PassThrough,
    Null,
}

impl FallbackType {
    pub fn next(&self) -> Self {
        match self {
            FallbackType::Error => FallbackType::Default,
            FallbackType::Default => FallbackType::PassThrough,
            FallbackType::PassThrough => FallbackType::Null,
            FallbackType::Null => FallbackType::Error,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            FallbackType::Error => "Error",
            FallbackType::Default => "Default",
            FallbackType::PassThrough => "Pass Through",
            FallbackType::Null => "Null",
        }
    }
}

impl FieldMappingForm {
    pub fn is_valid(&self) -> bool {
        let target_valid = !self.target_field.value.trim().is_empty();
        let transform_valid = match self.transform_type {
            TransformType::Copy => !self.source_path.value.trim().is_empty(),
            TransformType::Constant => true, // constant can be empty (null)
            TransformType::Conditional => {
                !self.condition_source.value.trim().is_empty()
                    && (!self.condition_type.needs_value() || !self.condition_value.value.trim().is_empty())
            }
            TransformType::ValueMap => {
                !self.value_map_source.value.trim().is_empty()
                    && !self.value_map_entries.is_empty()
            }
        };
        target_valid && transform_valid
    }

    pub fn from_mapping(mapping: &FieldMapping) -> Self {
        use crate::transfer::{Condition, Fallback};

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
            Transform::Conditional { source_path, condition, then_value, else_value } => {
                form.transform_type = TransformType::Conditional;
                form.condition_source.value = source_path.to_string();
                match condition {
                    Condition::Equals { value } => {
                        form.condition_type = ConditionType::Equals;
                        form.condition_value.value = value.to_string();
                    }
                    Condition::NotEquals { value } => {
                        form.condition_type = ConditionType::NotEquals;
                        form.condition_value.value = value.to_string();
                    }
                    Condition::IsNull => {
                        form.condition_type = ConditionType::IsNull;
                    }
                    Condition::IsNotNull => {
                        form.condition_type = ConditionType::IsNotNull;
                    }
                }
                form.then_value.value = then_value.to_string();
                form.else_value.value = else_value.to_string();
            }
            Transform::ValueMap { source_path, mappings, fallback } => {
                form.transform_type = TransformType::ValueMap;
                form.value_map_source.value = source_path.to_string();
                match fallback {
                    Fallback::Error => form.value_map_fallback = FallbackType::Error,
                    Fallback::Default { value } => {
                        form.value_map_fallback = FallbackType::Default;
                        form.value_map_default.value = value.to_string();
                    }
                    Fallback::PassThrough => form.value_map_fallback = FallbackType::PassThrough,
                    Fallback::Null => form.value_map_fallback = FallbackType::Null,
                }
                form.value_map_entries = mappings.iter().map(|(src, tgt)| {
                    ValueMapEntry {
                        source_value: TextInputField { value: src.to_string(), ..Default::default() },
                        target_value: TextInputField { value: tgt.to_string(), ..Default::default() },
                    }
                }).collect();
            }
        }
        form
    }

    pub fn to_mapping(&self) -> Option<FieldMapping> {
        use crate::transfer::{FieldPath, Value, Condition, Fallback};

        let target = self.target_field.value.trim().to_string();

        let transform = match self.transform_type {
            TransformType::Copy => {
                let path = FieldPath::parse(self.source_path.value.trim()).ok()?;
                Transform::Copy { source_path: path }
            }
            TransformType::Constant => {
                let value = parse_value(self.constant_value.value.trim());
                Transform::Constant { value }
            }
            TransformType::Conditional => {
                let source_path = FieldPath::parse(self.condition_source.value.trim()).ok()?;
                let condition = match self.condition_type {
                    ConditionType::Equals => Condition::Equals {
                        value: parse_value(self.condition_value.value.trim()),
                    },
                    ConditionType::NotEquals => Condition::NotEquals {
                        value: parse_value(self.condition_value.value.trim()),
                    },
                    ConditionType::IsNull => Condition::IsNull,
                    ConditionType::IsNotNull => Condition::IsNotNull,
                };
                let then_value = parse_value(self.then_value.value.trim());
                let else_value = parse_value(self.else_value.value.trim());
                Transform::Conditional { source_path, condition, then_value, else_value }
            }
            TransformType::ValueMap => {
                let source_path = FieldPath::parse(self.value_map_source.value.trim()).ok()?;
                let mappings: Vec<(Value, Value)> = self.value_map_entries.iter()
                    .map(|e| (
                        parse_value(e.source_value.value.trim()),
                        parse_value(e.target_value.value.trim()),
                    ))
                    .collect();
                let fallback = match self.value_map_fallback {
                    FallbackType::Error => Fallback::Error,
                    FallbackType::Default => Fallback::Default {
                        value: parse_value(self.value_map_default.value.trim()),
                    },
                    FallbackType::PassThrough => Fallback::PassThrough,
                    FallbackType::Null => Fallback::Null,
                };
                Transform::ValueMap { source_path, mappings, fallback }
            }
        };

        Some(FieldMapping {
            id: None,
            target_field: target,
            transform,
        })
    }

    pub fn add_value_map_entry(&mut self) {
        self.value_map_entries.push(ValueMapEntry::default());
        self.value_map_selected = Some(self.value_map_entries.len() - 1);
    }

    pub fn remove_value_map_entry(&mut self, idx: usize) {
        if idx < self.value_map_entries.len() {
            self.value_map_entries.remove(idx);
            if self.value_map_entries.is_empty() {
                self.value_map_selected = None;
            } else if let Some(selected) = self.value_map_selected {
                if selected >= self.value_map_entries.len() {
                    self.value_map_selected = Some(self.value_map_entries.len() - 1);
                }
            }
        }
    }
}

fn parse_value(s: &str) -> crate::transfer::Value {
    use crate::transfer::Value;
    if s.is_empty() {
        Value::Null
    } else if let Ok(n) = s.parse::<i64>() {
        Value::Int(n)
    } else if let Ok(f) = s.parse::<f64>() {
        Value::Float(f)
    } else if let Ok(b) = s.parse::<bool>() {
        Value::Bool(b)
    } else {
        Value::String(s.to_string())
    }
}

#[derive(Clone)]
pub enum Msg {
    // Data loading
    ConfigLoaded(Result<TransferConfig, String>),
    SourceEntitiesLoaded(Result<Vec<String>, String>),
    TargetEntitiesLoaded(Result<Vec<String>, String>),
    SourceFieldsLoaded(Result<Vec<FieldMetadata>, String>),
    TargetFieldsLoaded(Result<Vec<FieldMetadata>, String>),

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
    FieldFormTarget(AutocompleteEvent),
    FieldFormSourcePath(AutocompleteEvent),
    FieldFormConstant(TextInputEvent),
    FieldFormToggleType,

    // Conditional transform fields
    FieldFormConditionSource(AutocompleteEvent),
    FieldFormToggleConditionType,
    FieldFormConditionValue(TextInputEvent),
    FieldFormThenValue(TextInputEvent),
    FieldFormElseValue(TextInputEvent),

    // ValueMap transform fields
    FieldFormValueMapSource(AutocompleteEvent),
    FieldFormToggleFallback,
    FieldFormValueMapDefault(TextInputEvent),
    FieldFormAddMapping,
    FieldFormRemoveMapping(usize),
    FieldFormMappingSource(usize, TextInputEvent),
    FieldFormMappingTarget(usize, TextInputEvent),

    // Delete confirmation
    ConfirmDelete,
    CancelDelete,

    // Auto-save result
    SaveCompleted(Result<(), String>),

    // Navigation
    Back,
    Preview,
}
