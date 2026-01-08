use crate::config::repository::transfer::{get_transfer_config, save_transfer_config};
use crate::transfer::{ResolverFallback, TransferConfig};
use crate::tui::element::FocusId;
use crate::tui::resource::Resource;
use crate::tui::widgets::{TreeState, ListState, ScrollableState, TextInputField};
use crate::tui::{App, AppId, Command, LayeredView, Subscription};

use crate::api::{FieldMetadata, FieldType};
use super::state::{DeleteTarget, EditorParams, EntityMappingForm, FieldMappingForm, ResolverForm, Msg, State, TransformType};
use super::super::preview::PreviewParams;
use super::view;

pub struct MappingEditorApp;

impl crate::tui::AppState for State {}

impl App for MappingEditorApp {
    type State = State;
    type Msg = Msg;
    type InitParams = EditorParams;

    fn init(params: EditorParams) -> (State, Command<Msg>) {
        let state = State {
            config_name: params.config_name.clone(),
            config: Resource::Loading,
            tree_state: TreeState::with_selection(),
            source_entities: Resource::Loading,
            target_entities: Resource::Loading,
            show_entity_modal: false,
            entity_form: EntityMappingForm::default(),
            editing_entity_idx: None,
            entity_modal_scroll: ScrollableState::new(),
            show_field_modal: false,
            field_form: FieldMappingForm::default(),
            editing_field: None,
            source_fields: Resource::NotAsked,
            target_fields: Resource::NotAsked,
            current_field_entity_idx: None,
            pending_field_modal: None,
            related_fields: std::collections::HashMap::new(),
            show_resolver_modal: false,
            resolver_form: ResolverForm::default(),
            editing_resolver_for_entity: None,
            editing_resolver_idx: None,
            resolver_match_fields: Resource::NotAsked,
            resolver_match_fields_entity: None,
            resolver_source_fields: Resource::NotAsked,
            resolver_related_fields: std::collections::HashMap::new(),
            show_delete_confirm: false,
            delete_target: None,
            show_quick_fields_modal: false,
            quick_fields_available: Vec::new(),
            quick_fields_list_state: ListState::with_selection(),
            quick_fields_entity_idx: None,
            pending_quick_fields: false,
            quick_fields_source_prefix: TextInputField::default(),
            quick_fields_target_prefix: TextInputField::default(),
            entity_target_fields_cache: std::collections::HashMap::new(),
            clipboard: None,
        };

        // Load config first (fast, local DB), then load entities with loading screen
        let cmd = Command::perform(
            load_config(params.config_name),
            Msg::ConfigLoaded,
        );

        (state, cmd)
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::ConfigLoaded(result) => {
                match result {
                    Ok(config) => {
                        let source_env = config.source_env.clone();
                        let target_env = config.target_env.clone();
                        state.config = Resource::Success(config);
                        state.tree_state.invalidate_cache();

                        // Load entity lists with loading screen
                        // Use unique task names even when source_env == target_env
                        Command::perform_parallel()
                            .add_task(
                                format!("Loading source entities from {}", source_env),
                                load_entities_for_env(source_env),
                            )
                            .add_task(
                                format!("Loading target entities from {}", target_env),
                                load_entities_for_env(target_env),
                            )
                            .with_title("Loading Entity Metadata")
                            .on_complete(AppId::TransferMappingEditor)
                            .build(|task_idx, result| {
                                let data = result.downcast::<Result<Vec<String>, String>>().unwrap();
                                match task_idx {
                                    0 => Msg::SourceEntitiesLoaded(*data),
                                    _ => Msg::TargetEntitiesLoaded(*data),
                                }
                            })
                    }
                    Err(e) => {
                        state.config = Resource::Failure(e);
                        Command::None
                    }
                }
            }

            Msg::SourceEntitiesLoaded(result) => {
                state.source_entities = match result {
                    Ok(entities) => Resource::Success(entities),
                    Err(e) => Resource::Failure(e),
                };

                // Auto-select first entity if tree has items but no selection
                if state.tree_state.selected().is_none() {
                    if let Resource::Success(config) = &state.config {
                        if !config.entity_mappings.is_empty() {
                            // Select first entity
                            state.tree_state.select(Some("entity_0".to_string()));
                        }
                    }
                }

                // Set focus when returning from loading screen
                Command::set_focus(FocusId::new("mapping-tree"))
            }

            Msg::TargetEntitiesLoaded(result) => {
                state.target_entities = match result {
                    Ok(entities) => Resource::Success(entities),
                    Err(e) => Resource::Failure(e),
                };
                Command::None
            }

            Msg::SourceFieldsLoaded(result) => {
                state.source_fields = match result {
                    Ok(fields) => Resource::Success(fields),
                    Err(e) => Resource::Failure(e),
                };
                // Check if we should open the field modal now
                try_open_pending_field_modal(state)
            }

            Msg::TargetFieldsLoaded(result) => {
                state.target_fields = match result {
                    Ok(fields) => {
                        // Cache fields for this entity (for showing types in tree)
                        if let Some(entity_idx) = state.current_field_entity_idx {
                            state.entity_target_fields_cache.insert(entity_idx, fields.clone());
                            // Invalidate tree cache so field types show up
                            state.tree_state.invalidate_cache();
                        }
                        Resource::Success(fields)
                    },
                    Err(e) => Resource::Failure(e),
                };
                // Check if we should open the field modal now
                try_open_pending_field_modal(state)
            }

            Msg::TreeEvent(event) => {
                state.tree_state.handle_event(event);
                Command::None
            }

            Msg::TreeSelect(id) => {
                state.tree_state.select(Some(id));
                Command::None
            }

            Msg::TreeViewportHeight(height) => {
                state.tree_state.set_viewport_height(height);
                Command::None
            }

            // Entity modal
            Msg::AddEntity => {
                state.show_entity_modal = true;
                state.editing_entity_idx = None;
                state.entity_form = EntityMappingForm::default();
                state.entity_form.priority.value = next_priority(state).to_string();
                state.entity_modal_scroll = ScrollableState::new();
                Command::set_focus(FocusId::new("entity-source"))
            }

            Msg::EditEntity(idx) => {
                if let Resource::Success(config) = &state.config {
                    if let Some(mapping) = config.entity_mappings.get(idx) {
                        state.show_entity_modal = true;
                        state.editing_entity_idx = Some(idx);
                        state.entity_form = EntityMappingForm::from_mapping(mapping);
                        state.entity_modal_scroll = ScrollableState::new();

                        // Load source fields for source filter autocomplete
                        let source_entity = mapping.source_entity.clone();
                        let source_env = config.source_env.clone();
                        state.source_fields = Resource::Loading;

                        // Load target fields for target filter autocomplete
                        let target_entity = mapping.target_entity.clone();
                        let target_env = config.target_env.clone();
                        state.target_fields = Resource::Loading;

                        return Command::batch(vec![
                            Command::set_focus(FocusId::new("entity-source")),
                            Command::perform(
                                load_entity_fields(source_env, source_entity),
                                Msg::SourceFieldsLoaded,
                            ),
                            Command::perform(
                                load_entity_fields(target_env, target_entity),
                                Msg::TargetFieldsLoaded,
                            ),
                        ]);
                    }
                }
                Command::None
            }

            Msg::DeleteEntity(idx) => {
                state.show_delete_confirm = true;
                state.delete_target = Some(DeleteTarget::Entity(idx));
                Command::None
            }

            Msg::CloseEntityModal => {
                state.show_entity_modal = false;
                state.editing_entity_idx = None;
                Command::set_focus(FocusId::new("mapping-tree"))
            }

            Msg::SaveEntity => {
                if !state.entity_form.is_valid() {
                    return Command::None;
                }

                if let Resource::Success(config) = &mut state.config {
                    let mut new_mapping = state.entity_form.to_mapping();

                    if let Some(idx) = state.editing_entity_idx {
                        // Editing: preserve field mappings and resolvers
                        if let Some(existing) = config.entity_mappings.get(idx) {
                            new_mapping.field_mappings = existing.field_mappings.clone();
                            new_mapping.resolvers = existing.resolvers.clone();
                        }
                        config.entity_mappings[idx] = new_mapping;
                    } else {
                        // Adding new
                        config.entity_mappings.push(new_mapping);
                    }

                    state.tree_state.invalidate_cache();
                }

                state.show_entity_modal = false;
                state.editing_entity_idx = None;

                // Auto-save
                if let Resource::Success(config) = &state.config {
                    let config_clone = config.clone();
                    return Command::batch(vec![
                        Command::perform(save_config(config_clone), Msg::SaveCompleted),
                        Command::set_focus(FocusId::new("mapping-tree")),
                    ]);
                }
                Command::set_focus(FocusId::new("mapping-tree"))
            }

            Msg::EntityFormSource(event) => {
                let options = match &state.source_entities {
                    Resource::Success(entities) => entities.clone(),
                    _ => vec![],
                };

                // Check if this is a selection event that should trigger field loading
                let should_load_fields = matches!(
                    &event,
                    crate::tui::widgets::AutocompleteEvent::Select(_) |
                    crate::tui::widgets::AutocompleteEvent::Navigate(crossterm::event::KeyCode::Enter)
                );

                state.entity_form.source_entity.handle_event::<Msg>(event, &options);

                // Load source entity fields for filter autocomplete when entity is selected
                if should_load_fields {
                    let entity_name = state.entity_form.source_entity.value.trim().to_string();
                    if !entity_name.is_empty() && options.contains(&entity_name) {
                        if let Resource::Success(config) = &state.config {
                            let source_env = config.source_env.clone();
                            state.source_fields = Resource::Loading;
                            return Command::perform(
                                load_entity_fields(source_env, entity_name),
                                Msg::SourceFieldsLoaded,
                            );
                        }
                    }
                }

                Command::None
            }

            Msg::EntityFormTarget(event) => {
                let options = match &state.target_entities {
                    Resource::Success(entities) => entities.clone(),
                    _ => vec![],
                };

                // Check if this is a selection event that should trigger field loading
                let should_load_fields = matches!(
                    &event,
                    crate::tui::widgets::AutocompleteEvent::Select(_) |
                    crate::tui::widgets::AutocompleteEvent::Navigate(crossterm::event::KeyCode::Enter)
                );

                state.entity_form.target_entity.handle_event::<Msg>(event, &options);

                // Load target entity fields for target filter autocomplete when entity is selected
                if should_load_fields {
                    let entity_name = state.entity_form.target_entity.value.trim().to_string();
                    if !entity_name.is_empty() && options.contains(&entity_name) {
                        if let Resource::Success(config) = &state.config {
                            let target_env = config.target_env.clone();
                            state.target_fields = Resource::Loading;
                            return Command::perform(
                                load_entity_fields(target_env, entity_name),
                                Msg::TargetFieldsLoaded,
                            );
                        }
                    }
                }

                Command::None
            }

            Msg::EntityFormPriority(event) => {
                state.entity_form.priority.handle_event(event, Some(10));
                Command::None
            }

            Msg::EntityFormToggleCreates => {
                state.entity_form.allow_creates = !state.entity_form.allow_creates;
                Command::None
            }

            Msg::EntityFormToggleUpdates => {
                state.entity_form.allow_updates = !state.entity_form.allow_updates;
                Command::None
            }

            Msg::EntityFormToggleDeletes => {
                state.entity_form.allow_deletes = !state.entity_form.allow_deletes;
                Command::None
            }

            Msg::EntityFormToggleDeactivates => {
                state.entity_form.allow_deactivates = !state.entity_form.allow_deactivates;
                Command::None
            }

            // Entity source filter
            Msg::EntityFormToggleFilter => {
                state.entity_form.filter_enabled = !state.entity_form.filter_enabled;
                Command::None
            }

            Msg::EntityFormFilterField(event) => {
                let options = match &state.source_fields {
                    Resource::Success(fields) => fields.iter().map(|f| f.logical_name.clone()).collect(),
                    _ => vec![],
                };
                state.entity_form.filter_field.handle_event::<Msg>(event, &options);
                Command::None
            }

            Msg::EntityFormToggleFilterCondition => {
                state.entity_form.filter_condition_type = state.entity_form.filter_condition_type.next();
                Command::None
            }

            Msg::EntityFormFilterValue(event) => {
                state.entity_form.filter_value.handle_event(event, Some(200));
                Command::None
            }

            // Entity target filter
            Msg::EntityFormToggleTargetFilter => {
                state.entity_form.target_filter_enabled = !state.entity_form.target_filter_enabled;
                Command::None
            }

            Msg::EntityFormTargetFilterField(event) => {
                let options = match &state.target_fields {
                    Resource::Success(fields) => fields.iter().map(|f| f.logical_name.clone()).collect(),
                    _ => vec![],
                };
                state.entity_form.target_filter_field.handle_event::<Msg>(event, &options);
                Command::None
            }

            Msg::EntityFormToggleTargetFilterCondition => {
                state.entity_form.target_filter_condition_type = state.entity_form.target_filter_condition_type.next();
                Command::None
            }

            Msg::EntityFormTargetFilterValue(event) => {
                state.entity_form.target_filter_value.handle_event(event, Some(200));
                Command::None
            }

            // Entity modal scroll
            Msg::EntityModalScroll(key) => {
                let content_height = state.entity_modal_scroll.content_height().unwrap_or(50);
                let viewport_height = state.entity_modal_scroll.viewport_height().unwrap_or(25);
                state.entity_modal_scroll.handle_key(key, content_height, viewport_height);
                Command::None
            }

            Msg::EntityModalViewport(viewport_height, content_height, viewport_width, content_width) => {
                state.entity_modal_scroll.set_viewport_height(viewport_height);
                state.entity_modal_scroll.update_scroll(viewport_height, content_height);
                state.entity_modal_scroll.set_viewport_width(viewport_width);
                state.entity_modal_scroll.update_horizontal_scroll(viewport_width, content_width);
                Command::None
            }

            // Field modal
            Msg::AddField(entity_idx) => {
                // Check if fields already loaded for this entity
                if state.current_field_entity_idx == Some(entity_idx)
                    && matches!(&state.source_fields, Resource::Success(_))
                    && matches!(&state.target_fields, Resource::Success(_))
                {
                    // Fields already loaded - open modal immediately
                    state.show_field_modal = true;
                    state.editing_field = Some((entity_idx, usize::MAX));
                    state.field_form = FieldMappingForm::default();
                    return Command::set_focus(FocusId::new("field-target"));
                }

                // Need to load fields first - extract entity info
                let entity_info = if let Resource::Success(config) = &state.config {
                    config.entity_mappings.get(entity_idx).map(|e| {
                        (
                            config.source_env.clone(),
                            config.target_env.clone(),
                            e.source_entity.clone(),
                            e.target_entity.clone(),
                        )
                    })
                } else {
                    None
                };

                if let Some((source_env, target_env, source_entity, target_entity)) = entity_info {
                    // Store pending modal open
                    state.pending_field_modal = Some((entity_idx, None));
                    state.current_field_entity_idx = Some(entity_idx);
                    state.source_fields = Resource::Loading;
                    state.target_fields = Resource::Loading;

                    // Use loading screen for field metadata fetch
                    // Use "source"/"target" prefix to ensure unique task names even when same entity/env
                    return Command::perform_parallel()
                        .add_task(
                            format!("Loading source fields for {} ({})", source_entity, source_env),
                            load_entity_fields(source_env.clone(), source_entity),
                        )
                        .add_task(
                            format!("Loading target fields for {} ({})", target_entity, target_env),
                            load_entity_fields(target_env.clone(), target_entity),
                        )
                        .with_title("Loading Field Metadata")
                        .on_complete(AppId::TransferMappingEditor)
                        .build(|task_idx, result| {
                            let data = result.downcast::<Result<Vec<FieldMetadata>, String>>().unwrap();
                            match task_idx {
                                0 => Msg::SourceFieldsLoaded(*data),
                                _ => Msg::TargetFieldsLoaded(*data),
                            }
                        });
                }
                Command::None
            }

            Msg::EditField(entity_idx, field_idx) => {
                // Check if fields already loaded for this entity
                if state.current_field_entity_idx == Some(entity_idx)
                    && matches!(&state.source_fields, Resource::Success(_))
                    && matches!(&state.target_fields, Resource::Success(_))
                {
                    // Fields already loaded - open modal immediately
                    if let Resource::Success(config) = &state.config {
                        if let Some(entity) = config.entity_mappings.get(entity_idx) {
                            if let Some(mapping) = entity.field_mappings.get(field_idx) {
                                state.show_field_modal = true;
                                state.editing_field = Some((entity_idx, field_idx));
                                state.field_form = FieldMappingForm::from_mapping(mapping);
                                return Command::set_focus(FocusId::new("field-target"));
                            }
                        }
                    }
                    return Command::None;
                }

                // Need to load fields first - extract entity info
                let entity_info = if let Resource::Success(config) = &state.config {
                    config.entity_mappings.get(entity_idx).map(|e| {
                        (
                            config.source_env.clone(),
                            config.target_env.clone(),
                            e.source_entity.clone(),
                            e.target_entity.clone(),
                        )
                    })
                } else {
                    None
                };

                if let Some((source_env, target_env, source_entity, target_entity)) = entity_info {
                    // Store pending modal open with field index
                    state.pending_field_modal = Some((entity_idx, Some(field_idx)));
                    state.current_field_entity_idx = Some(entity_idx);
                    state.source_fields = Resource::Loading;
                    state.target_fields = Resource::Loading;

                    // Use loading screen for field metadata fetch
                    // Use "source"/"target" prefix to ensure unique task names even when same entity/env
                    return Command::perform_parallel()
                        .add_task(
                            format!("Loading source fields for {} ({})", source_entity, source_env),
                            load_entity_fields(source_env.clone(), source_entity),
                        )
                        .add_task(
                            format!("Loading target fields for {} ({})", target_entity, target_env),
                            load_entity_fields(target_env.clone(), target_entity),
                        )
                        .with_title("Loading Field Metadata")
                        .on_complete(AppId::TransferMappingEditor)
                        .build(|task_idx, result| {
                            let data = result.downcast::<Result<Vec<FieldMetadata>, String>>().unwrap();
                            match task_idx {
                                0 => Msg::SourceFieldsLoaded(*data),
                                _ => Msg::TargetFieldsLoaded(*data),
                            }
                        });
                }
                Command::None
            }

            Msg::DeleteField(entity_idx, field_idx) => {
                state.show_delete_confirm = true;
                state.delete_target = Some(DeleteTarget::Field(entity_idx, field_idx));
                Command::None
            }

            Msg::CopyField(entity_idx, field_idx) => {
                if let Resource::Success(config) = &state.config {
                    if let Some(entity) = config.entity_mappings.get(entity_idx) {
                        if let Some(field_mapping) = entity.field_mappings.get(field_idx) {
                            state.clipboard = Some(field_mapping.clone());
                            log::info!("Copied field mapping '{}' to clipboard", field_mapping.target_field);
                        }
                    }
                }
                Command::None
            }

            Msg::PasteField(entity_idx) => {
                let Some(field_mapping) = state.clipboard.clone() else {
                    return Command::None;
                };

                if let Resource::Success(config) = &mut state.config {
                    if let Some(entity) = config.entity_mappings.get_mut(entity_idx) {
                        entity.field_mappings.push(field_mapping.clone());
                        log::info!("Pasted field mapping '{}' to entity '{}'", field_mapping.target_field, entity.target_entity);
                        state.tree_state.invalidate_cache();

                        // Auto-save
                        let config_clone = config.clone();
                        return Command::perform(save_config(config_clone), Msg::SaveCompleted);
                    }
                }
                Command::None
            }

            Msg::CloseFieldModal => {
                state.show_field_modal = false;
                state.editing_field = None;
                state.related_fields.clear();
                Command::set_focus(FocusId::new("mapping-tree"))
            }

            Msg::SaveField => {
                if !state.field_form.is_valid() {
                    return Command::None;
                }

                if let Some(new_mapping) = state.field_form.to_mapping() {
                    if let Resource::Success(config) = &mut state.config {
                        if let Some((entity_idx, field_idx)) = state.editing_field {
                            if let Some(entity) = config.entity_mappings.get_mut(entity_idx) {
                                if field_idx == usize::MAX {
                                    // Adding new
                                    entity.field_mappings.push(new_mapping);
                                } else {
                                    // Editing existing
                                    entity.field_mappings[field_idx] = new_mapping;
                                }
                                state.tree_state.invalidate_cache();
                            }
                        }
                    }
                }

                state.show_field_modal = false;
                state.editing_field = None;
                state.related_fields.clear();

                // Auto-save
                if let Resource::Success(config) = &state.config {
                    let config_clone = config.clone();
                    return Command::batch(vec![
                        Command::perform(save_config(config_clone), Msg::SaveCompleted),
                        Command::set_focus(FocusId::new("mapping-tree")),
                    ]);
                }
                Command::set_focus(FocusId::new("mapping-tree"))
            }

            Msg::FieldFormTarget(event) => {
                let options: Vec<String> = match &state.target_fields {
                    Resource::Success(fields) => fields.iter().map(|f| f.logical_name.clone()).collect(),
                    _ => vec![],
                };
                state.field_form.target_field.handle_event::<Msg>(event, &options);
                Command::None
            }

            Msg::FieldFormSourcePath(event) => {
                let options = get_source_options(state);
                state.field_form.source_path.handle_event::<Msg>(event, &options);
                // Check if we need to load related entity fields
                check_for_nested_lookup(state, &state.field_form.source_path.value.clone())
            }

            Msg::FieldFormConstant(event) => {
                state.field_form.constant_value.handle_event(event, Some(500));
                Command::None
            }

            Msg::FieldFormToggleType => {
                state.field_form.transform_type = state.field_form.transform_type.next();
                // Clear resolver when changing transform type
                state.field_form.resolver_name = None;

                // Auto-prefill ValueMap entries for OptionSet fields
                if state.field_form.transform_type == TransformType::ValueMap {
                    // Copy source_path to value_map_source when switching if not already set
                    if state.field_form.value_map_source.value.is_empty()
                        && !state.field_form.source_path.value.is_empty()
                    {
                        state.field_form.value_map_source.value = state.field_form.source_path.value.clone();
                    }

                    // Try to prefill from source field option values
                    let source_path = state.field_form.value_map_source.value.clone();
                    if !source_path.is_empty() {
                        let base_field = source_path.split('.').next().unwrap_or(&source_path);
                        if let Resource::Success(fields) = &state.source_fields {
                            if let Some(source_field) = fields.iter().find(|f| f.logical_name == base_field) {
                                state.field_form.prefill_valuemap_from_optionset(source_field);
                            }
                        }
                    }
                }

                Command::None
            }

            Msg::FieldFormCycleResolver => {
                // Cycle through: None -> resolver1 -> resolver2 -> ... -> None
                // Get resolvers from the current entity being edited
                if let (Resource::Success(config), Some((entity_idx, _))) = (&state.config, state.editing_field) {
                    let resolver_names: Vec<&str> = config.entity_mappings
                        .get(entity_idx)
                        .map(|em| em.resolvers.iter().map(|r| r.name.as_str()).collect())
                        .unwrap_or_default();

                    if resolver_names.is_empty() {
                        state.field_form.resolver_name = None;
                    } else {
                        let current_idx = state.field_form.resolver_name.as_ref()
                            .and_then(|name| resolver_names.iter().position(|r| r == name));

                        let next_idx = match current_idx {
                            None => Some(0), // None -> first resolver
                            Some(idx) if idx + 1 < resolver_names.len() => Some(idx + 1), // next resolver
                            Some(_) => None, // last resolver -> None
                        };

                        state.field_form.resolver_name = next_idx.map(|i| resolver_names[i].to_string());
                    }
                }
                Command::None
            }

            // Conditional transform fields
            Msg::FieldFormConditionSource(event) => {
                let options = get_source_options(state);
                state.field_form.condition_source.handle_event::<Msg>(event, &options);
                // Check if we need to load related entity fields
                check_for_nested_lookup(state, &state.field_form.condition_source.value.clone())
            }

            Msg::FieldFormToggleConditionType => {
                state.field_form.condition_type = state.field_form.condition_type.next();
                Command::None
            }

            Msg::FieldFormConditionValue(event) => {
                state.field_form.condition_value.handle_event(event, Some(100));
                Command::None
            }

            Msg::FieldFormThenValue(event) => {
                state.field_form.then_value.handle_event(event, Some(100));
                Command::None
            }

            Msg::FieldFormElseValue(event) => {
                state.field_form.else_value.handle_event(event, Some(100));
                Command::None
            }

            // ValueMap transform fields
            Msg::FieldFormValueMapSource(event) => {
                let options = get_source_options(state);
                state.field_form.value_map_source.handle_event::<Msg>(event, &options);

                // Auto-prefill when source field is selected and is OptionSet
                let current_value = state.field_form.value_map_source.value.trim().to_string();
                if !current_value.is_empty() {
                    let base_field = current_value.split('.').next().unwrap_or(&current_value);
                    if let Resource::Success(fields) = &state.source_fields {
                        if let Some(source_field) = fields.iter().find(|f| f.logical_name == base_field) {
                            // Only prefill if entries are empty (don't override user's work)
                            state.field_form.prefill_valuemap_from_optionset(source_field);
                        }
                    }
                }

                // Check if we need to load related entity fields
                check_for_nested_lookup(state, &state.field_form.value_map_source.value.clone())
            }

            Msg::FieldFormToggleFallback => {
                state.field_form.value_map_fallback = state.field_form.value_map_fallback.next();
                Command::None
            }

            Msg::FieldFormValueMapDefault(event) => {
                state.field_form.value_map_default.handle_event(event, Some(100));
                Command::None
            }

            Msg::FieldFormAddMapping => {
                state.field_form.add_value_map_entry();
                Command::None
            }

            Msg::FieldFormRemoveMapping(idx) => {
                state.field_form.remove_value_map_entry(idx);
                Command::None
            }

            Msg::FieldFormMappingSource(idx, event) => {
                if let Some(entry) = state.field_form.value_map_entries.get_mut(idx) {
                    entry.source_value.handle_event(event, Some(100));
                }
                Command::None
            }

            Msg::FieldFormMappingTarget(idx, event) => {
                if let Some(entry) = state.field_form.value_map_entries.get_mut(idx) {
                    entry.target_value.handle_event(event, Some(100));
                }
                Command::None
            }

            Msg::FieldFormCycleSourceOption(idx, backwards) => {
                // Get source field's option values
                let source_field_name = state.field_form.value_map_source.value.trim();
                let source_options: Vec<_> = if let Resource::Success(fields) = &state.source_fields {
                    fields.iter()
                        .find(|f| f.logical_name == source_field_name)
                        .map(|f| f.option_values.clone())
                        .unwrap_or_default()
                } else {
                    vec![]
                };

                if !source_options.is_empty() {
                    if let Some(entry) = state.field_form.value_map_entries.get_mut(idx) {
                        let current_value = entry.source_value.value.trim();

                        let current_idx = source_options.iter().position(|opt| {
                            opt.value.to_string() == current_value
                        });

                        let next_idx = match current_idx {
                            Some(i) if backwards => {
                                if i == 0 { source_options.len() - 1 } else { i - 1 }
                            }
                            Some(i) => {
                                if i + 1 >= source_options.len() { 0 } else { i + 1 }
                            }
                            None => 0,
                        };

                        entry.source_value.value = source_options[next_idx].value.to_string();
                    }
                }
                Command::None
            }

            Msg::FieldFormCycleTargetOption(idx, backwards) => {
                // Get target field's option values
                let target_field_name = state.field_form.target_field.value.trim();
                let target_options: Vec<_> = if let Resource::Success(fields) = &state.target_fields {
                    fields.iter()
                        .find(|f| f.logical_name == target_field_name)
                        .map(|f| f.option_values.clone())
                        .unwrap_or_default()
                } else {
                    vec![]
                };

                if !target_options.is_empty() {
                    if let Some(entry) = state.field_form.value_map_entries.get_mut(idx) {
                        let current_value = entry.target_value.value.trim();

                        // Include null as an option (represented as "null" string)
                        // Options cycle: opt1 -> opt2 -> ... -> optN -> null -> opt1
                        let is_null = current_value == "null" || current_value.is_empty();

                        let current_idx = if is_null {
                            None // null is "after" all options
                        } else {
                            target_options.iter().position(|opt| {
                                opt.value.to_string() == current_value
                            })
                        };

                        // null is at the end, after all options
                        let null_idx = target_options.len();

                        let next_idx = match current_idx {
                            Some(i) if backwards => {
                                if i == 0 { null_idx } else { i - 1 }
                            }
                            Some(i) => {
                                i + 1 // might be null_idx
                            }
                            None => {
                                // Currently null
                                if backwards {
                                    target_options.len() - 1
                                } else {
                                    0
                                }
                            }
                        };

                        if next_idx >= target_options.len() {
                            entry.target_value.value = "null".to_string();
                        } else {
                            entry.target_value.value = target_options[next_idx].value.to_string();
                        }
                    }
                }
                Command::None
            }

            Msg::FieldFormValueMapScroll(key) => {
                let viewport_height = state.field_form.value_map_scroll.viewport_height().unwrap_or(12);
                let content_height = state.field_form.value_map_scroll.content_height().unwrap_or(12);
                state.field_form.value_map_scroll.handle_key(key, content_height, viewport_height);
                Command::None
            }

            Msg::FieldFormValueMapScrollDimensions(vh, ch, _vw, _cw) => {
                state.field_form.value_map_scroll.set_viewport_height(vh);
                state.field_form.value_map_scroll.update_scroll(vh, ch);
                Command::None
            }

            // Format transform fields
            Msg::FieldFormFormatTemplate(event) => {
                state.field_form.format_template.handle_event(event, Some(500));
                Command::None
            }

            Msg::FieldFormToggleNullHandling => {
                state.field_form.format_null_handling = state.field_form.format_null_handling.next();
                Command::None
            }

            // Replace transform fields
            Msg::FieldFormReplaceSource(event) => {
                let options = get_source_options(state);
                state.field_form.replace_source.handle_event::<Msg>(event, &options);
                // Check if we need to load related entity fields
                check_for_nested_lookup(state, &state.field_form.replace_source.value.clone())
            }

            Msg::FieldFormAddReplace => {
                state.field_form.add_replace_entry();
                Command::None
            }

            Msg::FieldFormRemoveReplace(idx) => {
                state.field_form.remove_replace_entry(idx);
                Command::None
            }

            Msg::FieldFormReplacePattern(idx, event) => {
                if let Some(entry) = state.field_form.replace_entries.get_mut(idx) {
                    entry.pattern.handle_event(event, Some(200));
                }
                Command::None
            }

            Msg::FieldFormReplaceReplacement(idx, event) => {
                if let Some(entry) = state.field_form.replace_entries.get_mut(idx) {
                    entry.replacement.handle_event(event, Some(200));
                }
                Command::None
            }

            Msg::FieldFormToggleReplaceRegex(idx) => {
                if let Some(entry) = state.field_form.replace_entries.get_mut(idx) {
                    entry.is_regex = !entry.is_regex;
                }
                Command::None
            }

            // Resolver modal
            Msg::AddResolver(entity_idx) => {
                if let Resource::Success(config) = &state.config {
                    state.show_resolver_modal = true;
                    state.editing_resolver_for_entity = Some(entity_idx);
                    state.editing_resolver_idx = None;
                    state.resolver_form = ResolverForm::default();
                    state.resolver_match_fields = Resource::NotAsked;
                    state.resolver_match_fields_entity = None;
                    state.resolver_source_fields = Resource::Loading;

                    // Load source entity fields for source_path autocomplete
                    if let Some(entity_mapping) = config.entity_mappings.get(entity_idx) {
                        let source_entity = entity_mapping.source_entity.clone();
                        let source_env = config.source_env.clone();
                        return Command::batch(vec![
                            Command::set_focus(FocusId::new("resolver-name")),
                            Command::perform(
                                load_entity_fields(source_env, source_entity),
                                Msg::ResolverSourceFieldsLoaded,
                            ),
                        ]);
                    }
                }
                Command::set_focus(FocusId::new("resolver-name"))
            }

            Msg::EditResolver(entity_idx, resolver_idx) => {
                if let Resource::Success(config) = &state.config {
                    if let Some(entity_mapping) = config.entity_mappings.get(entity_idx) {
                        if let Some(resolver) = entity_mapping.resolvers.get(resolver_idx) {
                            state.show_resolver_modal = true;
                            state.editing_resolver_for_entity = Some(entity_idx);
                            state.editing_resolver_idx = Some(resolver_idx);
                            state.resolver_form = ResolverForm::from_resolver(resolver);

                            // Load match fields for the selected source entity (target env)
                            let resolver_entity = resolver.source_entity.clone();
                            let target_env = config.target_env.clone();
                            state.resolver_match_fields = Resource::Loading;
                            state.resolver_match_fields_entity = Some(resolver_entity.clone());

                            // Load source entity fields for source_path autocomplete (source env)
                            let source_entity = entity_mapping.source_entity.clone();
                            let source_env = config.source_env.clone();
                            state.resolver_source_fields = Resource::Loading;

                            return Command::batch(vec![
                                Command::perform(
                                    load_entity_fields(target_env, resolver_entity),
                                    Msg::ResolverMatchFieldsLoaded,
                                ),
                                Command::perform(
                                    load_entity_fields(source_env, source_entity),
                                    Msg::ResolverSourceFieldsLoaded,
                                ),
                            ]);
                        }
                    }
                }
                Command::None
            }

            Msg::DeleteResolver(entity_idx, resolver_idx) => {
                state.show_delete_confirm = true;
                state.delete_target = Some(DeleteTarget::Resolver(entity_idx, resolver_idx));
                Command::None
            }

            Msg::CloseResolverModal => {
                state.show_resolver_modal = false;
                state.editing_resolver_for_entity = None;
                state.editing_resolver_idx = None;
                state.resolver_match_fields = Resource::NotAsked;
                state.resolver_match_fields_entity = None;
                state.resolver_source_fields = Resource::NotAsked;
                state.resolver_related_fields.clear();
                Command::set_focus(FocusId::new("mapping-tree"))
            }

            Msg::SaveResolver => {
                if !state.resolver_form.is_valid() {
                    return Command::None;
                }

                if let (Resource::Success(config), Some(entity_idx)) = (&mut state.config, state.editing_resolver_for_entity) {
                    if let Some(entity_mapping) = config.entity_mappings.get_mut(entity_idx) {
                        let mut new_resolver = state.resolver_form.to_resolver();

                        if let Some(resolver_idx) = state.editing_resolver_idx {
                            // Editing: preserve ID if present
                            if let Some(existing) = entity_mapping.resolvers.get(resolver_idx) {
                                new_resolver.id = existing.id;
                            }
                            entity_mapping.resolvers[resolver_idx] = new_resolver;
                        } else {
                            // Adding new
                            entity_mapping.resolvers.push(new_resolver);
                        }

                        state.tree_state.invalidate_cache();
                    }
                }

                state.show_resolver_modal = false;
                state.editing_resolver_for_entity = None;
                state.editing_resolver_idx = None;
                state.resolver_match_fields = Resource::NotAsked;
                state.resolver_match_fields_entity = None;
                state.resolver_source_fields = Resource::NotAsked;

                // Auto-save
                if let Resource::Success(config) = &state.config {
                    let config_clone = config.clone();
                    return Command::batch(vec![
                        Command::perform(save_config(config_clone), Msg::SaveCompleted),
                        Command::set_focus(FocusId::new("mapping-tree")),
                    ]);
                }
                Command::set_focus(FocusId::new("mapping-tree"))
            }

            Msg::ResolverFormName(event) => {
                state.resolver_form.name.handle_event(event, Some(100));
                Command::None
            }

            Msg::ResolverFormSourceEntity(event) => {
                let options = match &state.target_entities {
                    Resource::Success(entities) => entities.clone(),
                    _ => vec![],
                };
                state.resolver_form.source_entity.handle_event::<Msg>(event, &options);

                // When entity changes, load its fields for match_field autocomplete
                check_resolver_entity_selection(state)
            }

            Msg::ResolverAddMatchFieldRow => {
                state.resolver_form.add_row();
                Command::None
            }

            Msg::ResolverRemoveMatchFieldRow => {
                state.resolver_form.remove_current_row();
                Command::None
            }

            Msg::ResolverMatchField(idx, event) => {
                state.resolver_form.focused_row = idx;
                let options: Vec<String> = match &state.resolver_match_fields {
                    Resource::Success(fields) => fields.iter().map(|f| f.logical_name.clone()).collect(),
                    _ => vec![],
                };
                if let Some(row) = state.resolver_form.match_field_rows.get_mut(idx) {
                    row.target_field.handle_event::<Msg>(event, &options);
                }
                Command::None
            }

            Msg::ResolverSourcePath(idx, event) => {
                state.resolver_form.focused_row = idx;
                let options = build_resolver_source_options(state);
                if let Some(row) = state.resolver_form.match_field_rows.get_mut(idx) {
                    row.source_path.handle_event::<Msg>(event, &options);
                }
                // Check if we need to load related entity fields for nested lookup
                if let Some(row) = state.resolver_form.match_field_rows.get(idx) {
                    let current_value = row.source_path.value.clone();
                    return check_resolver_nested_lookup(state, &current_value);
                }
                Command::None
            }

            Msg::ResolverFormCycleFallback => {
                state.resolver_form.fallback = state.resolver_form.fallback.cycle();
                Command::None
            }

            Msg::ResolverFormDefaultGuid(event) => {
                state.resolver_form.default_guid.handle_event(event, None);
                Command::None
            }

            Msg::ResolverMatchFieldsLoaded(result) => {
                state.resolver_match_fields = match result {
                    Ok(fields) => Resource::Success(fields),
                    Err(e) => Resource::Failure(e),
                };
                Command::None
            }

            Msg::ResolverSourceFieldsLoaded(result) => {
                state.resolver_source_fields = match result {
                    Ok(fields) => Resource::Success(fields),
                    Err(e) => Resource::Failure(e),
                };
                Command::None
            }

            Msg::ResolverRelatedFieldsLoaded { lookup_field, result } => {
                state.resolver_related_fields.insert(
                    lookup_field,
                    match result {
                        Ok(fields) => Resource::Success(fields),
                        Err(e) => Resource::Failure(e),
                    }
                );
                // Restore focus to the source path field
                Command::set_focus(FocusId::new("resolver-source-0"))
            }

            // Delete confirmation
            Msg::ConfirmDelete => {
                if let Some(target) = state.delete_target.take() {
                    if let Resource::Success(config) = &mut state.config {
                        match target {
                            DeleteTarget::Entity(idx) => {
                                if idx < config.entity_mappings.len() {
                                    config.entity_mappings.remove(idx);
                                    state.tree_state.invalidate_cache();
                                }
                            }
                            DeleteTarget::Field(entity_idx, field_idx) => {
                                if let Some(entity) = config.entity_mappings.get_mut(entity_idx) {
                                    if field_idx < entity.field_mappings.len() {
                                        entity.field_mappings.remove(field_idx);
                                        state.tree_state.invalidate_cache();
                                    }
                                }
                            }
                            DeleteTarget::Resolver(entity_idx, resolver_idx) => {
                                if let Some(entity) = config.entity_mappings.get_mut(entity_idx) {
                                    if resolver_idx < entity.resolvers.len() {
                                        entity.resolvers.remove(resolver_idx);
                                        state.tree_state.invalidate_cache();
                                    }
                                }
                            }
                        }
                    }
                }
                state.show_delete_confirm = false;

                // Auto-save
                if let Resource::Success(config) = &state.config {
                    let config_clone = config.clone();
                    return Command::perform(save_config(config_clone), Msg::SaveCompleted);
                }
                Command::None
            }

            Msg::CancelDelete => {
                state.show_delete_confirm = false;
                state.delete_target = None;
                Command::None
            }

            Msg::SaveCompleted(result) => {
                if let Err(e) = result {
                    log::error!("Failed to save config: {}", e);
                    // TODO: Show error modal
                }
                Command::None
            }

            Msg::RelatedFieldsLoaded { lookup_field, result } => {
                state.related_fields.insert(
                    lookup_field,
                    match result {
                        Ok(fields) => Resource::Success(fields),
                        Err(e) => Resource::Failure(e),
                    }
                );

                // Restore focus to the appropriate source field based on transform type
                let focus_id = match state.field_form.transform_type {
                    TransformType::Copy => "field-source",
                    TransformType::Conditional => "field-condition-source",
                    TransformType::ValueMap => "field-valuemap-source",
                    _ => "field-source",
                };
                Command::set_focus(FocusId::new(focus_id))
            }

            // Quick field picker
            Msg::OpenQuickFields => {
                // Get current entity index from tree selection
                // Supports entity_*, field_*_*, and resolver_*_* formats
                let entity_idx = state.tree_state.selected().and_then(|s| {
                    if let Some(idx_str) = s.strip_prefix("entity_") {
                        idx_str.parse::<usize>().ok()
                    } else if let Some(rest) = s.strip_prefix("field_") {
                        rest.split('_').next().and_then(|idx| idx.parse::<usize>().ok())
                    } else if let Some(rest) = s.strip_prefix("resolver_") {
                        rest.split('_').next().and_then(|idx| idx.parse::<usize>().ok())
                    } else {
                        None
                    }
                });

                let Some(entity_idx) = entity_idx else {
                    return Command::None;
                };

                // Check if fields are already loaded for this entity
                if state.current_field_entity_idx == Some(entity_idx)
                    && matches!(&state.source_fields, Resource::Success(_))
                    && matches!(&state.target_fields, Resource::Success(_))
                {
                    // Fields loaded - open modal
                    state.quick_fields_entity_idx = Some(entity_idx);
                    state.quick_fields_source_prefix = TextInputField::default();
                    state.quick_fields_target_prefix = TextInputField::default();
                    state.quick_fields_available = state.compute_quick_fields(entity_idx, "", "");
                    state.quick_fields_list_state = ListState::with_selection();
                    state.show_quick_fields_modal = true;
                    return Command::set_focus(FocusId::new("quick-fields-list"));
                }

                // Need to load fields first
                let entity_info = if let Resource::Success(config) = &state.config {
                    config.entity_mappings.get(entity_idx).map(|e| {
                        (
                            config.source_env.clone(),
                            config.target_env.clone(),
                            e.source_entity.clone(),
                            e.target_entity.clone(),
                        )
                    })
                } else {
                    None
                };

                if let Some((source_env, target_env, source_entity, target_entity)) = entity_info {
                    state.quick_fields_entity_idx = Some(entity_idx);
                    state.current_field_entity_idx = Some(entity_idx);
                    state.source_fields = Resource::Loading;
                    state.target_fields = Resource::Loading;
                    state.pending_quick_fields = true;

                    return Command::perform_parallel()
                        .add_task(
                            format!("Loading source fields for {} ({})", source_entity, source_env),
                            load_entity_fields(source_env.clone(), source_entity),
                        )
                        .add_task(
                            format!("Loading target fields for {} ({})", target_entity, target_env),
                            load_entity_fields(target_env.clone(), target_entity),
                        )
                        .with_title("Loading Field Metadata")
                        .on_complete(AppId::TransferMappingEditor)
                        .build(|task_idx, result| {
                            let data = result.downcast::<Result<Vec<FieldMetadata>, String>>().unwrap();
                            match task_idx {
                                0 => Msg::SourceFieldsLoaded(*data),
                                _ => Msg::TargetFieldsLoaded(*data),
                            }
                        });
                }
                Command::None
            }

            Msg::CloseQuickFields => {
                state.show_quick_fields_modal = false;
                state.quick_fields_entity_idx = None;
                Command::set_focus(FocusId::new("mapping-tree"))
            }

            Msg::QuickFieldsEvent(event) => {
                let item_count = state.quick_fields_available.len();
                state.quick_fields_list_state.handle_event(event, item_count, 15);
                Command::None
            }

            Msg::QuickFieldsSourcePrefix(event) => {
                state.quick_fields_source_prefix.handle_event(event, Some(50));
                if let Some(idx) = state.quick_fields_entity_idx {
                    state.quick_fields_available = state.compute_quick_fields(
                        idx,
                        &state.quick_fields_source_prefix.value,
                        &state.quick_fields_target_prefix.value,
                    );
                    state.quick_fields_list_state.clear_multi_selection();
                }
                Command::None
            }

            Msg::QuickFieldsTargetPrefix(event) => {
                state.quick_fields_target_prefix.handle_event(event, Some(50));
                if let Some(idx) = state.quick_fields_entity_idx {
                    state.quick_fields_available = state.compute_quick_fields(
                        idx,
                        &state.quick_fields_source_prefix.value,
                        &state.quick_fields_target_prefix.value,
                    );
                    state.quick_fields_list_state.clear_multi_selection();
                }
                Command::None
            }

            Msg::SaveQuickFields => {
                let entity_idx = match state.quick_fields_entity_idx {
                    Some(idx) => idx,
                    None => {
                        state.show_quick_fields_modal = false;
                        return Command::set_focus(FocusId::new("mapping-tree"));
                    }
                };

                // Get all selected indices from ListState
                let selected_indices = state.quick_fields_list_state.all_selected();
                if selected_indices.is_empty() {
                    state.show_quick_fields_modal = false;
                    state.quick_fields_entity_idx = None;
                    return Command::set_focus(FocusId::new("mapping-tree"));
                }

                // Create Copy mappings for all selected fields
                if let Resource::Success(config) = &mut state.config {
                    if let Some(entity) = config.entity_mappings.get_mut(entity_idx) {
                        use crate::transfer::{FieldMapping, Transform, FieldPath};

                        for idx in selected_indices {
                            if let Some(field_match) = state.quick_fields_available.get(idx) {
                                let mapping = FieldMapping {
                                    id: None,
                                    target_field: field_match.target_logical_name.clone(),
                                    transform: Transform::Copy {
                                        source_path: FieldPath::simple(&field_match.source.logical_name),
                                        resolver: None,
                                    },
                                };
                                entity.field_mappings.push(mapping);
                            }
                        }

                        state.tree_state.invalidate_cache();
                    }
                }

                state.show_quick_fields_modal = false;
                state.quick_fields_entity_idx = None;

                // Auto-save
                if let Resource::Success(config) = &state.config {
                    let config_clone = config.clone();
                    return Command::batch(vec![
                        Command::perform(save_config(config_clone), Msg::SaveCompleted),
                        Command::set_focus(FocusId::new("mapping-tree")),
                    ]);
                }
                Command::set_focus(FocusId::new("mapping-tree"))
            }

            // Navigation
            Msg::Back => {
                Command::navigate_to(AppId::TransferConfigList)
            }

            Msg::Preview => {
                // Navigate to preview app with current config info
                if let Resource::Success(config) = &state.config {
                    let params = PreviewParams {
                        config_name: state.config_name.clone(),
                        source_env: config.source_env.clone(),
                        target_env: config.target_env.clone(),
                    };
                    Command::start_app(AppId::TransferPreview, params)
                } else {
                    log::warn!("Cannot preview: config not loaded");
                    Command::None
                }
            }
        }
    }

    fn view(state: &mut State) -> LayeredView<Msg> {
        let theme = &crate::global_runtime_config().theme;
        view::render(state, theme)
    }

    fn subscriptions(state: &State) -> Vec<Subscription<Msg>> {
        view::subscriptions(state)
    }

    fn title() -> &'static str {
        "Mapping Editor"
    }
}

