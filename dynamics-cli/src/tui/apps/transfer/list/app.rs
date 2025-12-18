use crate::config::repository::transfer::{
    delete_transfer_config, get_transfer_config, list_transfer_configs, save_transfer_config,
    transfer_config_exists, TransferConfigSummary,
};
use crate::transfer::TransferConfig;
use crate::tui::element::FocusId;
use crate::tui::resource::Resource;
use crate::tui::widgets::ListState;
use crate::tui::{App, AppId, Command, LayeredView, Subscription};

use super::state::{CloneConfigForm, CreateConfigForm, Msg, State};
use super::view;

pub struct TransferConfigListApp;

impl crate::tui::AppState for State {}

impl App for TransferConfigListApp {
    type State = State;
    type Msg = Msg;
    type InitParams = ();

    fn init(_params: ()) -> (State, Command<Msg>) {
        let state = State {
            configs: Resource::Loading,
            list_state: ListState::with_selection(),
            show_delete_confirm: false,
            selected_for_delete: None,
            show_create_modal: false,
            create_form: CreateConfigForm::default(),
            environments: Resource::NotAsked,
            show_clone_modal: false,
            clone_form: CloneConfigForm::default(),
            selected_for_clone: None,
        };

        let cmd = Command::perform(load_configs(), Msg::ConfigsLoaded);

        (state, cmd)
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::ConfigsLoaded(result) => {
                state.configs = match result {
                    Ok(configs) => Resource::Success(configs),
                    Err(e) => Resource::Failure(e),
                };
                // Set focus to list once configs are loaded
                Command::set_focus(FocusId::new("config-list"))
            }

            Msg::EnvironmentsLoaded(result) => {
                state.environments = match result {
                    Ok(envs) => Resource::Success(envs),
                    Err(e) => Resource::Failure(e),
                };
                Command::None
            }

            Msg::ListNavigate(key) => {
                if let Resource::Success(configs) = &state.configs {
                    let visible_height = 20;
                    state
                        .list_state
                        .handle_key(key, configs.len(), visible_height);
                }
                Command::None
            }

            Msg::SelectConfig(idx) => {
                if let Resource::Success(configs) = &state.configs {
                    if let Some(config) = configs.get(idx) {
                        return navigate_to_editor(&config.name);
                    }
                }
                Command::None
            }

            Msg::CreateNew => {
                state.show_create_modal = true;
                state.create_form = CreateConfigForm::default();
                if matches!(state.environments, Resource::NotAsked) {
                    state.environments = Resource::Loading;
                    return Command::batch(vec![
                        Command::perform(load_environments(), Msg::EnvironmentsLoaded),
                        Command::set_focus(FocusId::new("create-name")),
                    ]);
                }
                Command::set_focus(FocusId::new("create-name"))
            }

            Msg::CloseCreateModal => {
                state.show_create_modal = false;
                Command::set_focus(FocusId::new("config-list"))
            }

            Msg::CreateFormName(event) => {
                state.create_form.name.handle_event(event, Some(100));
                Command::None
            }

            Msg::CreateFormSourceEnv(event) => {
                let options = match &state.environments {
                    Resource::Success(envs) => envs.clone(),
                    _ => vec![],
                };
                state.create_form.source_env.handle_event::<Msg>(event, &options);
                Command::None
            }

            Msg::CreateFormTargetEnv(event) => {
                let options = match &state.environments {
                    Resource::Success(envs) => envs.clone(),
                    _ => vec![],
                };
                state.create_form.target_env.handle_event::<Msg>(event, &options);
                Command::None
            }

            Msg::SaveNewConfig => {
                if !state.create_form.is_valid() {
                    return Command::None;
                }

                let name = state.create_form.name.value.trim().to_string();
                let source_env = state.create_form.source_env.value.trim().to_string();
                let target_env = state.create_form.target_env.value.trim().to_string();

                state.show_create_modal = false;

                Command::perform(
                    create_config(name, source_env, target_env),
                    Msg::ConfigCreated,
                )
            }

            Msg::ConfigCreated(result) => {
                match result {
                    Ok(name) => {
                        // Refresh the list, then navigate to editor
                        state.configs = Resource::Loading;
                        Command::batch(vec![
                            Command::perform(load_configs(), Msg::ConfigsLoaded),
                            navigate_to_editor(&name),
                        ])
                    }
                    Err(_e) => {
                        // TODO: show error
                        Command::None
                    }
                }
            }

            Msg::EditSelected => {
                if let Resource::Success(configs) = &state.configs {
                    if let Some(idx) = state.list_state.selected() {
                        if let Some(config) = configs.get(idx) {
                            return navigate_to_editor(&config.name);
                        }
                    }
                }
                Command::None
            }

            Msg::DeleteSelected => {
                if let Resource::Success(configs) = &state.configs {
                    if let Some(idx) = state.list_state.selected() {
                        if let Some(config) = configs.get(idx) {
                            state.show_delete_confirm = true;
                            state.selected_for_delete = Some(config.name.clone());
                        }
                    }
                }
                Command::None
            }

            Msg::ConfirmDelete => {
                if let Some(name) = state.selected_for_delete.take() {
                    state.show_delete_confirm = false;
                    return Command::perform(delete_config(name), Msg::DeleteResult);
                }
                Command::None
            }

            Msg::CancelDelete => {
                state.show_delete_confirm = false;
                state.selected_for_delete = None;
                Command::None
            }

            Msg::DeleteResult(result) => {
                if result.is_ok() {
                    state.configs = Resource::Loading;
                    Command::perform(load_configs(), Msg::ConfigsLoaded)
                } else {
                    // TODO: Show error modal
                    Command::None
                }
            }

            Msg::Refresh => {
                state.configs = Resource::Loading;
                Command::perform(load_configs(), Msg::ConfigsLoaded)
            }

            Msg::CloneSelected => {
                if let Resource::Success(configs) = &state.configs {
                    if let Some(idx) = state.list_state.selected() {
                        if let Some(config) = configs.get(idx) {
                            state.show_clone_modal = true;
                            state.selected_for_clone = Some(config.name.clone());
                            state.clone_form = CloneConfigForm::default();
                            state.clone_form.name.value = format!("{} (Copy)", config.name);
                            return Command::set_focus(FocusId::new("clone-name"));
                        }
                    }
                }
                Command::None
            }

            Msg::CloseCloneModal => {
                state.show_clone_modal = false;
                state.selected_for_clone = None;
                Command::set_focus(FocusId::new("config-list"))
            }

            Msg::CloneFormName(event) => {
                state.clone_form.name.handle_event(event, Some(100));
                Command::None
            }

            Msg::SaveClone => {
                if !state.clone_form.is_valid() {
                    return Command::None;
                }

                if let Some(original_name) = state.selected_for_clone.take() {
                    let new_name = state.clone_form.name.value.trim().to_string();
                    state.show_clone_modal = false;
                    return Command::perform(
                        clone_config(original_name, new_name),
                        Msg::CloneResult,
                    );
                }
                Command::None
            }

            Msg::CloneResult(result) => match result {
                Ok(name) => {
                    state.configs = Resource::Loading;
                    Command::batch(vec![
                        Command::perform(load_configs(), Msg::ConfigsLoaded),
                        navigate_to_editor(&name),
                    ])
                }
                Err(_e) => {
                    // TODO: show error modal
                    Command::None
                }
            },
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
        "Transfer Configs"
    }
}

