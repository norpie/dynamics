use std::collections::HashMap;

use crate::transfer::{TransferConfig, EntityMapping, FieldMapping, OperationFilter, Transform, Replacement, Resolver, ResolverFallback, SourceFilter, FieldPath, Condition};
use crate::tui::resource::Resource;
use crate::tui::widgets::{TreeState, AutocompleteField, TextInputField, ScrollableState, ListState};
use crate::tui::widgets::events::{TreeEvent, AutocompleteEvent, TextInputEvent, ListEvent};
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

    // Related entity fields cache - keyed by lookup field name (e.g., "parentaccountid")
    pub related_fields: HashMap<String, Resource<Vec<FieldMetadata>>>,

    // Resolver modal
    pub show_resolver_modal: bool,
    pub resolver_form: ResolverForm,
    /// Which entity mapping this resolver belongs to
    pub editing_resolver_for_entity: Option<usize>,
    /// Index within that entity's resolvers (None = adding new)
    pub editing_resolver_idx: Option<usize>,
    pub resolver_match_fields: Resource<Vec<FieldMetadata>>,
    /// Tracks which entity the resolver_match_fields were loaded for
    pub resolver_match_fields_entity: Option<String>,
    /// Source fields for resolver source_path autocomplete
    pub resolver_source_fields: Resource<Vec<FieldMetadata>>,
    /// Related fields for nested lookup traversal in resolver source_path
    pub resolver_related_fields: HashMap<String, Resource<Vec<FieldMetadata>>>,

    // Delete confirmation
    pub show_delete_confirm: bool,
    pub delete_target: Option<DeleteTarget>,

    // Quick field picker modal
    pub show_quick_fields_modal: bool,
    pub quick_fields_available: Vec<FieldMetadata>,
    pub quick_fields_list_state: ListState,
    pub quick_fields_entity_idx: Option<usize>,
    pub pending_quick_fields: bool,

    // Per-entity field metadata cache (for showing types in tree)
    // Key: entity_idx, Value: target entity's field metadata
    pub entity_target_fields_cache: HashMap<usize, Vec<FieldMetadata>>,
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
            related_fields: HashMap::new(),
            show_resolver_modal: false,
            resolver_form: ResolverForm::default(),
            editing_resolver_for_entity: None,
            editing_resolver_idx: None,
            resolver_match_fields: Resource::NotAsked,
            resolver_match_fields_entity: None,
            resolver_source_fields: Resource::NotAsked,
            resolver_related_fields: HashMap::new(),
            show_delete_confirm: false,
            delete_target: None,
            show_quick_fields_modal: false,
            quick_fields_available: Vec::new(),
            quick_fields_list_state: ListState::with_selection(),
            quick_fields_entity_idx: None,
            pending_quick_fields: false,
            entity_target_fields_cache: HashMap::new(),
        }
    }
}

impl State {
    /// Compute fields available for quick-add
    /// Returns fields that exist in both source and target (same logical_name),
    /// excluding already-mapped fields and system fields
    pub fn compute_quick_fields(&self, entity_idx: usize) -> Vec<FieldMetadata> {
        use std::collections::HashSet;
        use crate::tui::apps::sync::types::is_system_field;

        let source_fields = match &self.source_fields {
            Resource::Success(fields) => fields,
            _ => return Vec::new(),
        };

        let target_fields = match &self.target_fields {
            Resource::Success(fields) => fields,
            _ => return Vec::new(),
        };

        let config = match &self.config {
            Resource::Success(config) => config,
            _ => return Vec::new(),
        };

        let entity_mapping = match config.entity_mappings.get(entity_idx) {
            Some(m) => m,
            None => return Vec::new(),
        };

        // Get already-mapped target fields
        let already_mapped: HashSet<&str> = entity_mapping
            .field_mappings
            .iter()
            .map(|fm| fm.target_field.as_str())
            .collect();

        // Build set of target field names for intersection check
        let target_field_names: HashSet<&str> = target_fields
            .iter()
            .map(|f| f.logical_name.as_str())
            .collect();

        // Filter source fields: must exist in target, not already mapped, not system
        let mut available: Vec<FieldMetadata> = source_fields
            .iter()
            .filter(|f| target_field_names.contains(f.logical_name.as_str()))
            .filter(|f| !already_mapped.contains(f.logical_name.as_str()))
            .filter(|f| !is_system_field(&f.logical_name))
            .cloned()
            .collect();

        // Sort by logical name
        available.sort_by(|a, b| a.logical_name.cmp(&b.logical_name));

        available
    }
}

