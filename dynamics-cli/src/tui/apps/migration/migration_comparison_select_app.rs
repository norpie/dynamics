use crate::tui::{
    app::App,
    command::{AppId, Command},
    element::{ColumnBuilder, Element, FocusId, LayoutConstraint},
    subscription::Subscription,
    state::theme::Theme,
    widgets::list::{ListItem, ListState},
    widgets::{AutocompleteField, AutocompleteEvent, TextInputField, TextInputEvent, FileBrowserEvent},
    widgets::file_browser::{FileBrowserState, FileBrowserAction},
    renderer::LayeredView,
    Resource,
};
use dynamics_lib_macros::Validate;
use crate::config::repository::migrations::SavedComparison;
use crossterm::event::KeyCode;
use ratatui::{
    prelude::Stylize,
    style::Style,
    text::{Line, Span},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::{HashMap, HashSet};
use crate::{col, row, spacer, button_row, use_constraints, error_display};

pub struct MigrationComparisonSelectApp;

#[derive(Clone, Default, Validate)]
pub struct CreateComparisonForm {
    #[validate(not_empty, message = "Comparison name is required")]
    name: TextInputField,

    #[validate(not_empty, message = "Source entity is required")]
    source_entity: AutocompleteField,

    #[validate(not_empty, message = "Target entity is required")]
    target_entity: AutocompleteField,

    validation_error: Option<String>,
}

#[derive(Clone, Default, Validate)]
pub struct RenameComparisonForm {
    #[validate(not_empty, message = "Comparison name is required")]
    new_name: TextInputField,
}

#[derive(Clone, Default, Validate)]
pub struct ImportComparisonForm {
    #[validate(not_empty, message = "Comparison name is required")]
    name: TextInputField,

    validation_error: Option<String>,
}

/// Export/Import JSON structure
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComparisonExportData {
    pub version: String,
    pub export_date: String,
    pub source_entity: String,
    pub target_entity: String,
    pub field_mappings: HashMap<String, Vec<String>>,
    pub prefix_mappings: HashMap<String, Vec<String>>,
    pub imported_mappings: HashMap<String, Vec<String>>,
    pub import_source_file: Option<String>,
    pub ignored_items: Vec<String>,
    pub example_pairs: Vec<ExamplePairExport>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExamplePairExport {
    pub source_record_id: String,
    pub target_record_id: String,
    pub label: Option<String>,
}

#[derive(Clone)]
pub struct State {
    migration_name: Option<String>,
    source_env: Option<String>,
    target_env: Option<String>,
    comparisons: Vec<SavedComparison>,
    list_state: ListState,
    source_entities: Resource<Vec<String>>,
    target_entities: Resource<Vec<String>>,
    show_create_modal: bool,
    create_form: CreateComparisonForm,
    show_delete_confirm: bool,
    delete_comparison_id: Option<i64>,
    delete_comparison_name: Option<String>,
    show_rename_modal: bool,
    rename_comparison_id: Option<i64>,
    rename_form: RenameComparisonForm,
    // Export state
    show_export_modal: bool,
    export_browser: FileBrowserState,
    export_filename: TextInputField,
    export_comparison_id: Option<i64>,
    export_comparison_name: Option<String>,
    // Import state
    show_import_browser: bool,
    show_import_config: bool,
    import_browser: FileBrowserState,
    import_form: ImportComparisonForm,
    import_file_path: Option<PathBuf>,
    // Batch export state
    show_batch_export_modal: bool,
    batch_export_browser: FileBrowserState,
    batch_export_filename: TextInputField,
}

impl Default for State {
    fn default() -> Self {
        let home_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        Self {
            migration_name: None,
            source_env: None,
            target_env: None,
            comparisons: Vec::new(),
            list_state: ListState::default(),
            source_entities: Resource::default(),
            target_entities: Resource::default(),
            show_create_modal: false,
            create_form: CreateComparisonForm::default(),
            show_delete_confirm: false,
            delete_comparison_id: None,
            delete_comparison_name: None,
            show_rename_modal: false,
            rename_comparison_id: None,
            rename_form: RenameComparisonForm::default(),
            show_export_modal: false,
            export_browser: FileBrowserState::new(home_dir.clone()),
            export_filename: TextInputField::default(),
            export_comparison_id: None,
            export_comparison_name: None,
            show_import_browser: false,
            show_import_config: false,
            import_browser: FileBrowserState::new(home_dir.clone()),
            import_form: ImportComparisonForm::default(),
            import_file_path: None,
            show_batch_export_modal: false,
            batch_export_browser: FileBrowserState::new(home_dir.clone()),
            batch_export_filename: TextInputField::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntitiesLoadedData {
    pub source_entities: Vec<String>,
    pub target_entities: Vec<String>,
}

#[derive(Clone, serde::Deserialize)]
pub struct MigrationMetadata {
    pub migration_name: String,
    pub source_env: String,
    pub target_env: String,
}

#[derive(Clone)]
pub enum Msg {
    ParallelDataLoaded(usize, Result<Vec<String>, String>),
    ComparisonsLoaded(Result<Vec<SavedComparison>, String>),
    ListNavigate(KeyCode),
    SelectComparison,
    CreateComparison,
    CreateFormNameEvent(TextInputEvent),
    CreateFormSourceEvent(AutocompleteEvent),
    CreateFormTargetEvent(AutocompleteEvent),
    CreateFormSubmit,
    CreateFormCancel,
    ComparisonCreated(Result<i64, String>),
    RequestDelete,
    ConfirmDelete,
    CancelDelete,
    ComparisonDeleted(Result<(), String>),
    RequestRename,
    RenameFormNameEvent(TextInputEvent),
    RenameFormSubmit,
    RenameFormCancel,
    ComparisonRenamed(Result<(), String>),
    PreloadAllComparisons,
    PreloadTaskComplete, // Ignore individual preload task results
    // Export messages
    RequestExport,
    ExportBrowseNavigate(KeyCode),
    ExportDirectoryEntered(PathBuf),
    ExportFilenameEvent(TextInputEvent),
    ExportConfirm,
    ExportCancel,
    ExportComplete(Result<(), String>),
    // Import messages
    RequestImport,
    ImportBrowseNavigate(KeyCode),
    ImportFileSelected(PathBuf),
    ImportFormNameEvent(TextInputEvent),
    ImportFormSubmit,
    ImportFormCancel,
    ImportComplete(Result<i64, String>),
    // Batch export messages
    RequestBatchExport,
    BatchExportBrowseNavigate(KeyCode),
    BatchExportDirectoryEntered(PathBuf),
    BatchExportFilenameEvent(TextInputEvent),
    BatchExportConfirm,
    BatchExportCancel,
    BatchExportComplete(Result<(), String>),
    Back,
}

impl ListItem for SavedComparison {
    type Msg = Msg;

    fn to_element(&self, is_selected: bool, _is_hovered: bool) -> Element<Self::Msg> {
        let theme = &crate::global_runtime_config().theme;
        let (fg_color, bg_style) = if is_selected {
            (theme.accent_primary, Some(Style::default().bg(theme.bg_surface)))
        } else {
            (theme.text_primary, None)
        };

        let mut builder = Element::styled_text(Line::from(vec![
            Span::styled(
                format!("  {} ({} -> {})", self.name, self.source_entity, self.target_entity),
                Style::default().fg(fg_color),
            ),
        ]));

        if let Some(bg) = bg_style {
            builder = builder.background(bg);
        }

        builder.build()
    }
}

impl crate::tui::AppState for State {}

impl State {
    fn open_delete_modal(&mut self, comparison_id: i64, comparison_name: String) {
        self.delete_comparison_id = Some(comparison_id);
        self.delete_comparison_name = Some(comparison_name);
        self.show_delete_confirm = true;
    }

    fn close_delete_modal(&mut self) {
        self.show_delete_confirm = false;
        self.delete_comparison_id = None;
        self.delete_comparison_name = None;
    }

    fn open_rename_modal(&mut self, comparison_id: i64, comparison_name: String) {
        self.rename_comparison_id = Some(comparison_id);
        self.rename_form.new_name.set_value(comparison_name);
        self.show_rename_modal = true;
    }

    fn close_rename_modal(&mut self) {
        self.show_rename_modal = false;
        self.rename_comparison_id = None;
        self.rename_form = RenameComparisonForm::default();
    }

    fn close_create_modal(&mut self) {
        self.show_create_modal = false;
        self.create_form.validation_error = None;
    }

    fn open_export_modal(&mut self, comparison_id: i64, comparison_name: String) {
        self.export_comparison_id = Some(comparison_id);
        self.export_comparison_name = Some(comparison_name.clone());

        // Initialize file browser to home directory
        let home_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        self.export_browser = FileBrowserState::new(home_dir);

        // Set filter to show only directories
        self.export_browser.set_filter(|entry| entry.is_dir);

        // Set default filename
        let default_filename = format!("{}.json", comparison_name);
        self.export_filename.set_value(default_filename);

        self.show_export_modal = true;
    }

    fn close_export_modal(&mut self) {
        self.show_export_modal = false;
        self.export_comparison_id = None;
        self.export_comparison_name = None;
        self.export_filename = TextInputField::default();
    }

    fn open_import_browser(&mut self) {
        let home_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        self.import_browser = FileBrowserState::new(home_dir);

        // Set filter to show .json files and directories
        self.import_browser.set_filter(|entry| {
            entry.is_dir || entry.name.ends_with(".json")
        });

        self.show_import_browser = true;
    }

    fn close_import_browser(&mut self) {
        self.show_import_browser = false;
    }

    fn open_import_config(&mut self, file_path: PathBuf) {
        self.import_file_path = Some(file_path.clone());

        // Extract filename without extension as default name
        let default_name = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported Comparison")
            .to_string();

        self.import_form = ImportComparisonForm::default();
        self.import_form.name.set_value(default_name);

        self.show_import_browser = false;
        self.show_import_config = true;
    }

    fn close_import_config(&mut self) {
        self.show_import_config = false;
        self.import_file_path = None;
        self.import_form = ImportComparisonForm::default();
    }

    fn open_batch_export_modal(&mut self) {
        // Initialize file browser to home directory
        let home_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        self.batch_export_browser = FileBrowserState::new(home_dir);

        // Set filter to show only directories
        self.batch_export_browser.set_filter(|entry| entry.is_dir);

        // Set default filename with timestamp
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let migration_name = self.migration_name.as_deref().unwrap_or("migration");
        let default_filename = format!("{}_{}_mappings.xlsx", migration_name, timestamp);
        self.batch_export_filename.set_value(default_filename);

        self.show_batch_export_modal = true;
    }

    fn close_batch_export_modal(&mut self) {
        self.show_batch_export_modal = false;
        self.batch_export_filename = TextInputField::default();
    }
}

pub struct MigrationSelectParams {
    pub migration_name: String,
    pub source_env: String,
    pub target_env: String,
}

impl Default for MigrationSelectParams {
    fn default() -> Self {
        Self {
            migration_name: String::new(),
            source_env: String::new(),
            target_env: String::new(),
        }
    }
}

impl App for MigrationComparisonSelectApp {
    type State = State;
    type Msg = Msg;
    type InitParams = MigrationSelectParams;

    fn init(params: MigrationSelectParams) -> (State, Command<Msg>) {
        let mut state = State::default();
        state.migration_name = Some(params.migration_name.clone());
        state.source_env = Some(params.source_env.clone());
        state.target_env = Some(params.target_env.clone());
        state.source_entities = crate::tui::Resource::Loading;
        state.target_entities = crate::tui::Resource::Loading;

        // Load entities in parallel with automatic LoadingScreen
        let cmd = Command::perform_parallel()
            .add_task(
                format!("Loading source entities ({})", params.source_env),
                {
                    let source_env = params.source_env.clone();
                    async move {
                        use crate::api::metadata::parse_entity_list;
                        let config = crate::global_config();
                        let manager = crate::client_manager();

                        match config.get_entity_cache(&source_env, 24).await {
                            Ok(Some(cached)) => Ok::<Vec<String>, String>(cached),
                            _ => {
                                let client = manager.get_client(&source_env).await.map_err(|e| e.to_string())?;
                                let metadata_xml = client.fetch_metadata().await.map_err(|e| e.to_string())?;
                                let entities = parse_entity_list(&metadata_xml).map_err(|e| e.to_string())?;
                                let _ = config.set_entity_cache(&source_env, entities.clone()).await;
                                Ok(entities)
                            }
                        }
                    }
                }
            )
            .add_task(
                format!("Loading target entities ({})", params.target_env),
                {
                    let target_env = params.target_env.clone();
                    async move {
                        use crate::api::metadata::parse_entity_list;
                        let config = crate::global_config();
                        let manager = crate::client_manager();

                        match config.get_entity_cache(&target_env, 24).await {
                            Ok(Some(cached)) => Ok::<Vec<String>, String>(cached),
                            _ => {
                                let client = manager.get_client(&target_env).await.map_err(|e| e.to_string())?;
                                let metadata_xml = client.fetch_metadata().await.map_err(|e| e.to_string())?;
                                let entities = parse_entity_list(&metadata_xml).map_err(|e| e.to_string())?;
                                let _ = config.set_entity_cache(&target_env, entities.clone()).await;
                                Ok(entities)
                            }
                        }
                    }
                }
            )
            .with_title("Loading Migration Data")
            .on_complete(AppId::MigrationComparisonSelect)
            .build(|task_idx, result| {
                let data = result.downcast::<Result<Vec<String>, String>>().unwrap();
                Msg::ParallelDataLoaded(task_idx, *data)
            });

        (state, cmd)
    }

    fn update(state: &mut Self::State, msg: Self::Msg) -> Command<Self::Msg> {
        log::debug!("MigrationComparisonSelectApp::update() called with message");
        match msg {
            Msg::ParallelDataLoaded(task_idx, result) => {
                // Store result in appropriate Resource
                match task_idx {
                    0 => {
                        if let Err(ref e) = result {
                            log::error!("Failed to load source entities: {}", e);
                        }
                        state.source_entities = Resource::from_result(result);
                    }
                    1 => {
                        if let Err(ref e) = result {
                            log::error!("Failed to load target entities: {}", e);
                        }
                        state.target_entities = Resource::from_result(result);
                    }
                    _ => {}
                }

                // Load comparisons when both entities are loaded
                if let (Resource::Success(_), Resource::Success(_)) =
                    (&state.source_entities, &state.target_entities)
                {
                    let migration_name = state.migration_name.clone().unwrap();
                    Command::perform(
                        async move {
                            let config = crate::global_config();
                            config.get_comparisons(&migration_name).await.map_err(|e| e.to_string())
                        },
                        Msg::ComparisonsLoaded,
                    )
                } else {
                    Command::None
                }
            }
            Msg::ComparisonsLoaded(result) => {
                match result {
                    Ok(comparisons) => {
                        state.comparisons = comparisons;
                        state.list_state = ListState::new();
                        let item_count = state.comparisons.len();
                        if !state.comparisons.is_empty() {
                            state.list_state.select_and_scroll(Some(0), item_count);
                            // Focus the list after loading comparisons
                            return Command::set_focus(FocusId::new("comparison-list"));
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to load comparisons: {}", e);
                    }
                }
                Command::None
            }
            Msg::ListNavigate(key) => {
                let visible_height = 20;
                state.list_state.handle_key(key, state.comparisons.len(), visible_height);
                Command::None
            }
            Msg::SelectComparison => {
                log::info!("SelectComparison triggered - list size: {}, selected: {:?}",
                    state.comparisons.len(), state.list_state.selected());
                if let Some(selected_idx) = state.list_state.selected() {
                    if let Some(comparison) = state.comparisons.get(selected_idx) {
                        log::info!("Opening comparison: {:?} -> {:?}",
                            comparison.source_entities, comparison.target_entities);
                        let params = super::entity_comparison::EntityComparisonParams {
                            migration_name: state.migration_name.clone().unwrap_or_default(),
                            source_env: state.source_env.clone().unwrap_or_default(),
                            target_env: state.target_env.clone().unwrap_or_default(),
                            source_entities: comparison.source_entities.clone(),
                            target_entities: comparison.target_entities.clone(),
                        };
                        return Command::batch(vec![
                            Command::start_app(AppId::EntityComparison, params),
                            Command::quit_self(),
                        ]);
                    } else {
                        log::warn!("Selected index {} out of bounds", selected_idx);
                    }
                } else {
                    log::warn!("No comparison selected");
                }
                Command::None
            }
            Msg::CreateComparison => {
                state.show_create_modal = true;
                state.create_form = CreateComparisonForm::default();
                Command::set_focus(FocusId::new("create-name-input"))
            }
            Msg::CreateFormNameEvent(event) => {
                state.create_form.name.handle_event(event, Some(50));
                Command::None
            }
            Msg::CreateFormSourceEvent(event) => {
                let options = state.source_entities.as_ref().ok().cloned().unwrap_or_default();
                state.create_form.source_entity.handle_event::<Msg>(event, &options);
                Command::None
            }
            Msg::CreateFormTargetEvent(event) => {
                let options = state.target_entities.as_ref().ok().cloned().unwrap_or_default();
                state.create_form.target_entity.handle_event::<Msg>(event, &options);
                Command::None
            }
            Msg::CreateFormSubmit => {
                // Validate using generated macro method
                match state.create_form.validate() {
                    Ok(_) => {
                        let name = state.create_form.name.value().trim().to_string();
                        let source_entity = state.create_form.source_entity.value().trim().to_string();
                        let target_entity = state.create_form.target_entity.value().trim().to_string();

                        // Additional validation: check entities exist in lists
                        if let Resource::Success(source_list) = &state.source_entities {
                            if !source_list.contains(&source_entity) {
                                state.create_form.validation_error = Some(format!("Source entity '{}' not found", source_entity));
                                return Command::None;
                            }
                        }

                        if let Resource::Success(target_list) = &state.target_entities {
                            if !target_list.contains(&target_entity) {
                                state.create_form.validation_error = Some(format!("Target entity '{}' not found", target_entity));
                                return Command::None;
                            }
                        }

                        let migration_name = state.migration_name.clone().unwrap_or_default();
                        state.show_create_modal = false;
                        state.create_form.validation_error = None;

                        Command::perform(
                            async move {
                                let config = crate::global_config();
                                let comparison = SavedComparison {
                                    id: 0, // Will be assigned by database
                                    name,
                                    migration_name,
                                    source_entity: source_entity.clone(),
                                    target_entity: target_entity.clone(),
                                    source_entities: vec![source_entity],  // Single entity for now
                                    target_entities: vec![target_entity],  // Single entity for now
                                    entity_comparison: None,
                                    created_at: chrono::Utc::now(),
                                    last_used: chrono::Utc::now(),
                                };
                                config.add_comparison(comparison).await
                                    .map_err(|e| e.to_string())
                            },
                            Msg::ComparisonCreated
                        )
                    }
                    Err(validation_error) => {
                        state.create_form.validation_error = Some(validation_error);
                        Command::None
                    }
                }
            }
            Msg::CreateFormCancel => {
                state.close_create_modal();
                Command::None
            }
            Msg::ComparisonCreated(result) => {
                match result {
                    Ok(id) => {
                        log::info!("Created comparison with ID: {}", id);
                        let migration_name = state.migration_name.clone().unwrap_or_default();
                        reload_comparisons(migration_name)
                    }
                    Err(e) => {
                        log::error!("Failed to create comparison: {}", e);
                        Command::None
                    }
                }
            }
            Msg::RequestDelete => {
                if let Some(selected_idx) = state.list_state.selected() {
                    if let Some(comparison) = state.comparisons.get(selected_idx) {
                        state.open_delete_modal(comparison.id, comparison.name.clone());
                    }
                }
                Command::None
            }
            Msg::ConfirmDelete => {
                if let Some(id) = state.delete_comparison_id {
                    state.show_delete_confirm = false;
                    // Async delete from database
                    Command::perform(
                        async move {
                            let config = crate::global_config();
                            config.delete_comparison(id).await.map_err(|e| e.to_string())
                        },
                        Msg::ComparisonDeleted
                    )
                } else {
                    Command::None
                }
            }
            Msg::CancelDelete => {
                state.close_delete_modal();
                Command::None
            }
            Msg::ComparisonDeleted(result) => {
                match result {
                    Ok(_) => {
                        state.close_delete_modal();
                        let migration_name = state.migration_name.clone().unwrap_or_default();
                        reload_comparisons(migration_name)
                    }
                    Err(e) => {
                        log::error!("Failed to delete comparison: {}", e);
                        Command::None
                    }
                }
            }
            Msg::RequestRename => {
                if let Some(selected_idx) = state.list_state.selected() {
                    if let Some(comparison) = state.comparisons.get(selected_idx) {
                        state.open_rename_modal(comparison.id, comparison.name.clone());
                    }
                }
                Command::None
            }
            Msg::RenameFormNameEvent(event) => {
                state.rename_form.new_name.handle_event(event, Some(50));
                Command::None
            }
            Msg::RenameFormSubmit => {
                let id = state.rename_comparison_id;
                let new_name = state.rename_form.new_name.value().trim().to_string();

                if new_name.is_empty() || id.is_none() {
                    return Command::None;
                }

                state.show_rename_modal = false;
                let id = id.unwrap();

                Command::perform(
                    async move {
                        let config = crate::global_config();
                        config.rename_comparison(id, &new_name).await
                            .map_err(|e| e.to_string())
                    },
                    Msg::ComparisonRenamed
                )
            }
            Msg::RenameFormCancel => {
                state.close_rename_modal();
                Command::None
            }
            Msg::ComparisonRenamed(result) => {
                match result {
                    Ok(_) => {
                        state.close_rename_modal();
                        let migration_name = state.migration_name.clone().unwrap_or_default();
                        reload_comparisons(migration_name)
                    }
                    Err(e) => {
                        log::error!("Failed to rename comparison: {}", e);
                        Command::None
                    }
                }
            }
            Msg::PreloadAllComparisons => {
                if state.comparisons.is_empty() {
                    return Command::None;
                }

                let source_env = state.source_env.clone().unwrap_or_default();
                let target_env = state.target_env.clone().unwrap_or_default();

                // Build parallel command with all comparison fetches
                let mut builder = Command::perform_parallel();

                for comparison in &state.comparisons {
                    let source_entity = comparison.source_entity.clone();
                    let target_entity = comparison.target_entity.clone();

                    // Add 6 metadata tasks per comparison
                    builder = builder
                        .add_task(
                            format!("Loading {} fields ({})", source_entity, source_env),
                            {
                                let env = source_env.clone();
                                let entity = source_entity.clone();
                                async move {
                                    use crate::tui::apps::migration::entity_comparison::{FetchType, fetch_with_cache};
                                    fetch_with_cache(&env, &entity, FetchType::SourceFields, true).await
                                }
                            }
                        )
                        .add_task(
                            format!("Loading {} forms ({})", source_entity, source_env),
                            {
                                let env = source_env.clone();
                                let entity = source_entity.clone();
                                async move {
                                    use crate::tui::apps::migration::entity_comparison::{FetchType, fetch_with_cache};
                                    fetch_with_cache(&env, &entity, FetchType::SourceForms, true).await
                                }
                            }
                        )
                        .add_task(
                            format!("Loading {} views ({})", source_entity, source_env),
                            {
                                let env = source_env.clone();
                                let entity = source_entity.clone();
                                async move {
                                    use crate::tui::apps::migration::entity_comparison::{FetchType, fetch_with_cache};
                                    fetch_with_cache(&env, &entity, FetchType::SourceViews, true).await
                                }
                            }
                        )
                        .add_task(
                            format!("Loading {} fields ({})", target_entity, target_env),
                            {
                                let env = target_env.clone();
                                let entity = target_entity.clone();
                                async move {
                                    use crate::tui::apps::migration::entity_comparison::{FetchType, fetch_with_cache};
                                    fetch_with_cache(&env, &entity, FetchType::TargetFields, true).await
                                }
                            }
                        )
                        .add_task(
                            format!("Loading {} forms ({})", target_entity, target_env),
                            {
                                let env = target_env.clone();
                                let entity = target_entity.clone();
                                async move {
                                    use crate::tui::apps::migration::entity_comparison::{FetchType, fetch_with_cache};
                                    fetch_with_cache(&env, &entity, FetchType::TargetForms, true).await
                                }
                            }
                        )
                        .add_task(
                            format!("Loading {} views ({})", target_entity, target_env),
                            {
                                let env = target_env.clone();
                                let entity = target_entity.clone();
                                async move {
                                    use crate::tui::apps::migration::entity_comparison::{FetchType, fetch_with_cache};
                                    fetch_with_cache(&env, &entity, FetchType::TargetViews, true).await
                                }
                            }
                        );

                    // Add example pair fetch tasks
                    let source_entity_for_examples = source_entity.clone();
                    let target_entity_for_examples = target_entity.clone();
                    let source_env_for_examples = source_env.clone();
                    let target_env_for_examples = target_env.clone();

                    builder = builder.add_task(
                        format!("Loading example pairs for {} -> {}", source_entity, target_entity),
                        async move {
                            let config = crate::global_config();
                            let example_pairs = config.get_example_pairs(&source_entity_for_examples, &target_entity_for_examples)
                                .await
                                .unwrap_or_default();

                            // Fetch all example records
                            for pair in example_pairs {
                                let _ = crate::tui::apps::migration::entity_comparison::fetch_example_pair_data(
                                    &source_env_for_examples,
                                    &source_entity_for_examples,
                                    &pair.source_record_id,
                                    &target_env_for_examples,
                                    &target_entity_for_examples,
                                    &pair.target_record_id,
                                ).await;
                            }

                            Ok::<(), String>(())
                        }
                    );
                }

                builder
                    .with_title("Preloading All Comparisons")
                    .on_complete(AppId::MigrationComparisonSelect)
                    .on_cancel(AppId::MigrationComparisonSelect)
                    .cancellable(true)
                    .build(|_task_idx, _result| {
                        // Ignore individual task results, cache already populated
                        Msg::PreloadTaskComplete
                    })
            }
            Msg::PreloadTaskComplete => {
                // No-op: preload tasks complete, cache already populated
                Command::None
            }
            // Export handlers
            Msg::RequestExport => {
                if let Some(selected_idx) = state.list_state.selected() {
                    if let Some(comparison) = state.comparisons.get(selected_idx) {
                        state.open_export_modal(comparison.id, comparison.name.clone());
                        return Command::set_focus(FocusId::new("export-file-browser"));
                    }
                }
                Command::None
            }
            Msg::ExportBrowseNavigate(key) => {
                match key {
                    KeyCode::Enter => {
                        // Handle Enter key specially - activate the selected item
                        if let Some(action) = state.export_browser.handle_event(FileBrowserEvent::Activate) {
                            match action {
                                FileBrowserAction::DirectoryEntered(path) => {
                                    // Enter the directory
                                    let _ = state.export_browser.set_path(path);
                                }
                                _ => {}
                            }
                        }
                        Command::None
                    }
                    _ => {
                        // Handle other navigation keys normally
                        state.export_browser.handle_navigation_key(key);
                        Command::None
                    }
                }
            }
            Msg::ExportDirectoryEntered(_path) => {
                // No longer used since we handle Enter in Navigate
                Command::None
            }
            Msg::ExportFilenameEvent(event) => {
                state.export_filename.handle_event(event, Some(255));
                Command::None
            }
            Msg::ExportConfirm => {
                if let (Some(id), Some(_name)) = (state.export_comparison_id, &state.export_comparison_name) {
                    let directory = state.export_browser.current_path().to_path_buf();
                    let filename = state.export_filename.value().trim().to_string();

                    if filename.is_empty() {
                        return Command::None;
                    }

                    let file_path = directory.join(&filename);

                    // Close modal immediately
                    state.close_export_modal();

                    // Perform export asynchronously
                    Command::perform(
                        async move {
                            export_comparison(id, file_path).await
                        },
                        Msg::ExportComplete
                    )
                } else {
                    Command::None
                }
            }
            Msg::ExportCancel => {
                state.close_export_modal();
                Command::None
            }
            Msg::ExportComplete(result) => {
                match result {
                    Ok(()) => {
                        log::info!("Comparison exported successfully");
                        // TODO: Show success message to user
                    }
                    Err(e) => {
                        log::error!("Failed to export comparison: {}", e);
                        // TODO: Show error modal
                    }
                }
                Command::None
            }
            // Import handlers
            Msg::RequestImport => {
                state.open_import_browser();
                Command::set_focus(FocusId::new("import-file-browser"))
            }
            Msg::ImportBrowseNavigate(key) => {
                match key {
                    KeyCode::Enter => {
                        // Handle Enter key specially - activate the selected item
                        if let Some(action) = state.import_browser.handle_event(FileBrowserEvent::Activate) {
                            match action {
                                FileBrowserAction::FileSelected(path) => {
                                    // File selected, open import config
                                    state.open_import_config(path);
                                    return Command::set_focus(FocusId::new("import-name-input"));
                                }
                                FileBrowserAction::DirectoryEntered(path) => {
                                    // Enter the directory
                                    let _ = state.import_browser.set_path(path);
                                }
                                _ => {}
                            }
                        }
                        Command::None
                    }
                    _ => {
                        // Handle other navigation keys normally
                        state.import_browser.handle_navigation_key(key);
                        Command::None
                    }
                }
            }
            Msg::ImportFileSelected(file_path) => {
                // No longer used since we handle Enter in Navigate
                state.open_import_config(file_path);
                Command::set_focus(FocusId::new("import-name-input"))
            }
            Msg::ImportFormNameEvent(event) => {
                state.import_form.name.handle_event(event, Some(50));
                Command::None
            }
            Msg::ImportFormSubmit => {
                // Validate form
                match state.import_form.validate() {
                    Ok(_) => {
                        let name = state.import_form.name.value().trim().to_string();
                        let file_path = state.import_file_path.clone();
                        let migration_name = state.migration_name.clone().unwrap_or_default();

                        state.close_import_config();

                        if let Some(path) = file_path {
                            Command::perform(
                                async move {
                                    import_comparison(path, migration_name, name).await
                                },
                                Msg::ImportComplete
                            )
                        } else {
                            Command::None
                        }
                    }
                    Err(validation_error) => {
                        state.import_form.validation_error = Some(validation_error);
                        Command::None
                    }
                }
            }
            Msg::ImportFormCancel => {
                state.close_import_config();
                state.close_import_browser();
                Command::None
            }
            Msg::ImportComplete(result) => {
                match result {
                    Ok(id) => {
                        log::info!("Comparison imported successfully with ID: {}", id);
                        let migration_name = state.migration_name.clone().unwrap_or_default();
                        reload_comparisons(migration_name)
                    }
                    Err(e) => {
                        log::error!("Failed to import comparison: {}", e);
                        // TODO: Show error modal
                        Command::None
                    }
                }
            }
            // Batch export handlers
            Msg::RequestBatchExport => {
                log::info!("📦 Batch export requested");
                state.open_batch_export_modal();
                Command::set_focus(FocusId::new("batch-export-file-browser"))
            }
            Msg::BatchExportBrowseNavigate(key) => {
                match key {
                    KeyCode::Enter => {
                        if let Some(action) = state.batch_export_browser.handle_event(FileBrowserEvent::Activate) {
                            match action {
                                FileBrowserAction::DirectoryEntered(path) => {
                                    let _ = state.batch_export_browser.set_path(path);
                                }
                                _ => {}
                            }
                        }
                        Command::None
                    }
                    _ => {
                        state.batch_export_browser.handle_navigation_key(key);
                        Command::None
                    }
                }
            }
            Msg::BatchExportDirectoryEntered(_path) => {
                // No longer used since we handle Enter in Navigate
                Command::None
            }
            Msg::BatchExportFilenameEvent(event) => {
                state.batch_export_filename.handle_event(event, Some(255));
                Command::None
            }
            Msg::BatchExportConfirm => {
                let directory = state.batch_export_browser.current_path().to_path_buf();
                let filename = state.batch_export_filename.value().trim().to_string();

                log::info!("📦 Batch export confirm - directory: {:?}, filename: {}", directory, filename);

                if filename.is_empty() {
                    log::warn!("Batch export cancelled: empty filename");
                    return Command::None;
                }

                let file_path = directory.join(&filename);
                let comparisons = state.comparisons.clone();

                log::info!("📦 Starting batch export of {} comparisons to {:?}", comparisons.len(), file_path);

                // Close modal immediately
                state.close_batch_export_modal();

                // Perform batch export asynchronously
                Command::perform(
                    async move {
                        log::info!("📦 Batch export async task started");
                        let config = crate::global_config();
                        let result = super::batch_export::export_all_comparisons_to_excel(&config.pool, &comparisons, file_path).await
                            .map_err(|e| e.to_string());
                        log::info!("📦 Batch export async task completed: {:?}", result.is_ok());
                        result
                    },
                    Msg::BatchExportComplete
                )
            }
            Msg::BatchExportCancel => {
                state.close_batch_export_modal();
                Command::None
            }
            Msg::BatchExportComplete(result) => {
                match result {
                    Ok(()) => {
                        log::info!("✅ Batch export completed successfully");
                        // TODO: Show success message to user
                    }
                    Err(e) => {
                        log::error!("❌ Failed to batch export: {}", e);
                        // TODO: Show error modal
                    }
                }
                Command::None
            }
            Msg::Back => Command::batch(vec![
                Command::navigate_to(AppId::MigrationEnvironment),
                Command::quit_self(),
            ]),
        }
    }

    fn view(state: &mut Self::State) -> LayeredView<Self::Msg> {
        use_constraints!();
        let theme = &crate::global_runtime_config().theme;

        log::trace!("MigrationComparisonSelectApp::view() - migration_name={:?}, comparisons={}",
            state.migration_name, state.comparisons.len());
        let list_content = if state.comparisons.is_empty() {
            Element::text("")
        } else {
            Element::list(
                "comparison-list",
                &state.comparisons,
                &state.list_state,
                theme,
            )
            .on_select(|_| Msg::SelectComparison)
            .on_navigate(Msg::ListNavigate)
            .on_activate(|_| Msg::SelectComparison)
            .build()
        };

        let main_ui = Element::panel(list_content)
            .title("Comparisons")
            .build();

        if state.show_delete_confirm {
            // Render delete confirmation modal
            let comparison_name = state.delete_comparison_name.as_deref().unwrap_or("Unknown");

            // Delete confirmation buttons
            let cancel_button = Element::button("delete-cancel", "Cancel".to_string())
                .on_press(Msg::CancelDelete)
                .build();

            let confirm_button = Element::button("delete-confirm", "Delete".to_string())
                .on_press(Msg::ConfirmDelete)
                .style(Style::default().fg(theme.accent_error))
                .build();

            let buttons = Element::row(vec![cancel_button, confirm_button])
                .spacing(2)
                .build();

            // Modal content
            let modal_content = Element::panel(
                Element::container(
                    col![
                        Element::styled_text(Line::from(vec![
                            Span::styled("Delete Comparison", Style::default().fg(theme.accent_tertiary).bold())
                        ])).build() => Length(1),
                        spacer!() => Length(1),
                        Element::text(format!("Delete comparison '{}'?\n\nThis action cannot be undone.", comparison_name)) => Length(3),
                        spacer!() => Length(1),
                        buttons => Length(3),
                    ]
                )
                .padding(2)
                .build()
            )
            .width(60)
            .height(13)
            .build();

            LayeredView::new(main_ui).with_app_modal(modal_content, crate::tui::Alignment::Center)
        } else if state.show_rename_modal {
            // Name input
            let name_input = Element::panel(
                Element::text_input(
                    "rename-name-input",
                    state.rename_form.new_name.value(),
                    &state.rename_form.new_name.state
                )
                .placeholder("Comparison name")
                .on_event(Msg::RenameFormNameEvent)
                .build()
            )
            .title("New Name")
            .build();

            // Buttons
            let buttons = button_row![
                ("rename-cancel", "Cancel", Msg::RenameFormCancel),
                ("rename-confirm", "Rename", Msg::RenameFormSubmit),
            ];

            // Modal content
            let modal_content = Element::panel(
                Element::container(
                    col![
                        name_input => Length(3),
                        spacer!() => Length(1),
                        buttons => Length(3),
                    ]
                )
                .padding(2)
                .build()
            )
            .title("Rename Comparison")
            .width(60)
            .height(13)
            .build();

            LayeredView::new(main_ui).with_app_modal(modal_content, crate::tui::Alignment::Center)
        } else if state.show_create_modal {
            // Name input (using TextInput directly without autocomplete for simple text)
            let name_input = Element::panel(
                Element::text_input(
                    "create-name-input",
                    state.create_form.name.value(),
                    &state.create_form.name.state,
                )
                .placeholder("Comparison name")
                .on_event(Msg::CreateFormNameEvent)
                .build()
            )
            .title("Name")
            .build();

            // Source entity autocomplete with panel
            let source_autocomplete = Element::panel(
                Element::autocomplete(
                    "create-source-autocomplete",
                    state.source_entities.as_ref().ok().cloned().unwrap_or_default(),
                    state.create_form.source_entity.value().to_string(),
                    &mut state.create_form.source_entity.state,
                )
                .placeholder("Type source entity name...")
                .on_event(Msg::CreateFormSourceEvent)
                .build()
            )
            .title("Source Entity")
            .build();

            // Target entity autocomplete with panel
            let target_autocomplete = Element::panel(
                Element::autocomplete(
                    "create-target-autocomplete",
                    state.target_entities.as_ref().ok().cloned().unwrap_or_default(),
                    state.create_form.target_entity.value().to_string(),
                    &mut state.create_form.target_entity.state,
                )
                .placeholder("Type target entity name...")
                .on_event(Msg::CreateFormTargetEvent)
                .build()
            )
            .title("Target Entity")
            .build();

            // Buttons
            let buttons = button_row![
                ("create-cancel", "Cancel", Msg::CreateFormCancel),
                ("create-confirm", "Create", Msg::CreateFormSubmit),
            ];

            // Modal content
            let modal_body = if state.create_form.validation_error.is_some() {
                col![
                    name_input => Length(3),
                    spacer!() => Length(1),
                    source_autocomplete => Length(3),
                    spacer!() => Length(1),
                    target_autocomplete => Length(3),
                    spacer!() => Length(1),
                    error_display!(state.create_form.validation_error, theme) => Length(2),
                    buttons => Length(3),
                ]
            } else {
                col![
                    name_input => Length(3),
                    spacer!() => Length(1),
                    source_autocomplete => Length(3),
                    spacer!() => Length(1),
                    target_autocomplete => Length(3),
                    spacer!() => Length(1),
                    buttons => Length(3),
                ]
            };

            let modal_content = Element::panel(
                Element::container(modal_body)
                .padding(2)
                .build()
            )
            .title("Create New Comparison")
            .width(80)
            .height(if state.create_form.validation_error.is_some() { 23 } else { 21 })
            .build();

            LayeredView::new(main_ui).with_app_modal(modal_content, crate::tui::Alignment::Center)
        } else if state.show_export_modal {
            // Export modal with file browser and filename input
            let current_path_str = state.export_browser.current_path()
                .to_str()
                .unwrap_or("")
                .to_string();

            let path_display = Element::panel(
                Element::text(current_path_str)
            )
            .title("Current Directory")
            .build();

            let file_browser = Element::panel(
                Element::file_browser(
                    "export-file-browser",
                    &state.export_browser,
                    theme
                )
                .on_navigate(Msg::ExportBrowseNavigate)
                .build()
            )
            .title("Select Directory")
            .build();

            let filename_input = Element::panel(
                Element::text_input(
                    "export-filename-input",
                    state.export_filename.value(),
                    &state.export_filename.state
                )
                .placeholder("filename.json")
                .on_event(Msg::ExportFilenameEvent)
                .build()
            )
            .title("Filename")
            .build();

            let buttons = button_row![
                ("export-cancel", "Cancel", Msg::ExportCancel),
                ("export-confirm", "Export", Msg::ExportConfirm),
            ];

            let modal_content = Element::panel(
                Element::container(
                    col![
                        path_display => Length(3),
                        spacer!() => Length(1),
                        file_browser => Fill(1),
                        spacer!() => Length(1),
                        filename_input => Length(3),
                        spacer!() => Length(1),
                        buttons => Length(3),
                    ]
                )
                .padding(2)
                .build()
            )
            .title("Export Comparison")
            .width(100)
            .height(35)
            .build();

            LayeredView::new(main_ui).with_app_modal(modal_content, crate::tui::Alignment::Center)
        } else if state.show_import_browser {
            // Import file browser modal
            let current_path_str = state.import_browser.current_path()
                .to_str()
                .unwrap_or("")
                .to_string();

            let path_display = Element::panel(
                Element::text(current_path_str)
            )
            .title("Current Directory")
            .build();

            let file_browser = Element::panel(
                Element::file_browser(
                    "import-file-browser",
                    &state.import_browser,
                    theme
                )
                .on_navigate(Msg::ImportBrowseNavigate)
                .build()
            )
            .title("Select JSON File")
            .build();

            let buttons = button_row![
                ("import-browse-cancel", "Cancel", Msg::ImportFormCancel),
            ];

            let modal_content = Element::panel(
                Element::container(
                    col![
                        path_display => Length(3),
                        spacer!() => Length(1),
                        file_browser => Fill(1),
                        spacer!() => Length(1),
                        Element::text("Press Enter to select file") => Length(1),
                        spacer!() => Length(1),
                        buttons => Length(3),
                    ]
                )
                .padding(2)
                .build()
            )
            .title("Import Comparison")
            .width(100)
            .height(35)
            .build();

            LayeredView::new(main_ui).with_app_modal(modal_content, crate::tui::Alignment::Center)
        } else if state.show_import_config {
            // Import configuration modal (name only, entities come from JSON)
            let name_input = Element::panel(
                Element::text_input(
                    "import-name-input",
                    state.import_form.name.value(),
                    &state.import_form.name.state,
                )
                .placeholder("Comparison name")
                .on_event(Msg::ImportFormNameEvent)
                .build()
            )
            .title("Comparison Name")
            .build();

            let buttons = button_row![
                ("import-config-cancel", "Cancel", Msg::ImportFormCancel),
                ("import-config-confirm", "Import", Msg::ImportFormSubmit),
            ];

            let modal_body = if state.import_form.validation_error.is_some() {
                col![
                    Element::text("Entities will be read from the JSON file.") => Length(1),
                    spacer!() => Length(1),
                    name_input => Length(3),
                    spacer!() => Length(1),
                    error_display!(state.import_form.validation_error, theme) => Length(2),
                    buttons => Length(3),
                ]
            } else {
                col![
                    Element::text("Entities will be read from the JSON file.") => Length(1),
                    spacer!() => Length(1),
                    name_input => Length(3),
                    spacer!() => Length(1),
                    buttons => Length(3),
                ]
            };

            let modal_content = Element::panel(
                Element::container(modal_body)
                .padding(2)
                .build()
            )
            .title("Import Comparison")
            .width(60)
            .height(if state.import_form.validation_error.is_some() { 15 } else { 13 })
            .build();

            LayeredView::new(main_ui).with_app_modal(modal_content, crate::tui::Alignment::Center)
        } else if state.show_batch_export_modal {
            // Batch export modal with file browser and filename input
            let current_path_str = state.batch_export_browser.current_path()
                .to_str()
                .unwrap_or("")
                .to_string();

            let path_display = Element::panel(
                Element::text(current_path_str)
            )
            .title("Current Directory")
            .build();

            let file_browser = Element::panel(
                Element::file_browser(
                    "batch-export-file-browser",
                    &state.batch_export_browser,
                    theme
                )
                .on_navigate(Msg::BatchExportBrowseNavigate)
                .build()
            )
            .title("Select Directory")
            .build();

            let filename_input = Element::panel(
                Element::text_input(
                    "batch-export-filename-input",
                    state.batch_export_filename.value(),
                    &state.batch_export_filename.state
                )
                .placeholder("filename.xlsx")
                .on_event(Msg::BatchExportFilenameEvent)
                .build()
            )
            .title("Filename")
            .build();

            let buttons = button_row![
                ("batch-export-cancel", "Cancel", Msg::BatchExportCancel),
                ("batch-export-confirm", "Export All", Msg::BatchExportConfirm),
            ];

            let modal_content = Element::panel(
                Element::container(
                    col![
                        path_display => Length(3),
                        spacer!() => Length(1),
                        file_browser => Fill(1),
                        spacer!() => Length(1),
                        filename_input => Length(3),
                        spacer!() => Length(1),
                        Element::text(format!("Will export mappings from {} comparison(s)",
                            state.comparisons.len())) => Length(1),
                        Element::text("(Comparisons without mappings will be skipped)") => Length(1),
                        spacer!() => Length(1),
                        buttons => Length(3),
                    ]
                )
                .padding(2)
                .build()
            )
            .title("Batch Export All Mappings")
            .width(100)
            .height(38)
            .build();

            LayeredView::new(main_ui).with_app_modal(modal_content, crate::tui::Alignment::Center)
        } else {
            LayeredView::new(main_ui)
        }
    }

    fn subscriptions(state: &Self::State) -> Vec<Subscription<Self::Msg>> {
        let mut subs = vec![];

        if !state.show_create_modal && !state.show_delete_confirm && !state.show_rename_modal
            && !state.show_export_modal && !state.show_import_browser && !state.show_import_config
            && !state.show_batch_export_modal {
            let config = crate::global_runtime_config();

            subs.push(Subscription::keyboard(KeyCode::Esc, "Back to migration list", Msg::Back));
            subs.push(Subscription::keyboard(config.get_keybind("migration_comparison.back"), "Back to migration list", Msg::Back));

            if !state.comparisons.is_empty() {
                subs.push(Subscription::keyboard(
                    KeyCode::Enter,
                    "Select comparison",
                    Msg::SelectComparison,
                ));
            }

            let create_kb = config.get_keybind("migration_comparison.create");
            subs.push(Subscription::keyboard(create_kb, "Create comparison", Msg::CreateComparison));

            let delete_kb = config.get_keybind("migration_comparison.delete");
            subs.push(Subscription::keyboard(delete_kb, "Delete comparison", Msg::RequestDelete));

            let rename_kb = config.get_keybind("migration_comparison.rename");
            subs.push(Subscription::keyboard(rename_kb, "Rename comparison", Msg::RequestRename));

            let preload_kb = config.get_keybind("migration_comparison.preload");
            subs.push(Subscription::keyboard(preload_kb, "Preload all comparisons", Msg::PreloadAllComparisons));

            // Export/Import hotkeys
            if state.list_state.selected().is_some() {
                subs.push(Subscription::keyboard(KeyCode::Char('e'), "Export comparison", Msg::RequestExport));
            }
            subs.push(Subscription::keyboard(KeyCode::Char('i'), "Import comparison", Msg::RequestImport));

            // Batch export hotkey (always available if comparisons exist)
            if !state.comparisons.is_empty() {
                subs.push(Subscription::keyboard(KeyCode::Char('E'), "Batch export all", Msg::RequestBatchExport));
            }
        } else if state.show_create_modal {
            subs.push(Subscription::keyboard(KeyCode::Esc, "Close modal", Msg::CreateFormCancel));
        } else if state.show_delete_confirm {
            subs.push(Subscription::keyboard(KeyCode::Esc, "Cancel delete", Msg::CancelDelete));
        } else if state.show_rename_modal {
            subs.push(Subscription::keyboard(KeyCode::Esc, "Close modal", Msg::RenameFormCancel));
        } else if state.show_export_modal {
            subs.push(Subscription::keyboard(KeyCode::Esc, "Cancel export", Msg::ExportCancel));
        } else if state.show_import_browser {
            subs.push(Subscription::keyboard(KeyCode::Esc, "Cancel import", Msg::ImportFormCancel));
        } else if state.show_import_config {
            subs.push(Subscription::keyboard(KeyCode::Esc, "Cancel import", Msg::ImportFormCancel));
        } else if state.show_batch_export_modal {
            subs.push(Subscription::keyboard(KeyCode::Esc, "Cancel batch export", Msg::BatchExportCancel));
        }

        subs
    }

    fn title() -> &'static str {
        "Migration Comparison Select"
    }

    fn status(state: &Self::State) -> Option<Line<'static>> {
        log::trace!("MigrationComparisonSelectApp::status() - migration_name={:?}", state.migration_name);
        let theme = &crate::global_runtime_config().theme;
        if let Some(ref migration_name) = state.migration_name {
            let source = state.source_env.as_deref().unwrap_or("?");
            let target = state.target_env.as_deref().unwrap_or("?");

            let source_count_str = match &state.source_entities {
                Resource::Loading => "...".to_string(),
                Resource::Success(v) => v.len().to_string(),
                Resource::Failure(_) => "ERR".to_string(),
                Resource::NotAsked => "0".to_string(),
            };

            let target_count_str = match &state.target_entities {
                Resource::Loading => "...".to_string(),
                Resource::Success(v) => v.len().to_string(),
                Resource::Failure(_) => "ERR".to_string(),
                Resource::NotAsked => "0".to_string(),
            };

            Some(Line::from(vec![
                Span::styled(migration_name.clone(), Style::default().fg(theme.text_primary)),
                Span::styled(
                    format!(" ({} → {})", source, target),
                    Style::default().fg(theme.text_secondary),
                ),
                Span::styled(
                    format!(" ({}:{})", source_count_str, target_count_str),
                    Style::default().fg(theme.border_primary),
                ),
            ]))
        } else {
            Some(Line::from(vec![
                Span::styled("Loading migration data...", Style::default().fg(theme.text_secondary))
            ]))
        }
    }
}

// Helper functions

fn reload_comparisons(migration_name: String) -> Command<Msg> {
    Command::perform(
        async move {
            let config = crate::global_config();
            config.get_comparisons(&migration_name).await.map_err(|e| e.to_string())
        },
        Msg::ComparisonsLoaded
    )
}

/// Export comparison data to JSON file
async fn export_comparison(comparison_id: i64, file_path: PathBuf) -> Result<(), String> {
    log::info!("Exporting comparison {} to {:?}", comparison_id, file_path);

    let config = crate::global_config();

    // Fetch comparison from database
    let comparison = config.get_comparison_by_id(comparison_id)
        .await
        .map_err(|e| format!("Failed to fetch comparison: {}", e))?
        .ok_or_else(|| "Comparison not found".to_string())?;

    // Parse entity_comparison JSON if present
    let (field_mappings, prefix_mappings, imported_mappings, import_source_file, ignored_items) =
        if let Some(json_str) = &comparison.entity_comparison {
            parse_entity_comparison_json(json_str)?
        } else {
            (HashMap::new(), HashMap::new(), HashMap::new(), None, Vec::new())
        };

    // Fetch example pairs
    let example_pairs = config.get_example_pairs(&comparison.source_entity, &comparison.target_entity)
        .await
        .map_err(|e| format!("Failed to fetch example pairs: {}", e))?
        .into_iter()
        .map(|pair| ExamplePairExport {
            source_record_id: pair.source_record_id,
            target_record_id: pair.target_record_id,
            label: pair.label,
        })
        .collect();

    // Build export data
    let export_data = ComparisonExportData {
        version: "1.0".to_string(),
        export_date: chrono::Utc::now().to_rfc3339(),
        source_entity: comparison.source_entity,
        target_entity: comparison.target_entity,
        field_mappings,
        prefix_mappings,
        imported_mappings,
        import_source_file,
        ignored_items,
        example_pairs,
    };

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&export_data)
        .map_err(|e| format!("Failed to serialize: {}", e))?;

    // Write to file
    std::fs::write(&file_path, json)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    log::info!("Successfully exported comparison to {:?}", file_path);
    Ok(())
}

/// Import comparison data from JSON file
async fn import_comparison(
    file_path: PathBuf,
    migration_name: String,
    name: String,
) -> Result<i64, String> {
    log::info!("Importing comparison from {:?}", file_path);

    // Read file
    let json = std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // Parse JSON
    let import_data: ComparisonExportData = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    // Get entities from JSON file
    let source_entity = import_data.source_entity.clone();
    let target_entity = import_data.target_entity.clone();

    // Build entity_comparison JSON
    let entity_comparison_json = build_entity_comparison_json(
        import_data.field_mappings,
        import_data.prefix_mappings,
        import_data.imported_mappings,
        import_data.import_source_file,
        import_data.ignored_items,
    )?;

    // Create SavedComparison
    let config = crate::global_config();
    let comparison = SavedComparison {
        id: 0, // Will be assigned by database
        name,
        migration_name,
        source_entity: source_entity.clone(),
        target_entity: target_entity.clone(),
        source_entities: vec![source_entity.clone()],  // Single entity for now
        target_entities: vec![target_entity.clone()],  // Single entity for now
        entity_comparison: Some(entity_comparison_json),
        created_at: chrono::Utc::now(),
        last_used: chrono::Utc::now(),
    };

    let comparison_id = config.add_comparison(comparison)
        .await
        .map_err(|e| format!("Failed to save comparison: {}", e))?;

    // Get existing example pairs to avoid duplicates
    let existing_pairs = config.get_example_pairs(&source_entity, &target_entity)
        .await
        .map_err(|e| format!("Failed to get existing example pairs: {}", e))?;

    // Save example pairs (skip if already exists)
    for pair in import_data.example_pairs {
        // Check if this pair already exists (same source and target UUIDs)
        let already_exists = existing_pairs.iter().any(|existing| {
            existing.source_record_id == pair.source_record_id
                && existing.target_record_id == pair.target_record_id
        });

        if already_exists {
            log::debug!("Skipping duplicate example pair: {} -> {}", pair.source_record_id, pair.target_record_id);
            continue;
        }

        let example_pair = crate::tui::apps::migration::entity_comparison::ExamplePair {
            id: uuid::Uuid::new_v4().to_string(),
            source_record_id: pair.source_record_id,
            target_record_id: pair.target_record_id,
            label: pair.label,
        };

        config.save_example_pair(&source_entity, &target_entity, &example_pair)
            .await
            .map_err(|e| format!("Failed to save example pair: {}", e))?;
    }

    log::info!("Successfully imported comparison with ID: {}", comparison_id);
    Ok(comparison_id)
}

/// Parse entity_comparison JSON string from database
fn parse_entity_comparison_json(json_str: &str) -> Result<(
    HashMap<String, Vec<String>>,
    HashMap<String, Vec<String>>,
    HashMap<String, Vec<String>>,
    Option<String>,
    Vec<String>,
), String> {
    #[derive(Deserialize)]
    struct EntityComparisonData {
        field_mappings: Option<HashMap<String, Vec<String>>>,
        prefix_mappings: Option<HashMap<String, Vec<String>>>,
        imported_mappings: Option<HashMap<String, Vec<String>>>,
        import_source_file: Option<String>,
        ignored_items: Option<Vec<String>>,
    }

    let data: EntityComparisonData = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse entity_comparison JSON: {}", e))?;

    Ok((
        data.field_mappings.unwrap_or_default(),
        data.prefix_mappings.unwrap_or_default(),
        data.imported_mappings.unwrap_or_default(),
        data.import_source_file,
        data.ignored_items.unwrap_or_default(),
    ))
}

/// Build entity_comparison JSON string for database
fn build_entity_comparison_json(
    field_mappings: HashMap<String, Vec<String>>,
    prefix_mappings: HashMap<String, Vec<String>>,
    imported_mappings: HashMap<String, Vec<String>>,
    import_source_file: Option<String>,
    ignored_items: Vec<String>,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct EntityComparisonData {
        field_mappings: HashMap<String, Vec<String>>,
        prefix_mappings: HashMap<String, Vec<String>>,
        imported_mappings: HashMap<String, Vec<String>>,
        import_source_file: Option<String>,
        ignored_items: Vec<String>,
    }

    let data = EntityComparisonData {
        field_mappings,
        prefix_mappings,
        imported_mappings,
        import_source_file,
        ignored_items,
    };

    serde_json::to_string(&data)
        .map_err(|e| format!("Failed to serialize entity_comparison: {}", e))
}