async fn load_configs() -> Result<Vec<TransferConfigSummary>, String> {
    let pool = &crate::global_config().pool;
    list_transfer_configs(pool)
        .await
        .map_err(|e| e.to_string())
}

async fn load_environments() -> Result<Vec<String>, String> {
    let pool = &crate::global_config().pool;
    crate::global_config()
        .list_environments()
        .await
        .map_err(|e| e.to_string())
}

async fn delete_config(name: String) -> Result<(), String> {
    let pool = &crate::global_config().pool;
    delete_transfer_config(pool, &name)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

async fn create_config(name: String, source_env: String, target_env: String) -> Result<String, String> {
    let pool = &crate::global_config().pool;

    let config = TransferConfig {
        id: None,
        name: name.clone(),
        source_env,
        target_env,
        entity_mappings: vec![],
    };

    save_transfer_config(pool, &config)
        .await
        .map(|_| name)
        .map_err(|e| e.to_string())
}

async fn clone_config(original_name: String, new_name: String) -> Result<String, String> {
    let pool = &crate::global_config().pool;

    // Check if new name already exists
    if transfer_config_exists(pool, &new_name)
        .await
        .map_err(|e| e.to_string())?
    {
        return Err(format!("Config '{}' already exists", new_name));
    }

    // Load original config
    let original = get_transfer_config(pool, &original_name)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Config '{}' not found", original_name))?;

    // Clone and reset IDs
    let mut cloned = original.clone();
    cloned.id = None;
    cloned.name = new_name.clone();
    for entity in &mut cloned.entity_mappings {
        entity.id = None;
        for resolver in &mut entity.resolvers {
            resolver.id = None;
        }
        for field in &mut entity.field_mappings {
            field.id = None;
        }
    }

    // Save the cloned config
    save_transfer_config(pool, &cloned)
        .await
        .map(|_| new_name)
        .map_err(|e| e.to_string())
}

fn navigate_to_editor(config_name: &str) -> Command<Msg> {
    use crate::tui::AppId;
    use crate::tui::apps::transfer::EditorParams;

    Command::start_app(
        AppId::TransferMappingEditor,
        EditorParams {
            config_name: config_name.to_string(),
        },
    )
}