#[derive(Clone)]
pub enum DeleteTarget {
    Entity(usize),
    Field(usize, usize),
    /// (entity_idx, resolver_idx)
    Resolver(usize, usize),
}

#[derive(Clone)]
pub struct EntityMappingForm {
    pub source_entity: AutocompleteField,
    pub target_entity: AutocompleteField,
    pub priority: TextInputField,
    /// Operation filter - which operations to allow
    pub allow_creates: bool,
    pub allow_updates: bool,
    pub allow_deletes: bool,
    pub allow_deactivates: bool,
    /// Source record filter
    pub filter_enabled: bool,
    pub filter_field: AutocompleteField,
    pub filter_condition_type: ConditionType,
    pub filter_value: TextInputField,
}

impl Default for EntityMappingForm {
    fn default() -> Self {
        Self {
            source_entity: AutocompleteField::default(),
            target_entity: AutocompleteField::default(),
            priority: TextInputField::default(),
            allow_creates: true,
            allow_updates: true,
            allow_deletes: false,
            allow_deactivates: false,
            filter_enabled: false,
            filter_field: AutocompleteField::default(),
            filter_condition_type: ConditionType::default(),
            filter_value: TextInputField::default(),
        }
    }
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
        form.allow_creates = mapping.operation_filter.creates;
        form.allow_updates = mapping.operation_filter.updates;
        form.allow_deletes = mapping.operation_filter.deletes;
        form.allow_deactivates = mapping.operation_filter.deactivates;

        // Load source filter if present
        if let Some(filter) = &mapping.source_filter {
            form.filter_enabled = true;
            form.filter_field.value = filter.field_path.to_string();
            form.filter_condition_type = ConditionType::from_condition(&filter.condition);
            form.filter_value.value = match &filter.condition {
                Condition::Equals { value } | Condition::NotEquals { value } => {
                    value.to_string()
                }
                Condition::IsNull | Condition::IsNotNull => String::new(),
            };
        }

        form
    }

    pub fn to_mapping(&self) -> EntityMapping {
        // Build source filter if enabled
        let source_filter = if self.filter_enabled && !self.filter_field.value.trim().is_empty() {
            let field_path = FieldPath::parse(self.filter_field.value.trim())
                .unwrap_or_else(|_| FieldPath::simple(self.filter_field.value.trim()));
            let condition = self.filter_condition_type.to_condition(&self.filter_value.value);
            Some(SourceFilter::new(field_path, condition))
        } else {
            None
        };

        EntityMapping {
            id: None,
            source_entity: self.source_entity.value.trim().to_string(),
            target_entity: self.target_entity.value.trim().to_string(),
            priority: self.priority.value.trim().parse().unwrap_or(0),
            operation_filter: OperationFilter {
                creates: self.allow_creates,
                updates: self.allow_updates,
                deletes: self.allow_deletes,
                deactivates: self.allow_deactivates,
            },
            source_filter,
            resolvers: vec![],
            field_mappings: vec![],
        }
    }
}

/// A single match field row in the resolver form
#[derive(Clone, Default)]
pub struct MatchFieldRow {
    /// Source path - where to get value from source record (e.g., cgk_userid.cgk_email)
    pub source_path: AutocompleteField,
    /// Field to match against in target entity
    pub target_field: AutocompleteField,
}

