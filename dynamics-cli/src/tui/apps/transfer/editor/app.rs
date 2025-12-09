use crate::config::repository::transfer::{get_transfer_config, save_transfer_config};
use crate::transfer::TransferConfig;
use crate::tui::element::FocusId;
use crate::tui::resource::Resource;
use crate::tui::widgets::TreeState;
use crate::tui::{App, AppId, Command, LayeredView, Subscription};

use crate::api::FieldMetadata;
use super::state::{DeleteTarget, EditorParams, EntityMappingForm, FieldMappingForm, Msg, State, TransformType};
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
            dirty: false,
            source_entities: Resource::Loading,
            target_entities: Resource::Loading,
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
                        Command::perform_parallel()
                            .add_task(
                                format!("Loading entities from {}", source_env),
                                load_entities_for_env(source_env),
                            )
                            .add_task(
                                format!("Loading entities from {}", target_env),
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
                    Ok(fields) => Resource::Success(fields),
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

            // Entity modal
            Msg::AddEntity => {
                state.show_entity_modal = true;
                state.editing_entity_idx = None;
                state.entity_form = EntityMappingForm::default();
                state.entity_form.priority.value = next_priority(state).to_string();
                Command::set_focus(FocusId::new("entity-source"))
            }

            Msg::EditEntity(idx) => {
                if let Resource::Success(config) = &state.config {
                    if let Some(mapping) = config.entity_mappings.get(idx) {
                        state.show_entity_modal = true;
                        state.editing_entity_idx = Some(idx);
                        state.entity_form = EntityMappingForm::from_mapping(mapping);
                        return Command::set_focus(FocusId::new("entity-source"));
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
                        // Editing: preserve field mappings
                        if let Some(existing) = config.entity_mappings.get(idx) {
                            new_mapping.field_mappings = existing.field_mappings.clone();
                        }
                        config.entity_mappings[idx] = new_mapping;
                    } else {
                        // Adding new
                        config.entity_mappings.push(new_mapping);
                    }

                    state.dirty = true;
                    state.tree_state.invalidate_cache();
                }

                state.show_entity_modal = false;
                state.editing_entity_idx = None;
                Command::set_focus(FocusId::new("mapping-tree"))
            }

            Msg::EntityFormSource(event) => {
                let options = match &state.source_entities {
                    Resource::Success(entities) => entities.clone(),
                    _ => vec![],
                };
                state.entity_form.source_entity.handle_event::<Msg>(event, &options);
                Command::None
            }

            Msg::EntityFormTarget(event) => {
                let options = match &state.target_entities {
                    Resource::Success(entities) => entities.clone(),
                    _ => vec![],
                };
                state.entity_form.target_entity.handle_event::<Msg>(event, &options);
                Command::None
            }

            Msg::EntityFormPriority(event) => {
                state.entity_form.priority.handle_event(event, Some(10));
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
                    return Command::perform_parallel()
                        .add_task(
                            format!("Loading fields from {}", source_entity),
                            load_entity_fields(source_env.clone(), source_entity),
                        )
                        .add_task(
                            format!("Loading fields from {}", target_entity),
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
                    return Command::perform_parallel()
                        .add_task(
                            format!("Loading fields from {}", source_entity),
                            load_entity_fields(source_env.clone(), source_entity),
                        )
                        .add_task(
                            format!("Loading fields from {}", target_entity),
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

            Msg::CloseFieldModal => {
                state.show_field_modal = false;
                state.editing_field = None;
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
                                state.dirty = true;
                                state.tree_state.invalidate_cache();
                            }
                        }
                    }
                }

                state.show_field_modal = false;
                state.editing_field = None;
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
                let options: Vec<String> = match &state.source_fields {
                    Resource::Success(fields) => fields.iter().map(|f| f.logical_name.clone()).collect(),
                    _ => vec![],
                };
                state.field_form.source_path.handle_event::<Msg>(event, &options);
                Command::None
            }

            Msg::FieldFormConstant(event) => {
                state.field_form.constant_value.handle_event(event, Some(500));
                Command::None
            }

            Msg::FieldFormToggleType => {
                state.field_form.transform_type = match state.field_form.transform_type {
                    TransformType::Copy => TransformType::Constant,
                    TransformType::Constant => TransformType::Copy,
                };
                Command::None
            }

            // Delete confirmation
            Msg::ConfirmDelete => {
                if let Some(target) = state.delete_target.take() {
                    if let Resource::Success(config) = &mut state.config {
                        match target {
                            DeleteTarget::Entity(idx) => {
                                if idx < config.entity_mappings.len() {
                                    config.entity_mappings.remove(idx);
                                    state.dirty = true;
                                    state.tree_state.invalidate_cache();
                                }
                            }
                            DeleteTarget::Field(entity_idx, field_idx) => {
                                if let Some(entity) = config.entity_mappings.get_mut(entity_idx) {
                                    if field_idx < entity.field_mappings.len() {
                                        entity.field_mappings.remove(field_idx);
                                        state.dirty = true;
                                        state.tree_state.invalidate_cache();
                                    }
                                }
                            }
                        }
                    }
                }
                state.show_delete_confirm = false;
                Command::None
            }

            Msg::CancelDelete => {
                state.show_delete_confirm = false;
                state.delete_target = None;
                Command::None
            }

            // Save
            Msg::Save => {
                if let Resource::Success(config) = &state.config {
                    let config_clone = config.clone();
                    return Command::perform(save_config(config_clone), Msg::SaveCompleted);
                }
                Command::None
            }

            Msg::SaveCompleted(result) => {
                match result {
                    Ok(()) => {
                        state.dirty = false;
                    }
                    Err(_e) => {
                        // TODO: Show error modal
                    }
                }
                Command::None
            }

            // Navigation
            Msg::Back => {
                Command::navigate_to(AppId::TransferConfigList)
            }

            Msg::Preview => {
                // TODO: Navigate to preview app
                log::info!("Would navigate to preview for: {}", state.config_name);
                Command::None
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

    let fields = client
        .fetch_entity_fields(&entity_name)
        .await
        .map_err(|e| {
            log::error!("Failed to fetch fields for {}: {}", entity_name, e);
            format!("Failed to fetch fields for {}: {}", entity_name, e)
        })?;

    log::info!("Fetched {} fields for '{}'", fields.len(), entity_name);

    // Cache for future use
    let metadata = crate::api::EntityMetadata {
        fields: fields.clone(),
        relationships: vec![],
        views: vec![],
        forms: vec![],
    };
    let _ = config.set_entity_metadata_cache(&env_name, &entity_name, &metadata).await;

    Ok(fields)
}