async fn load_config(name: String) -> Result<TransferConfig, String> {
    let pool = &crate::global_config().pool;
    get_transfer_config(pool, &name)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Config '{}' not found", name))
}

async fn save_config(config: TransferConfig) -> Result<(), String> {
    let pool = &crate::global_config().pool;
    save_transfer_config(pool, &config)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn next_priority(state: &State) -> u32 {
    if let Resource::Success(config) = &state.config {
        config.entity_mappings.iter().map(|e| e.priority).max().unwrap_or(0) + 1
    } else {
        1
    }
}

/// Load entity list for an environment (with caching)
async fn load_entities_for_env(env_name: String) -> Result<Vec<String>, String> {
    let config = crate::global_config();
    let manager = crate::client_manager();

    // Try cache first (24 hours)
    match config.get_entity_cache(&env_name, 24).await {
        Ok(Some(cached)) => return Ok(cached),
        _ => {}
    }

    // Cache miss - fetch from API
    let client = manager
        .get_client(&env_name)
        .await
        .map_err(|e| format!("Failed to get client for {}: {}", env_name, e))?;

    let metadata_xml = client
        .fetch_metadata()
        .await
        .map_err(|e| format!("Failed to fetch metadata: {}", e))?;

    let entities = crate::api::metadata::parse_entity_list(&metadata_xml)
        .map_err(|e| format!("Failed to parse metadata: {}", e))?;

    // Cache for future use
    let _ = config.set_entity_cache(&env_name, entities.clone()).await;

    Ok(entities)
}

/// Try to open the field modal if fields are loaded and there's a pending open
fn try_open_pending_field_modal(state: &mut State) -> Command<Msg> {
    // Check if both fields are loaded
    let fields_loaded = matches!(&state.source_fields, Resource::Success(_))
        && matches!(&state.target_fields, Resource::Success(_));

    if !fields_loaded {
        return Command::None;
    }

    // Check if there's a pending quick fields modal
    if state.pending_quick_fields {
        state.pending_quick_fields = false;
        if let Some(entity_idx) = state.quick_fields_entity_idx {
            state.quick_fields_source_prefix = TextInputField::default();
            state.quick_fields_target_prefix = TextInputField::default();
            state.quick_fields_available = state.compute_quick_fields(entity_idx, "", "");
            state.quick_fields_list_state = ListState::with_selection();
            state.show_quick_fields_modal = true;
        }
        return Command::set_focus(FocusId::new("quick-fields-list"));
    }

    // Check if there's a pending modal to open
    let pending = state.pending_field_modal.take();
    if let Some((entity_idx, field_idx_opt)) = pending {
        match field_idx_opt {
            None => {
                // Add new field
                state.show_field_modal = true;
                state.editing_field = Some((entity_idx, usize::MAX));
                state.field_form = FieldMappingForm::default();
                return Command::set_focus(FocusId::new("field-target"));
            }
            Some(field_idx) => {
                // Edit existing field
                if let Resource::Success(config) = &state.config {
                    if let Some(entity) = config.entity_mappings.get(entity_idx) {
                        if let Some(mapping) = entity.field_mappings.get(field_idx) {
                            state.show_field_modal = true;
                            state.editing_field = Some((entity_idx, field_idx));
                            state.field_form = FieldMappingForm::from_mapping(mapping);
                            return Command::set_focus(FocusId::new("field-target"));
                        }
                    }
                }
            }
        }
    }

    Command::None
}

/// Load field metadata for a specific entity (with caching)
async fn load_entity_fields(env_name: String, entity_name: String) -> Result<Vec<FieldMetadata>, String> {
    log::info!("Loading fields for entity '{}' from env '{}'", entity_name, env_name);

    let config = crate::global_config();
    let manager = crate::client_manager();

    // Try cache first (24 hours)
    match config.get_entity_metadata_cache(&env_name, &entity_name, 24).await {
        Ok(Some(cached)) => {
            log::info!("Cache hit: {} fields for '{}'", cached.fields.len(), entity_name);
            return Ok(cached.fields);
        }
        _ => {
            log::info!("Cache miss for '{}'", entity_name);
        }
    }

    // Cache miss - fetch from API
    let client = manager
        .get_client(&env_name)
        .await
        .map_err(|e| {
            log::error!("Failed to get client for {}: {}", env_name, e);
            format!("Failed to get client for {}: {}", env_name, e)
        })?;

    // Use fetch_entity_fields_alt which uses EntityDefinitions API
    // This properly populates related_entity for lookup fields (needed for nested autocomplete)
    let fields = client
        .fetch_entity_fields_alt(&entity_name)
        .await
        .map_err(|e| {
            log::error!("Failed to fetch fields for {}: {}", entity_name, e);
            format!("Failed to fetch fields for {}: {}", entity_name, e)
        })?;

    log::info!("Fetched {} fields for '{}'", fields.len(), entity_name);

    // Cache for future use
    let metadata = crate::api::EntityMetadata {
        fields: fields.clone(),
        ..Default::default()
    };
    let _ = config.set_entity_metadata_cache(&env_name, &entity_name, &metadata).await;

    Ok(fields)
}

/// Check if a field is a virtual/computed field (like accountidname)
/// These fields are not queryable and should be excluded from source field selection
fn is_virtual_field(field: &FieldMetadata) -> bool {
    matches!(&field.field_type, FieldType::Other(t) if t == "Virtual")
}

/// Build the source field options for autocomplete, including nested lookup paths
/// Supports multi-level nesting (e.g., cgk_deadlineid.cgk_projectmanagerid.emailaddress1)
fn get_source_options(state: &State) -> Vec<String> {
    let mut options = Vec::new();

    // Add base fields (excluding virtual/computed fields)
    if let Resource::Success(fields) = &state.source_fields {
        for f in fields {
            if !is_virtual_field(f) {
                options.push(f.logical_name.clone());
            }
        }
    }

    // Add nested paths for loaded related entities (also excluding virtual fields)
    // The key is now the full path prefix (e.g., "cgk_deadlineid" or "cgk_deadlineid.cgk_projectmanagerid")
    for (path_prefix, resource) in &state.related_fields {
        if let Resource::Success(related_fields) = resource {
            for f in related_fields {
                if !is_virtual_field(f) {
                    options.push(format!("{}.{}", path_prefix, f.logical_name));
                }
            }
        }
    }

    options
}

/// Check if the current source path value contains a lookup field reference
/// that requires loading the related entity's fields.
/// Returns a Command to load the related entity if needed.
/// Supports multi-level nesting (e.g., cgk_deadlineid.cgk_projectmanagerid.emailaddress1)
fn check_for_nested_lookup(state: &mut State, current_value: &str) -> Command<Msg> {
    if !current_value.contains('.') {
        return Command::None;
    }

    let segments: Vec<&str> = current_value.split('.').collect();
    log::debug!("check_for_nested_lookup: segments = {:?}", segments);

    // We need to check each level of nesting and load the deepest unloaded one
    // For "a.b.c", we check: "a" (level 0), "a.b" (level 1)
    // The last segment is the field we're trying to access, not a lookup to expand

    for depth in 0..segments.len().saturating_sub(1) {
        let path_prefix = segments[..=depth].join(".");
        let field_to_check = segments[depth];

        // Skip if already loaded for this path prefix
        if state.related_fields.contains_key(&path_prefix) {
            continue;
        }

        // Find the field metadata to check if it's a lookup
        let lookup_info = if depth == 0 {
            // First level: look in source_fields
            if let Resource::Success(fields) = &state.source_fields {
                if let Some(field) = fields.iter().find(|f| f.logical_name == field_to_check) {
                    log::debug!("  Found field '{}' with type {:?}, related_entity: {:?}",
                        field.logical_name, field.field_type, field.related_entity);
                }
                fields.iter()
                    .find(|f| f.logical_name == field_to_check && f.field_type == FieldType::Lookup)
                    .and_then(|f| f.related_entity.clone().map(|e| (path_prefix.clone(), e)))
            } else {
                log::debug!("  source_fields not loaded yet");
                None
            }
        } else {
            // Deeper levels: look in previously loaded related fields
            let parent_prefix = segments[..depth].join(".");
            if let Some(Resource::Success(fields)) = state.related_fields.get(&parent_prefix) {
                fields.iter()
                    .find(|f| f.logical_name == field_to_check && f.field_type == FieldType::Lookup)
                    .and_then(|f| f.related_entity.clone().map(|e| (path_prefix.clone(), e)))
            } else {
                // Parent level not loaded yet, can't proceed deeper
                None
            }
        };

        if let Some((path_prefix, related_entity)) = lookup_info {
            // Get source env name from config
            let source_env = if let Resource::Success(config) = &state.config {
                config.source_env.clone()
            } else {
                return Command::None;
            };

            state.related_fields.insert(path_prefix.clone(), Resource::Loading);

            // Use loading screen for fetch
            return Command::perform_parallel()
                .add_task(
                    format!("Loading fields from {}", related_entity),
                    load_entity_fields(source_env, related_entity),
                )
                .with_title("Loading Related Entity")
                .on_complete(AppId::TransferMappingEditor)
                .build(move |_task_idx, result| {
                    let data = result.downcast::<Result<Vec<FieldMetadata>, String>>().unwrap();
                    Msg::RelatedFieldsLoaded {
                        lookup_field: path_prefix.clone(),
                        result: *data,
                    }
                });
        }
    }

    Command::None
}

/// Check if resolver source entity selection changed and load match fields
fn check_resolver_entity_selection(state: &mut State) -> Command<Msg> {
    let entity_name = state.resolver_form.source_entity.value.trim().to_string();
    if entity_name.is_empty() {
        return Command::None;
    }

    // Only load if the entity matches one from the list (complete selection)
    let is_valid_entity = match &state.target_entities {
        Resource::Success(entities) => entities.contains(&entity_name),
        _ => false,
    };

    if !is_valid_entity {
        return Command::None;
    }

    // Check if fields already loaded for THIS entity (not just any entity)
    if let Some(loaded_entity) = &state.resolver_match_fields_entity {
        if loaded_entity == &entity_name {
            // Already loaded for this entity, don't reload
            return Command::None;
        }
    }

    if let Resource::Success(config) = &state.config {
        let target_env = config.target_env.clone();
        state.resolver_match_fields = Resource::Loading;
        state.resolver_match_fields_entity = Some(entity_name.clone());
        return Command::perform(
            load_entity_fields(target_env, entity_name),
            Msg::ResolverMatchFieldsLoaded,
        );
    }

    Command::None
}

/// Build autocomplete options for resolver source_path field
/// Includes base source entity fields and nested paths for loaded related entities
/// Supports multi-level nesting (e.g., cgk_deadlineid.cgk_projectmanagerid.emailaddress1)
fn build_resolver_source_options(state: &State) -> Vec<String> {
    let mut options = Vec::new();

    // Add base source fields (excluding virtual fields)
    if let Resource::Success(fields) = &state.resolver_source_fields {
        for f in fields {
            if !is_virtual_field(f) {
                options.push(f.logical_name.clone());
            }
        }
    }

    // Add nested paths for loaded related entities
    // The key is now the full path prefix (e.g., "cgk_deadlineid" or "cgk_deadlineid.cgk_projectmanagerid")
    for (path_prefix, resource) in &state.resolver_related_fields {
        if let Resource::Success(related_fields) = resource {
            for f in related_fields {
                if !is_virtual_field(f) {
                    options.push(format!("{}.{}", path_prefix, f.logical_name));
                }
            }
        }
    }

    options
}

/// Check if the current resolver source_path value contains a lookup field reference
/// that requires loading the related entity's fields.
/// Supports multi-level nesting (e.g., cgk_deadlineid.cgk_projectmanagerid.emailaddress1)
fn check_resolver_nested_lookup(state: &mut State, current_value: &str) -> Command<Msg> {
    if !current_value.contains('.') {
        return Command::None;
    }

    let segments: Vec<&str> = current_value.split('.').collect();

    // We need to check each level of nesting and load the deepest unloaded one
    // For "a.b.c", we check: "a" (level 0), "a.b" (level 1)
    // The last segment is the field we're trying to access, not a lookup to expand

    for depth in 0..segments.len().saturating_sub(1) {
        let path_prefix = segments[..=depth].join(".");
        let field_to_check = segments[depth];

        // Skip if already loaded for this path prefix
        if state.resolver_related_fields.contains_key(&path_prefix) {
            continue;
        }

        // Find the field metadata to check if it's a lookup
        let lookup_info = if depth == 0 {
            // First level: look in resolver_source_fields
            if let Resource::Success(fields) = &state.resolver_source_fields {
                fields.iter()
                    .find(|f| f.logical_name == field_to_check && f.field_type == FieldType::Lookup)
                    .and_then(|f| f.related_entity.clone().map(|e| (path_prefix.clone(), e)))
            } else {
                None
            }
        } else {
            // Deeper levels: look in previously loaded related fields
            let parent_prefix = segments[..depth].join(".");
            if let Some(Resource::Success(fields)) = state.resolver_related_fields.get(&parent_prefix) {
                fields.iter()
                    .find(|f| f.logical_name == field_to_check && f.field_type == FieldType::Lookup)
                    .and_then(|f| f.related_entity.clone().map(|e| (path_prefix.clone(), e)))
            } else {
                // Parent level not loaded yet, can't proceed deeper
                None
            }
        };

        if let Some((path_prefix, related_entity)) = lookup_info {
            // Get source env name from config (resolver source_path is from the transfer source entity)
            let source_env = if let Resource::Success(config) = &state.config {
                config.source_env.clone()
            } else {
                return Command::None;
            };

            state.resolver_related_fields.insert(path_prefix.clone(), Resource::Loading);

            // Use loading screen for fetch
            return Command::perform_parallel()
                .add_task(
                    format!("Loading fields from {}", related_entity),
                    load_entity_fields(source_env, related_entity),
                )
                .with_title("Loading Related Entity")
                .on_complete(AppId::TransferMappingEditor)
                .build(move |_task_idx, result| {
                    let data = result.downcast::<Result<Vec<FieldMetadata>, String>>().unwrap();
                    Msg::ResolverRelatedFieldsLoaded {
                        lookup_field: path_prefix.clone(),
                        result: *data,
                    }
                });
        }
    }

    Command::None
}