impl MatchFieldRow {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_values(source_path: &str, target_field: &str) -> Self {
        let mut row = Self::new();
        row.source_path.value = source_path.to_string();
        row.target_field.value = target_field.to_string();
        row
    }

    /// For backwards compatibility - sets target_field only
    pub fn with_target_field(target_field: &str) -> Self {
        let mut row = Self::new();
        row.target_field.value = target_field.to_string();
        row
    }

    pub fn is_valid(&self) -> bool {
        !self.target_field.value.trim().is_empty()
    }

    /// Check if source_path is required (for compound keys) and valid
    pub fn is_valid_compound(&self) -> bool {
        !self.target_field.value.trim().is_empty() && !self.source_path.value.trim().is_empty()
    }
}

#[derive(Clone)]
pub struct ResolverForm {
    pub name: TextInputField,
    pub source_entity: AutocompleteField,
    /// Match field rows (supports compound keys)
    pub match_field_rows: Vec<MatchFieldRow>,
    /// Currently focused row index
    pub focused_row: usize,
    pub fallback: ResolverFallback,
    pub default_guid: TextInputField, // For Default fallback
}

impl Default for ResolverForm {
    fn default() -> Self {
        Self {
            name: TextInputField::default(),
            source_entity: AutocompleteField::default(),
            match_field_rows: vec![MatchFieldRow::new()], // Start with one empty row
            focused_row: 0,
            fallback: ResolverFallback::default(),
            default_guid: TextInputField::default(),
        }
    }
}

impl ResolverForm {
    pub fn is_valid(&self) -> bool {
        let rows_valid = if self.match_field_rows.len() == 1 {
            // Single field - source_path optional
            self.match_field_rows.iter().all(|row| row.is_valid())
        } else {
            // Compound key - source_path required
            self.match_field_rows.iter().all(|row| row.is_valid_compound())
        };

        let base_valid = !self.name.value.trim().is_empty()
            && !self.source_entity.value.trim().is_empty()
            && !self.match_field_rows.is_empty()
            && rows_valid;

        // If using Default fallback, also validate the GUID
        if self.fallback.is_default() || !self.default_guid.value.trim().is_empty() {
            base_valid && uuid::Uuid::parse_str(self.default_guid.value.trim()).is_ok()
        } else {
            base_valid
        }
    }

    pub fn from_resolver(resolver: &Resolver) -> Self {
        let mut form = Self::default();
        form.name.value = resolver.name.clone();
        form.source_entity.value = resolver.source_entity.clone();

        // Load match fields with both source_path and target_field
        form.match_field_rows = resolver
            .match_fields
            .iter()
            .map(|mf| MatchFieldRow::with_values(&mf.source_path.to_string(), &mf.target_field))
            .collect();

        // Ensure at least one row
        if form.match_field_rows.is_empty() {
            form.match_field_rows.push(MatchFieldRow::new());
        }

        form.fallback = resolver.fallback.clone();
        if let Some(guid) = resolver.fallback.default_guid() {
            form.default_guid.value = guid.to_string();
        }
        form
    }

    pub fn to_resolver(&self) -> Resolver {
        use crate::transfer::MatchField;

        let fallback = if !self.default_guid.value.trim().is_empty() {
            if let Ok(guid) = uuid::Uuid::parse_str(self.default_guid.value.trim()) {
                ResolverFallback::Default(guid)
            } else {
                self.fallback.clone()
            }
        } else {
            self.fallback.clone()
        };

        // Build match fields from rows
        let is_compound = self.match_field_rows.len() > 1;
        let match_fields: Vec<MatchField> = self
            .match_field_rows
            .iter()
            .filter(|row| row.is_valid())
            .map(|row| {
                let target_field = row.target_field.value.trim();
                let source_path = row.source_path.value.trim();
                // For single-field resolver, use target_field as source_path if not specified
                let effective_source = if source_path.is_empty() && !is_compound {
                    target_field
                } else {
                    source_path
                };
                MatchField::from_paths(effective_source, target_field).unwrap_or_else(|_| MatchField::simple(target_field))
            })
            .collect();

        Resolver::with_match_fields(
            self.name.value.trim(),
            self.source_entity.value.trim(),
            match_fields,
            fallback,
        )
    }

    /// Add a new empty match field row
    pub fn add_row(&mut self) {
        self.match_field_rows.push(MatchFieldRow::new());
        self.focused_row = self.match_field_rows.len() - 1;
    }

    /// Remove the currently focused row (if more than one exists)
    pub fn remove_current_row(&mut self) {
        if self.match_field_rows.len() > 1 {
            self.match_field_rows.remove(self.focused_row);
            if self.focused_row >= self.match_field_rows.len() {
                self.focused_row = self.match_field_rows.len() - 1;
            }
        }
    }

    /// Get current row mutably
    pub fn current_row_mut(&mut self) -> Option<&mut MatchFieldRow> {
        self.match_field_rows.get_mut(self.focused_row)
    }
}

#[derive(Clone, Default)]
pub struct FieldMappingForm {
    pub target_field: AutocompleteField,
    pub transform_type: TransformType,

    // Copy transform fields
    pub source_path: AutocompleteField,
    /// Resolver name for Copy transform (None = direct copy)
    pub resolver_name: Option<String>,

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
    pub value_map_scroll: ScrollableState,

    // Format transform fields
    pub format_template: TextInputField,
    pub format_null_handling: NullHandlingType,

    // Replace transform fields
    pub replace_source: AutocompleteField,
    pub replace_entries: Vec<ReplaceEntry>,
}

#[derive(Clone, Default)]
pub struct ValueMapEntry {
    pub source_value: TextInputField,
    pub target_value: TextInputField,
}

#[derive(Clone, Default)]
pub struct ReplaceEntry {
    pub pattern: TextInputField,
    pub replacement: TextInputField,
    pub is_regex: bool,
}

#[derive(Clone, Default, PartialEq, Copy)]
pub enum TransformType {
    #[default]
    Copy,
    Constant,
    Conditional,
    ValueMap,
    Format,
    Replace,
}

impl TransformType {
    pub fn next(&self) -> Self {
        match self {
            TransformType::Copy => TransformType::Constant,
            TransformType::Constant => TransformType::Conditional,
            TransformType::Conditional => TransformType::ValueMap,
            TransformType::ValueMap => TransformType::Format,
            TransformType::Format => TransformType::Replace,
            TransformType::Replace => TransformType::Copy,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            TransformType::Copy => "Copy",
            TransformType::Constant => "Constant",
            TransformType::Conditional => "Conditional",
            TransformType::ValueMap => "Value Map",
            TransformType::Format => "Format",
            TransformType::Replace => "Replace",
        }
    }
}

#[derive(Clone, Default, PartialEq, Copy)]
pub enum NullHandlingType {
    #[default]
    Error,
    Empty,
    Zero,
}

impl NullHandlingType {
    pub fn next(&self) -> Self {
        match self {
            NullHandlingType::Error => NullHandlingType::Empty,
            NullHandlingType::Empty => NullHandlingType::Zero,
            NullHandlingType::Zero => NullHandlingType::Error,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            NullHandlingType::Error => "Error",
            NullHandlingType::Empty => "Empty",
            NullHandlingType::Zero => "Zero",
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

    /// Convert from a Condition enum value
    pub fn from_condition(condition: &Condition) -> Self {
        match condition {
            Condition::Equals { .. } => ConditionType::Equals,
            Condition::NotEquals { .. } => ConditionType::NotEquals,
            Condition::IsNull => ConditionType::IsNull,
            Condition::IsNotNull => ConditionType::IsNotNull,
        }
    }

    /// Convert to a Condition enum value
    pub fn to_condition(&self, value_str: &str) -> Condition {
        match self {
            ConditionType::Equals => Condition::Equals {
                value: parse_value(value_str),
            },
            ConditionType::NotEquals => Condition::NotEquals {
                value: parse_value(value_str),
            },
            ConditionType::IsNull => Condition::IsNull,
            ConditionType::IsNotNull => Condition::IsNotNull,
        }
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
    /// Basic validation - checks that required fields are filled
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
            TransformType::Format => !self.format_template.value.trim().is_empty(),
            TransformType::Replace => {
                !self.replace_source.value.trim().is_empty()
                    && !self.replace_entries.is_empty()
                    && self.replace_entries.iter().all(|e| !e.pattern.value.trim().is_empty())
            }
        };
        target_valid && transform_valid
    }

    /// Full validation with type checking
    /// Returns (can_save, warnings, errors)
    pub fn validate(
        &self,
        target_fields: &[crate::api::FieldMetadata],
        source_fields: &[crate::api::FieldMetadata],
    ) -> FormValidation {
        use super::validation::{ValidationResult, validate_constant_value, validate_copy_types, validate_optionset_copy};

        let mut validation = FormValidation::default();

        // Find target field metadata
        let target_meta = target_fields
            .iter()
            .find(|f| f.logical_name == self.target_field.value.trim());

        if self.target_field.value.trim().is_empty() {
            validation.target_error = Some("Target field is required".into());
            return validation;
        }

        // If target field not found in metadata, warn but allow
        let Some(target) = target_meta else {
            validation.target_warning = Some("Target field not found in schema".into());
            return validation;
        };

        match self.transform_type {
            TransformType::Copy => {
                let source_path = self.source_path.value.trim();
                if source_path.is_empty() {
                    validation.source_error = Some("Source field is required".into());
                    return validation;
                }

                // Get the base field name (before any dot for lookups)
                let base_field = source_path.split('.').next().unwrap_or(source_path);
                if let Some(source) = source_fields.iter().find(|f| f.logical_name == base_field) {
                    let result = validate_copy_types(&source.field_type, &target.field_type);
                    match result {
                        ValidationResult::Valid => {}
                        ValidationResult::Warning(msg) => validation.transform_warning = Some(msg),
                        ValidationResult::Error(msg) => validation.transform_error = Some(msg),
                    }

                    // Check for OptionSet copy warning (only if no existing warning/error)
                    if validation.transform_warning.is_none() && validation.transform_error.is_none() {
                        let optionset_result = validate_optionset_copy(&source.field_type, &target.field_type);
                        if let ValidationResult::Warning(msg) = optionset_result {
                            validation.transform_warning = Some(msg);
                        }
                    }
                } else {
                    validation.source_warning = Some("Source field not found in schema".into());
                }
            }

            TransformType::Constant => {
                let value = self.constant_value.value.trim();
                let result = validate_constant_value(value, &target.field_type, target.is_required);
                match result {
                    ValidationResult::Valid => {}
                    ValidationResult::Warning(msg) => validation.value_warning = Some(msg),
                    ValidationResult::Error(msg) => validation.value_error = Some(msg),
                }
            }

            TransformType::Conditional => {
                let source_path = self.condition_source.value.trim();
                if source_path.is_empty() {
                    validation.source_error = Some("Source field is required".into());
                    return validation;
                }

                // Validate then_value
                let then_result = validate_constant_value(
                    self.then_value.value.trim(),
                    &target.field_type,
                    false, // then/else can be null
                );
                match then_result {
                    ValidationResult::Valid => {}
                    ValidationResult::Warning(msg) => validation.then_warning = Some(msg),
                    ValidationResult::Error(msg) => validation.then_error = Some(msg),
                }

                // Validate else_value
                let else_result = validate_constant_value(
                    self.else_value.value.trim(),
                    &target.field_type,
                    false,
                );
                match else_result {
                    ValidationResult::Valid => {}
                    ValidationResult::Warning(msg) => validation.else_warning = Some(msg),
                    ValidationResult::Error(msg) => validation.else_error = Some(msg),
                }
            }

            TransformType::ValueMap => {
                let source_path = self.value_map_source.value.trim();
                if source_path.is_empty() {
                    validation.source_error = Some("Source field is required".into());
                    return validation;
                }

                if self.value_map_entries.is_empty() {
                    validation.transform_error = Some("At least one mapping is required".into());
                    return validation;
                }

                // Validate each mapping's target value
                for (i, entry) in self.value_map_entries.iter().enumerate() {
                    let result = validate_constant_value(
                        entry.target_value.value.trim(),
                        &target.field_type,
                        false,
                    );
                    if let ValidationResult::Error(msg) = result {
                        validation.mapping_errors.push((i, msg));
                    }
                }

                // Validate default value if fallback is Default
                if self.value_map_fallback == FallbackType::Default {
                    let result = validate_constant_value(
                        self.value_map_default.value.trim(),
                        &target.field_type,
                        false,
                    );
                    match result {
                        ValidationResult::Valid => {}
                        ValidationResult::Warning(msg) => validation.default_warning = Some(msg),
                        ValidationResult::Error(msg) => validation.default_error = Some(msg),
                    }
                }
            }

            TransformType::Format => {
                let template = self.format_template.value.trim();
                if template.is_empty() {
                    validation.transform_error = Some("Template is required".into());
                    return validation;
                }

                // Try to parse the template to validate syntax
                if let Err(e) = crate::transfer::transform::format::parse_template(template) {
                    validation.transform_error = Some(format!("Invalid template: {}", e));
                }
            }

            TransformType::Replace => {
                let source_path = self.replace_source.value.trim();
                if source_path.is_empty() {
                    validation.source_error = Some("Source field is required".into());
                    return validation;
                }

                if self.replace_entries.is_empty() {
                    validation.transform_error = Some("At least one replacement is required".into());
                    return validation;
                }

                // Check that all patterns are non-empty
                for (i, entry) in self.replace_entries.iter().enumerate() {
                    if entry.pattern.value.trim().is_empty() {
                        validation.transform_error = Some(format!("Pattern {} is empty", i + 1));
                        return validation;
                    }
                }
            }
        }

        validation
    }
}

/// Validation results for the field mapping form
#[derive(Clone, Default)]
pub struct FormValidation {
    pub target_error: Option<String>,
    pub target_warning: Option<String>,
    pub source_error: Option<String>,
    pub source_warning: Option<String>,
    pub value_error: Option<String>,
    pub value_warning: Option<String>,
    pub transform_error: Option<String>,
    pub transform_warning: Option<String>,
    pub then_error: Option<String>,
    pub then_warning: Option<String>,
    pub else_error: Option<String>,
    pub else_warning: Option<String>,
    pub default_error: Option<String>,
    pub default_warning: Option<String>,
    pub mapping_errors: Vec<(usize, String)>,
}

impl FormValidation {
    pub fn has_errors(&self) -> bool {
        self.target_error.is_some()
            || self.source_error.is_some()
            || self.value_error.is_some()
            || self.transform_error.is_some()
            || self.then_error.is_some()
            || self.else_error.is_some()
            || self.default_error.is_some()
            || !self.mapping_errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        self.target_warning.is_some()
            || self.source_warning.is_some()
            || self.value_warning.is_some()
            || self.transform_warning.is_some()
            || self.then_warning.is_some()
            || self.else_warning.is_some()
            || self.default_warning.is_some()
    }

    /// Get the first error message, if any
    pub fn first_error(&self) -> Option<&str> {
        self.target_error.as_deref()
            .or(self.source_error.as_deref())
            .or(self.value_error.as_deref())
            .or(self.transform_error.as_deref())
            .or(self.then_error.as_deref())
            .or(self.else_error.as_deref())
            .or(self.default_error.as_deref())
            .or(self.mapping_errors.first().map(|(_, msg)| msg.as_str()))
    }

    /// Get the first warning message, if any
    pub fn first_warning(&self) -> Option<&str> {
        self.target_warning.as_deref()
            .or(self.source_warning.as_deref())
            .or(self.value_warning.as_deref())
            .or(self.transform_warning.as_deref())
            .or(self.then_warning.as_deref())
            .or(self.else_warning.as_deref())
            .or(self.default_warning.as_deref())
    }
}

impl FieldMappingForm {
    pub fn from_mapping(mapping: &FieldMapping) -> Self {
        use crate::transfer::{Condition, Fallback};

        let mut form = Self::default();
        form.target_field.value = mapping.target_field.clone();

        match &mapping.transform {
            Transform::Copy { source_path, resolver } => {
                form.transform_type = TransformType::Copy;
                form.source_path.value = source_path.to_string();
                form.resolver_name = resolver.clone();
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
            Transform::Format { template, null_handling } => {
                use crate::transfer::transform::format::NullHandling;
                form.transform_type = TransformType::Format;
                form.format_template.value = template.to_string();
                form.format_null_handling = match null_handling {
                    NullHandling::Error => NullHandlingType::Error,
                    NullHandling::Empty => NullHandlingType::Empty,
                    NullHandling::Zero => NullHandlingType::Zero,
                };
            }
            Transform::Replace { source_path, replacements } => {
                form.transform_type = TransformType::Replace;
                form.replace_source.value = source_path.to_string();
                form.replace_entries = replacements.iter().map(|r| {
                    ReplaceEntry {
                        pattern: TextInputField { value: r.pattern.clone(), ..Default::default() },
                        replacement: TextInputField { value: r.replacement.clone(), ..Default::default() },
                        is_regex: r.is_regex,
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
                Transform::Copy {
                    source_path: path,
                    resolver: self.resolver_name.clone(),
                }
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
            TransformType::Format => {
                use crate::transfer::transform::format::NullHandling;
                let template = crate::transfer::transform::format::parse_template(
                    self.format_template.value.trim()
                ).ok()?;
                let null_handling = match self.format_null_handling {
                    NullHandlingType::Error => NullHandling::Error,
                    NullHandlingType::Empty => NullHandling::Empty,
                    NullHandlingType::Zero => NullHandling::Zero,
                };
                Transform::Format { template, null_handling }
            }
            TransformType::Replace => {
                let source_path = FieldPath::parse(self.replace_source.value.trim()).ok()?;
                let replacements: Vec<Replacement> = self.replace_entries.iter()
                    .filter(|e| !e.pattern.value.trim().is_empty())
                    .map(|e| Replacement::new(
                        e.pattern.value.trim(),
                        e.replacement.value.trim(),
                        e.is_regex,
                    ))
                    .collect();
                Transform::Replace { source_path, replacements }
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

    pub fn add_replace_entry(&mut self) {
        self.replace_entries.push(ReplaceEntry::default());
    }

    pub fn remove_replace_entry(&mut self, idx: usize) {
        if idx < self.replace_entries.len() {
            self.replace_entries.remove(idx);
        }
    }

    /// Prefill value map entries from source field option values
    /// Called when switching to ValueMap transform for an OptionSet field
    /// Only prefills if entries are currently empty to avoid overwriting user work
    pub fn prefill_valuemap_from_optionset(&mut self, source_field: &crate::api::FieldMetadata) {
        // Only prefill if we have option values and entries are currently empty
        if !source_field.option_values.is_empty() && self.value_map_entries.is_empty() {
            self.value_map_entries = source_field.option_values
                .iter()
                .map(|opt| {
                    // Format source value with label for clarity (e.g., "1 (Active)")
                    let source_display = opt.label.as_ref()
                        .map(|l| format!("{} ({})", opt.value, l))
                        .unwrap_or_else(|| opt.value.to_string());

                    ValueMapEntry {
                        source_value: TextInputField {
                            value: opt.value.to_string(), // Just the integer value
                            ..Default::default()
                        },
                        target_value: TextInputField::default(), // Leave blank for user
                    }
                })
                .collect();

            // Select first entry if any were added
            if !self.value_map_entries.is_empty() {
                self.value_map_selected = Some(0);
            }
        }
    }

    /// Check if the source field is an OptionSet type
    pub fn is_optionset_source(&self, source_fields: &[crate::api::FieldMetadata]) -> bool {
        use crate::api::metadata::FieldType;

        let source_path = if self.transform_type == TransformType::ValueMap {
            &self.value_map_source.value
        } else {
            &self.source_path.value
        };

        let base_field = source_path.trim().split('.').next().unwrap_or("");
        source_fields.iter()
            .find(|f| f.logical_name == base_field)
            .map(|f| matches!(f.field_type, FieldType::OptionSet | FieldType::MultiSelectOptionSet))
            .unwrap_or(false)
    }
}

fn parse_value(s: &str) -> crate::transfer::Value {
    use crate::transfer::Value;
    if s.is_empty() || s.eq_ignore_ascii_case("null") {
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
    EntityFormToggleCreates,
    EntityFormToggleUpdates,
    EntityFormToggleDeletes,
    EntityFormToggleDeactivates,

    // Entity source filter
    EntityFormToggleFilter,
    EntityFormFilterField(AutocompleteEvent),
    EntityFormToggleFilterCondition,
    EntityFormFilterValue(TextInputEvent),

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
    FieldFormCycleResolver, // Cycle through available resolvers for Copy transform

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
    /// Cycle through source option values (idx, backwards)
    FieldFormCycleSourceOption(usize, bool),
    /// Cycle through target option values (idx, backwards)
    FieldFormCycleTargetOption(usize, bool),
    /// Scroll value map entries
    FieldFormValueMapScroll(crossterm::event::KeyCode),
    /// Set value map scroll dimensions (viewport_height, content_height, viewport_width, content_width)
    FieldFormValueMapScrollDimensions(usize, usize, usize, usize),

    // Format transform fields
    FieldFormFormatTemplate(TextInputEvent),
    FieldFormToggleNullHandling,

    // Replace transform fields
    FieldFormReplaceSource(AutocompleteEvent),
    FieldFormAddReplace,
    FieldFormRemoveReplace(usize),
    FieldFormReplacePattern(usize, TextInputEvent),
    FieldFormReplaceReplacement(usize, TextInputEvent),
    FieldFormToggleReplaceRegex(usize),

    // Resolver modal actions
    /// AddResolver(entity_idx)
    AddResolver(usize),
    /// EditResolver(entity_idx, resolver_idx)
    EditResolver(usize, usize),
    /// DeleteResolver(entity_idx, resolver_idx)
    DeleteResolver(usize, usize),
    CloseResolverModal,
    SaveResolver,
    ResolverFormName(TextInputEvent),
    ResolverFormSourceEntity(AutocompleteEvent),
    ResolverFormCycleFallback,
    ResolverFormDefaultGuid(TextInputEvent),
    ResolverMatchFieldsLoaded(Result<Vec<FieldMetadata>, String>),
    /// Source fields for resolver source_path autocomplete
    ResolverSourceFieldsLoaded(Result<Vec<FieldMetadata>, String>),
    // Match field row operations
    ResolverAddMatchFieldRow,
    ResolverRemoveMatchFieldRow,
    ResolverMatchField(usize, AutocompleteEvent),
    ResolverSourcePath(usize, AutocompleteEvent),
    /// Related fields loaded for resolver source_path nested lookup
    ResolverRelatedFieldsLoaded {
        lookup_field: String,
        result: Result<Vec<FieldMetadata>, String>,
    },

    // Delete confirmation
    ConfirmDelete,
    CancelDelete,

    // Auto-save result
    SaveCompleted(Result<(), String>),

    // Related entity fields loading (for nested lookup autocomplete)
    RelatedFieldsLoaded {
        lookup_field: String,
        result: Result<Vec<FieldMetadata>, String>,
    },

    // Quick field picker
    OpenQuickFields,
    CloseQuickFields,
    QuickFieldsEvent(ListEvent),
    SaveQuickFields,

    // Navigation
    Back,
    Preview,
}
