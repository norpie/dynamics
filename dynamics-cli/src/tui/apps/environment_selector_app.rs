use crate::api::models::{CredentialSet, Environment as ApiEnvironment};
use crate::tui::apps::screens::ErrorScreenParams;
use crate::tui::renderer::LayeredView;
use crate::tui::widgets::{SelectEvent, SelectField, TextInputEvent, TextInputField};
use crate::tui::{App, AppId, Command, Element, FocusId, Resource, Subscription, Theme};
use crate::{col, row, spacer, use_constraints};
use crossterm::event::KeyCode;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use_constraints!();

pub struct EnvironmentSelectorApp;

// ============================================================================
// State
// ============================================================================

#[derive(Clone)]
pub struct State {
    // Data
    environments: Vec<ApiEnvironment>,
    credentials: Vec<String>,
    current_environment: Option<String>,

    // Environment panel
    env_selector: SelectField,
    env_name_field: TextInputField,
    env_host_field: TextInputField,
    env_creds_selector: SelectField,
    active_env_selector: SelectField,

    // Credential panel
    cred_selector: SelectField,
    cred_name_field: TextInputField,
    cred_type_selector: SelectField,
    // Credential type-specific fields
    cred_username_field: TextInputField,
    cred_password_field: TextInputField,
    cred_client_id_field: TextInputField,
    cred_client_secret_field: TextInputField,
    cred_tenant_id_field: TextInputField,
    cred_cert_path_field: TextInputField,

    // Loading/operation states
    data_load_state: Resource<()>,
    env_save_state: Resource<()>,
    env_delete_state: Resource<()>,
    cred_save_state: Resource<()>,
    cred_delete_state: Resource<()>,
    set_current_state: Resource<()>,

    // UI state
    env_panel_dirty: bool,
    cred_panel_dirty: bool,

    // Track recently saved items to auto-select them after reload
    recently_saved_env: Option<String>,
    recently_saved_cred: Option<String>,
}

impl State {
    fn new() -> Self {
        Self {
            environments: Vec::new(),
            credentials: Vec::new(),
            current_environment: None,

            env_selector: SelectField::new(),
            env_name_field: TextInputField::new(),
            env_host_field: TextInputField::new(),
            env_creds_selector: SelectField::new(),
            active_env_selector: SelectField::new(),

            cred_selector: SelectField::new(),
            cred_name_field: TextInputField::new(),
            cred_type_selector: SelectField::new(),
            cred_username_field: TextInputField::new(),
            cred_password_field: TextInputField::new(),
            cred_client_id_field: TextInputField::new(),
            cred_client_secret_field: TextInputField::new(),
            cred_tenant_id_field: TextInputField::new(),
            cred_cert_path_field: TextInputField::new(),

            data_load_state: Resource::NotAsked,
            env_save_state: Resource::NotAsked,
            env_delete_state: Resource::NotAsked,
            cred_save_state: Resource::NotAsked,
            cred_delete_state: Resource::NotAsked,
            set_current_state: Resource::NotAsked,

            env_panel_dirty: false,
            cred_panel_dirty: false,

            recently_saved_env: None,
            recently_saved_cred: None,
        }
    }

    fn get_selected_environment(&self) -> Option<&ApiEnvironment> {
        self.env_selector
            .value()
            .and_then(|name| self.environments.iter().find(|e| e.name == name))
    }

    fn get_selected_credential_name(&self) -> Option<&str> {
        self.cred_selector.value()
    }

    fn get_credential_type_options() -> Vec<String> {
        vec![
            "Username/Password".to_string(),
            "Client Credentials".to_string(),
            "Device Code".to_string(),
            "Certificate".to_string(),
        ]
    }

    fn credential_type_to_string(cred: &CredentialSet) -> String {
        match cred {
            CredentialSet::UsernamePassword { .. } => "Username/Password".to_string(),
            CredentialSet::ClientCredentials { .. } => "Client Credentials".to_string(),
            CredentialSet::DeviceCode { .. } => "Device Code".to_string(),
            CredentialSet::Certificate { .. } => "Certificate".to_string(),
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Messages
// ============================================================================

#[derive(Clone)]
pub enum Msg {
    // Data loading
    DataLoaded(Result<LoadedData, String>),

    // Environment selector
    EnvSelectorEvent(SelectEvent),
    EnvSelected(String),

    // Environment form fields
    EnvNameChanged(TextInputEvent),
    EnvHostChanged(TextInputEvent),
    EnvCredsEvent(SelectEvent),
    ActiveEnvEvent(SelectEvent),

    // Environment actions
    SaveEnvironment,
    EnvironmentSaved(Result<(), String>),
    DeleteEnvironment,
    EnvironmentDeleted(Result<(), String>),
    NewEnvironment,

    // Credential selector
    CredSelectorEvent(SelectEvent),
    CredSelected(String),
    CredentialDataLoaded(Result<CredentialSet, String>),

    // Credential form fields
    CredNameChanged(TextInputEvent),
    CredTypeEvent(SelectEvent),
    CredUsernameChanged(TextInputEvent),
    CredPasswordChanged(TextInputEvent),
    CredClientIdChanged(TextInputEvent),
    CredClientSecretChanged(TextInputEvent),
    CredTenantIdChanged(TextInputEvent),
    CredCertPathChanged(TextInputEvent),

    // Credential actions
    SaveCredential,
    CredentialSaved(Result<(), String>),
    DeleteCredential,
    CredentialDeleted(Result<(), String>),
    NewCredential,

    // Global actions
    SetCurrentEnvironment,
    CurrentEnvironmentSet(Result<(), String>),
}

#[derive(Clone)]
pub struct LoadedData {
    pub environments: Vec<ApiEnvironment>,
    pub credentials: Vec<String>,
    pub current_env: Option<String>,
}

impl crate::tui::AppState for State {}

// ============================================================================
// App Implementation
// ============================================================================

impl App for EnvironmentSelectorApp {
    type State = State;
    type Msg = Msg;
    type InitParams = ();

    fn init(_params: ()) -> (State, Command<Msg>) {
        let state = State::default();
        let cmd = Command::batch(vec![
            Command::perform(
                async {
                    let config = crate::global_config();
                    let manager = crate::client_manager();

                    let envs_result = config
                        .list_environments()
                        .await
                        .map_err(|e| e.to_string())?;

                    let mut environments = Vec::new();
                    for env_name in envs_result {
                        if let Ok(Some(env)) = config.get_environment(&env_name).await {
                            environments.push(env);
                        }
                    }

                    let credentials = config.list_credentials().await.map_err(|e| e.to_string())?;

                    let current = manager
                        .get_current_environment_name()
                        .await
                        .map_err(|e| e.to_string())?;

                    Ok(LoadedData {
                        environments,
                        credentials,
                        current_env: current,
                    })
                },
                Msg::DataLoaded,
            ),
            Command::set_focus(FocusId::new("env-selector")),
        ]);
        (state, cmd)
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::DataLoaded(Ok(data)) => {
                state.current_environment = data.current_env;
                state.environments = data.environments;
                state.credentials = data.credentials;
                state.data_load_state = Resource::Success(());

                // Select recently saved environment, or first one if none saved recently
                if !state.environments.is_empty() {
                    let env_to_select = if let Some(ref saved_name) = state.recently_saved_env {
                        // Try to find the recently saved environment
                        state
                            .environments
                            .iter()
                            .find(|e| &e.name == saved_name)
                            .or_else(|| state.environments.first())
                    } else {
                        // No recent save, select first
                        state.environments.first()
                    };

                    if let Some(env) = env_to_select {
                        let env_names: Vec<String> =
                            state.environments.iter().map(|e| e.name.clone()).collect();
                        state
                            .env_selector
                            .set_value_with_options(Some(env.name.clone()), &env_names);
                        state.env_name_field.set_value(env.name.clone());
                        state.env_host_field.set_value(env.host.clone());
                        state
                            .env_creds_selector
                            .set_value(Some(env.credentials_ref.clone()));
                        state.env_panel_dirty = false;
                    }

                    // Clear the recently saved flag after using it
                    state.recently_saved_env = None;
                }

                // Set active environment selector with proper index
                if let Some(ref current) = state.current_environment {
                    let env_names: Vec<String> =
                        state.environments.iter().map(|e| e.name.clone()).collect();
                    state
                        .active_env_selector
                        .set_value_with_options(Some(current.clone()), &env_names);
                }

                // Select recently saved credential, or first one if none saved recently
                if !state.credentials.is_empty() {
                    let cred_to_select = if let Some(ref saved_name) = state.recently_saved_cred {
                        // Try to find the recently saved credential
                        state
                            .credentials
                            .iter()
                            .find(|c| c == &saved_name)
                            .or_else(|| state.credentials.first())
                    } else {
                        // No recent save, select first
                        state.credentials.first()
                    };

                    if let Some(cred_name) = cred_to_select {
                        let cred_name = cred_name.clone();
                        state
                            .cred_selector
                            .set_value_with_options(Some(cred_name.clone()), &state.credentials);
                        state.cred_name_field.set_value(cred_name.clone());

                        // Clear the recently saved flag after using it
                        state.recently_saved_cred = None;

                        return Command::perform(
                            async move {
                                let config = crate::global_config();
                                log::debug!("Auto-loading credential: {}", cred_name);
                                config
                                    .get_credentials(&cred_name)
                                    .await
                                    .map_err(|e| e.to_string())?
                                    .ok_or_else(|| "Credential not found".to_string())
                            },
                            Msg::CredentialDataLoaded,
                        );
                    }
                }

                Command::None
            }
            Msg::DataLoaded(Err(err)) => {
                state.data_load_state = Resource::Failure(err.clone());
                log::error!("Failed to load data: {}", err);
                Command::start_app(
                    AppId::ErrorScreen,
                    ErrorScreenParams {
                        message: format!("Failed to load configuration: {}", err),
                        target: Some(AppId::EnvironmentSelector),
                    },
                )
            }

            Msg::EnvSelectorEvent(event) => {
                let env_names: Vec<String> =
                    state.environments.iter().map(|e| e.name.clone()).collect();

                let (cmd, selection) = state.env_selector.handle_event(event, &env_names);

                if let Some(SelectEvent::Select(idx)) = selection {
                    if let Some(env) = state.environments.get(idx) {
                        // Populate environment form fields inline
                        state.env_name_field.set_value(env.name.clone());
                        state.env_host_field.set_value(env.host.clone());
                        state
                            .env_creds_selector
                            .set_value(Some(env.credentials_ref.clone()));
                        state.env_panel_dirty = false;
                    }
                }

                cmd
            }

            Msg::EnvSelected(name) => {
                // Populate environment form fields
                if let Some(env) = state.environments.iter().find(|e| e.name == name) {
                    state.env_name_field.set_value(env.name.clone());
                    state.env_host_field.set_value(env.host.clone());
                    state
                        .env_creds_selector
                        .set_value(Some(env.credentials_ref.clone()));
                    state.env_panel_dirty = false;
                }
                Command::None
            }

            Msg::EnvNameChanged(event) => {
                state.env_name_field.handle_event(event, None);
                state.env_panel_dirty = true;
                Command::None
            }

            Msg::EnvHostChanged(event) => {
                state.env_host_field.handle_event(event, None);
                state.env_panel_dirty = true;
                Command::None
            }

            Msg::EnvCredsEvent(event) => {
                let (cmd, selection) = state
                    .env_creds_selector
                    .handle_event(event, &state.credentials);
                if selection.is_some() {
                    state.env_panel_dirty = true;
                }
                cmd
            }

            Msg::ActiveEnvEvent(event) => {
                let env_names: Vec<String> =
                    state.environments.iter().map(|e| e.name.clone()).collect();

                let (cmd, selection) = state.active_env_selector.handle_event(event, &env_names);

                if let Some(SelectEvent::Select(idx)) = selection {
                    if let Some(env) = state.environments.get(idx) {
                        let env_name = env.name.clone();
                        log::debug!("Setting active environment to: {}", env_name);

                        return Command::perform(
                            async move {
                                let manager = crate::client_manager();
                                manager
                                    .set_current_environment_in_config(env_name)
                                    .await
                                    .map_err(|e| e.to_string())
                            },
                            Msg::CurrentEnvironmentSet,
                        );
                    }
                }

                cmd
            }

            Msg::NewEnvironment => {
                state.env_selector.set_value(None);
                state.env_name_field.set_value(String::new());
                state.env_host_field.set_value(String::new());
                state.env_creds_selector.set_value(None);
                state.env_panel_dirty = true;
                Command::set_focus(FocusId::new("env-name"))
            }

            Msg::SaveEnvironment => {
                let name = state.env_name_field.value().to_string();
                let host = state.env_host_field.value().to_string();
                let creds_ref = state
                    .env_creds_selector
                    .value()
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                if name.is_empty() || host.is_empty() || creds_ref.is_empty() {
                    state.env_save_state =
                        Resource::Failure("Name, Host, and Credentials are required".to_string());
                    return Command::None;
                }

                state.env_save_state = Resource::Loading;
                // Remember the name so we can select it after reload
                state.recently_saved_env = Some(name.clone());

                Command::perform(
                    async move {
                        let config = crate::global_config();
                        let env = ApiEnvironment {
                            name,
                            host,
                            credentials_ref: creds_ref,
                        };
                        config.add_environment(env).await.map_err(|e| e.to_string())
                    },
                    Msg::EnvironmentSaved,
                )
            }

            Msg::EnvironmentSaved(Ok(())) => {
                state.env_save_state = Resource::Success(());
                state.env_panel_dirty = false;

                log::debug!("Environment saved successfully");

                // Reload data
                Command::perform(
                    async {
                        let config = crate::global_config();
                        let manager = crate::client_manager();

                        let envs_result = config
                            .list_environments()
                            .await
                            .map_err(|e| e.to_string())?;

                        let mut environments = Vec::new();
                        for env_name in envs_result {
                            if let Ok(Some(env)) = config.get_environment(&env_name).await {
                                environments.push(env);
                            }
                        }

                        let credentials =
                            config.list_credentials().await.map_err(|e| e.to_string())?;

                        let current = manager
                            .get_current_environment_name()
                            .await
                            .map_err(|e| e.to_string())?;

                        Ok(LoadedData {
                            environments,
                            credentials,
                            current_env: current,
                        })
                    },
                    Msg::DataLoaded,
                )
            }

            Msg::EnvironmentSaved(Err(err)) => {
                state.env_save_state = Resource::Failure(err.clone());
                state.recently_saved_env = None; // Clear on error
                log::error!("Failed to save environment: {}", err);
                Command::None
            }

            Msg::DeleteEnvironment => {
                if let Some(env_name) = state.env_selector.value() {
                    let env_name = env_name.to_string();
                    state.env_delete_state = Resource::Loading;

                    Command::perform(
                        async move {
                            let config = crate::global_config();
                            config
                                .delete_environment(&env_name)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        Msg::EnvironmentDeleted,
                    )
                } else {
                    Command::None
                }
            }

            Msg::EnvironmentDeleted(Ok(())) => {
                state.env_delete_state = Resource::Success(());
                state.env_selector.set_value(None);
                state.env_name_field.set_value(String::new());
                state.env_host_field.set_value(String::new());
                state.env_creds_selector.set_value(None);
                state.env_panel_dirty = false;

                // Reload data
                Command::perform(
                    async {
                        let config = crate::global_config();
                        let manager = crate::client_manager();

                        let envs_result = config
                            .list_environments()
                            .await
                            .map_err(|e| e.to_string())?;

                        let mut environments = Vec::new();
                        for env_name in envs_result {
                            if let Ok(Some(env)) = config.get_environment(&env_name).await {
                                environments.push(env);
                            }
                        }

                        let credentials =
                            config.list_credentials().await.map_err(|e| e.to_string())?;

                        let current = manager
                            .get_current_environment_name()
                            .await
                            .map_err(|e| e.to_string())?;

                        Ok(LoadedData {
                            environments,
                            credentials,
                            current_env: current,
                        })
                    },
                    Msg::DataLoaded,
                )
            }

            Msg::EnvironmentDeleted(Err(err)) => {
                state.env_delete_state = Resource::Failure(err.clone());
                log::error!("Failed to delete environment: {}", err);
                Command::None
            }

            Msg::CredSelectorEvent(event) => {
                let (cmd, selection) = state.cred_selector.handle_event(event, &state.credentials);

                if let Some(SelectEvent::Select(idx)) = selection {
                    if let Some(cred_name) = state.credentials.get(idx) {
                        let cred_name = cred_name.clone();
                        log::debug!("Credential selected: {}", cred_name);
                        // Load credential details
                        state.cred_name_field.set_value(cred_name.clone());

                        return Command::perform(
                            async move {
                                let config = crate::global_config();
                                log::debug!("Fetching credential from config: {}", cred_name);
                                config
                                    .get_credentials(&cred_name)
                                    .await
                                    .map_err(|e| e.to_string())?
                                    .ok_or_else(|| "Credential not found".to_string())
                            },
                            Msg::CredentialDataLoaded,
                        );
                    }
                }

                cmd
            }

            Msg::CredSelected(name) => {
                // Load credential details
                state.cred_name_field.set_value(name.clone());

                Command::perform(
                    async move {
                        let config = crate::global_config();
                        config
                            .get_credentials(&name)
                            .await
                            .map_err(|e| e.to_string())?
                            .ok_or_else(|| "Credential not found".to_string())
                    },
                    Msg::CredentialDataLoaded,
                )
            }

            Msg::CredentialDataLoaded(Ok(cred)) => {
                // Populate credential form based on type
                let type_str = State::credential_type_to_string(&cred);
                log::debug!("Loading credential of type: {}", type_str);
                state.cred_type_selector.set_value(Some(type_str.clone()));

                match cred {
                    CredentialSet::UsernamePassword {
                        username,
                        password,
                        client_id,
                        client_secret,
                    } => {
                        log::debug!(
                            "Setting Username/Password fields - username: {}, has_password: {}, client_id: {}, has_client_secret: {}",
                            username,
                            !password.is_empty(),
                            client_id,
                            !client_secret.is_empty()
                        );
                        state.cred_username_field.set_value(username);
                        state.cred_password_field.set_value(password);
                        state.cred_client_id_field.set_value(client_id);
                        state.cred_client_secret_field.set_value(client_secret);
                        state.cred_tenant_id_field.set_value(String::new());
                        state.cred_cert_path_field.set_value(String::new());
                    }
                    CredentialSet::ClientCredentials {
                        client_id,
                        client_secret,
                        tenant_id,
                    } => {
                        state.cred_username_field.set_value(String::new());
                        state.cred_password_field.set_value(String::new());
                        state.cred_client_id_field.set_value(client_id);
                        state.cred_client_secret_field.set_value(client_secret);
                        state.cred_tenant_id_field.set_value(tenant_id);
                        state.cred_cert_path_field.set_value(String::new());
                    }
                    CredentialSet::DeviceCode {
                        client_id,
                        tenant_id,
                    } => {
                        state.cred_username_field.set_value(String::new());
                        state.cred_password_field.set_value(String::new());
                        state.cred_client_id_field.set_value(client_id);
                        state.cred_client_secret_field.set_value(String::new());
                        state.cred_tenant_id_field.set_value(tenant_id);
                        state.cred_cert_path_field.set_value(String::new());
                    }
                    CredentialSet::Certificate {
                        client_id,
                        tenant_id,
                        cert_path,
                    } => {
                        state.cred_username_field.set_value(String::new());
                        state.cred_password_field.set_value(String::new());
                        state.cred_client_id_field.set_value(client_id);
                        state.cred_client_secret_field.set_value(String::new());
                        state.cred_tenant_id_field.set_value(tenant_id);
                        state.cred_cert_path_field.set_value(cert_path);
                    }
                }

                state.cred_panel_dirty = false;
                Command::None
            }

            Msg::CredentialDataLoaded(Err(err)) => {
                log::error!("Failed to load credential: {}", err);
                // Clear all credential fields on error
                state.cred_username_field.set_value(String::new());
                state.cred_password_field.set_value(String::new());
                state.cred_client_id_field.set_value(String::new());
                state.cred_client_secret_field.set_value(String::new());
                state.cred_tenant_id_field.set_value(String::new());
                state.cred_cert_path_field.set_value(String::new());
                // Show error in UI
                Command::start_app(
                    AppId::ErrorScreen,
                    ErrorScreenParams {
                        message: format!("Failed to load credential: {}", err),
                        target: Some(AppId::EnvironmentSelector),
                    },
                )
            }

            Msg::CredNameChanged(event) => {
                state.cred_name_field.handle_event(event, None);
                state.cred_panel_dirty = true;
                Command::None
            }

            Msg::CredTypeEvent(event) => {
                let types = State::get_credential_type_options();
                let (cmd, selection) = state.cred_type_selector.handle_event(event, &types);
                if selection.is_some() {
                    state.cred_panel_dirty = true;
                }
                cmd
            }

            Msg::CredUsernameChanged(event) => {
                state.cred_username_field.handle_event(event, None);
                state.cred_panel_dirty = true;
                Command::None
            }

            Msg::CredPasswordChanged(event) => {
                state.cred_password_field.handle_event(event, None);
                state.cred_panel_dirty = true;
                Command::None
            }

            Msg::CredClientIdChanged(event) => {
                state.cred_client_id_field.handle_event(event, None);
                state.cred_panel_dirty = true;
                Command::None
            }

            Msg::CredClientSecretChanged(event) => {
                state.cred_client_secret_field.handle_event(event, None);
                state.cred_panel_dirty = true;
                Command::None
            }

            Msg::CredTenantIdChanged(event) => {
                state.cred_tenant_id_field.handle_event(event, None);
                state.cred_panel_dirty = true;
                Command::None
            }

            Msg::CredCertPathChanged(event) => {
                state.cred_cert_path_field.handle_event(event, None);
                state.cred_panel_dirty = true;
                Command::None
            }

            Msg::NewCredential => {
                state.cred_selector.set_value(None);
                state.cred_name_field.set_value(String::new());
                state
                    .cred_type_selector
                    .set_value(Some("Username/Password".to_string()));
                state.cred_username_field.set_value(String::new());
                state.cred_password_field.set_value(String::new());
                state.cred_client_id_field.set_value(String::new());
                state.cred_client_secret_field.set_value(String::new());
                state.cred_tenant_id_field.set_value(String::new());
                state.cred_cert_path_field.set_value(String::new());
                state.cred_panel_dirty = true;
                Command::set_focus(FocusId::new("cred-name"))
            }

            Msg::SaveCredential => {
                let name = state.cred_name_field.value().to_string();
                let type_str = state
                    .cred_type_selector
                    .value()
                    .unwrap_or("Username/Password");

                if name.is_empty() {
                    state.cred_save_state =
                        Resource::Failure("Credential name is required".to_string());
                    return Command::None;
                }

                // Remember the name so we can select it after reload
                state.recently_saved_cred = Some(name.clone());

                let cred_set = match type_str {
                    "Username/Password" => CredentialSet::UsernamePassword {
                        username: state.cred_username_field.value().to_string(),
                        password: state.cred_password_field.value().to_string(),
                        client_id: state.cred_client_id_field.value().to_string(),
                        client_secret: state.cred_client_secret_field.value().to_string(),
                    },
                    "Client Credentials" => CredentialSet::ClientCredentials {
                        client_id: state.cred_client_id_field.value().to_string(),
                        client_secret: state.cred_client_secret_field.value().to_string(),
                        tenant_id: state.cred_tenant_id_field.value().to_string(),
                    },
                    "Device Code" => CredentialSet::DeviceCode {
                        client_id: state.cred_client_id_field.value().to_string(),
                        tenant_id: state.cred_tenant_id_field.value().to_string(),
                    },
                    "Certificate" => CredentialSet::Certificate {
                        client_id: state.cred_client_id_field.value().to_string(),
                        tenant_id: state.cred_tenant_id_field.value().to_string(),
                        cert_path: state.cred_cert_path_field.value().to_string(),
                    },
                    _ => {
                        state.cred_save_state =
                            Resource::Failure("Invalid credential type".to_string());
                        return Command::None;
                    }
                };

                state.cred_save_state = Resource::Loading;

                Command::perform(
                    async move {
                        let config = crate::global_config();
                        config
                            .add_credentials(name, cred_set)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Msg::CredentialSaved,
                )
            }

            Msg::CredentialSaved(Ok(())) => {
                state.cred_save_state = Resource::Success(());
                state.cred_panel_dirty = false;

                log::debug!("Credential saved successfully");

                // Reload data
                Command::perform(
                    async {
                        let config = crate::global_config();
                        let manager = crate::client_manager();

                        let envs_result = config
                            .list_environments()
                            .await
                            .map_err(|e| e.to_string())?;

                        let mut environments = Vec::new();
                        for env_name in envs_result {
                            if let Ok(Some(env)) = config.get_environment(&env_name).await {
                                environments.push(env);
                            }
                        }

                        let credentials =
                            config.list_credentials().await.map_err(|e| e.to_string())?;

                        let current = manager
                            .get_current_environment_name()
                            .await
                            .map_err(|e| e.to_string())?;

                        Ok(LoadedData {
                            environments,
                            credentials,
                            current_env: current,
                        })
                    },
                    Msg::DataLoaded,
                )
            }

            Msg::CredentialSaved(Err(err)) => {
                state.cred_save_state = Resource::Failure(err.clone());
                state.recently_saved_cred = None; // Clear on error
                log::error!("Failed to save credential: {}", err);
                Command::None
            }

            Msg::DeleteCredential => {
                if let Some(cred_name) = state.cred_selector.value() {
                    let cred_name = cred_name.to_string();
                    state.cred_delete_state = Resource::Loading;

                    Command::perform(
                        async move {
                            let config = crate::global_config();
                            config
                                .delete_credentials(&cred_name)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        Msg::CredentialDeleted,
                    )
                } else {
                    Command::None
                }
            }

            Msg::CredentialDeleted(Ok(())) => {
                state.cred_delete_state = Resource::Success(());
                state.cred_selector.set_value(None);
                state.cred_name_field.set_value(String::new());
                state
                    .cred_type_selector
                    .set_value(Some("Username/Password".to_string()));
                state.cred_username_field.set_value(String::new());
                state.cred_password_field.set_value(String::new());
                state.cred_client_id_field.set_value(String::new());
                state.cred_client_secret_field.set_value(String::new());
                state.cred_tenant_id_field.set_value(String::new());
                state.cred_cert_path_field.set_value(String::new());
                state.cred_panel_dirty = false;

                // Reload data
                Command::perform(
                    async {
                        let config = crate::global_config();
                        let manager = crate::client_manager();

                        let envs_result = config
                            .list_environments()
                            .await
                            .map_err(|e| e.to_string())?;

                        let mut environments = Vec::new();
                        for env_name in envs_result {
                            if let Ok(Some(env)) = config.get_environment(&env_name).await {
                                environments.push(env);
                            }
                        }

                        let credentials =
                            config.list_credentials().await.map_err(|e| e.to_string())?;

                        let current = manager
                            .get_current_environment_name()
                            .await
                            .map_err(|e| e.to_string())?;

                        Ok(LoadedData {
                            environments,
                            credentials,
                            current_env: current,
                        })
                    },
                    Msg::DataLoaded,
                )
            }

            Msg::CredentialDeleted(Err(err)) => {
                state.cred_delete_state = Resource::Failure(err.clone());
                log::error!("Failed to delete credential: {}", err);
                Command::None
            }

            Msg::SetCurrentEnvironment => {
                if let Some(env_name) = state.env_selector.value() {
                    let env_name = env_name.to_string();
                    state.set_current_state = Resource::Loading;

                    Command::perform(
                        async move {
                            let manager = crate::client_manager();
                            manager
                                .set_current_environment_in_config(env_name)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        Msg::CurrentEnvironmentSet,
                    )
                } else {
                    Command::None
                }
            }

            Msg::CurrentEnvironmentSet(Ok(())) => {
                state.set_current_state = Resource::Success(());

                // Reload data to update current environment indicator
                Command::perform(
                    async {
                        let config = crate::global_config();
                        let manager = crate::client_manager();

                        let envs_result = config
                            .list_environments()
                            .await
                            .map_err(|e| e.to_string())?;

                        let mut environments = Vec::new();
                        for env_name in envs_result {
                            if let Ok(Some(env)) = config.get_environment(&env_name).await {
                                environments.push(env);
                            }
                        }

                        let credentials =
                            config.list_credentials().await.map_err(|e| e.to_string())?;

                        let current = manager
                            .get_current_environment_name()
                            .await
                            .map_err(|e| e.to_string())?;

                        Ok(LoadedData {
                            environments,
                            credentials,
                            current_env: current,
                        })
                    },
                    Msg::DataLoaded,
                )
            }

            Msg::CurrentEnvironmentSet(Err(err)) => {
                state.set_current_state = Resource::Failure(err.clone());
                log::error!("Failed to set current environment: {}", err);
                Command::None
            }
        }
    }

    fn view(state: &mut State) -> LayeredView<Msg> {
        // Environment names for selector
        let theme = &crate::global_runtime_config().theme;
        let env_names: Vec<String> = state
            .environments
            .iter()
            .map(|e| {
                let mut name = e.name.clone();
                // Add indicator for current environment
                if Some(&e.name) == state.current_environment.as_ref() {
                    name = format!("● {}", name);
                }
                name
            })
            .collect();

        // Build environment panel
        let env_panel = build_environment_panel(state, &env_names);

        // Build credential panel
        let cred_panel = build_credential_panel(state);

        // Two-column layout (50/50 split using equal Fill weights)
        let main_content = row![
            env_panel => Fill(1),
            cred_panel => Fill(1)
        ];

        LayeredView::new(main_content)
    }

    fn subscriptions(_state: &State) -> Vec<Subscription<Msg>> {
        vec![]
    }

    fn title() -> &'static str {
        "Environment & Credential Configuration"
    }

    fn status(state: &State) -> Option<Line<'static>> {
        state.current_environment.as_ref().map(|env| {
            let theme = &crate::global_runtime_config().theme;
            Line::from(vec![
                Span::styled("Current: ", Style::default().fg(theme.text_tertiary)),
                Span::styled(env.clone(), Style::default().fg(theme.accent_success)),
            ])
        })
    }
}

// ============================================================================
// View Helpers
// ============================================================================

fn build_environment_panel<Msg: Clone + Send + 'static>(
    state: &mut State,
    env_names: &[String],
) -> Element<Msg>
where
    Msg: From<crate::tui::apps::environment_selector_app::Msg>,
{
    let theme = &crate::global_runtime_config().theme;
    use crate::tui::apps::environment_selector_app::Msg as AppMsg;

    // Form fields (wrapped in panels for labels)
    // Selector at top
    // Only show options if a value is selected, otherwise show empty to indicate "new"
    let env_select_options = if state.env_selector.value().is_some() {
        env_names.to_vec()
    } else {
        Vec::new()
    };
    let env_select = Element::select(
        "env-selector",
        env_select_options,
        &mut state.env_selector.state,
    )
    .on_event(|e| AppMsg::EnvSelectorEvent(e).into())
    .build();
    let env_select_panel = Element::panel(env_select)
        .title("Select Environment")
        .build();
    let name_input = Element::text_input(
        "env-name",
        state.env_name_field.value(),
        &state.env_name_field.state,
    )
    .on_event(|e| AppMsg::EnvNameChanged(e).into())
    .build();
    let name_panel = Element::panel(name_input).title("Name").build();

    let host_input = Element::text_input(
        "env-host",
        state.env_host_field.value(),
        &state.env_host_field.state,
    )
    .on_event(|e| AppMsg::EnvHostChanged(e).into())
    .build();
    let host_panel = Element::panel(host_input).title("Host").build();

    // Only show credentials options if a value is selected
    let creds_select_options = if state.env_creds_selector.value().is_some() {
        state.credentials.clone()
    } else {
        Vec::new()
    };
    let creds_select = Element::select(
        "env-creds",
        creds_select_options,
        &mut state.env_creds_selector.state,
    )
    .on_event(|e| AppMsg::EnvCredsEvent(e).into())
    .build();
    let creds_panel = Element::panel(creds_select).title("Credentials").build();

    // Action buttons
    let save_btn = if state.env_panel_dirty {
        Element::button("env-save-btn", "Save")
            .on_press(AppMsg::SaveEnvironment.into())
            .build()
    } else {
        Element::button("env-save-btn", "Save").build()
    };

    let delete_btn = if state.env_selector.value().is_some() {
        Element::button("env-delete-btn", "Delete")
            .on_press(AppMsg::DeleteEnvironment.into())
            .build()
    } else {
        Element::button("env-delete-btn", "Delete").build()
    };

    let new_btn = Element::button("env-new-btn", "New")
        .on_press(AppMsg::NewEnvironment.into())
        .build();

    let button_row = row![
        save_btn => Length(10),
        spacer!() => Length(1),
        delete_btn => Length(10),
        spacer!() => Length(1),
        new_btn => Length(10)
    ];

    let form_fields = col![
        env_select_panel => Length(3),
        name_panel => Length(3),
        host_panel => Length(3),
        creds_panel => Length(3),
        button_row => Length(3)
    ];

    let details_panel = Element::panel(form_fields)
        .title("Environment Details")
        .build();

    // Active environment selector
    let env_names: Vec<String> = state.environments.iter().map(|e| e.name.clone()).collect();

    let active_env_select = Element::select(
        "active-env-selector",
        env_names,
        &mut state.active_env_selector.state,
    )
    .on_event(|e| AppMsg::ActiveEnvEvent(e).into())
    .build();

    let active_env_panel = Element::panel(active_env_select)
        .title("Active Environment")
        .build();

    // Combine
    col![
        active_env_panel => Length(3),
        details_panel => Fill(1)
    ]
}

fn build_credential_panel<Msg: Clone + Send + 'static>(state: &mut State) -> Element<Msg>
where
    Msg: From<crate::tui::apps::environment_selector_app::Msg>,
{
    let theme = &crate::global_runtime_config().theme;
    use crate::tui::apps::environment_selector_app::Msg as AppMsg;
    use crate::tui::element::ColumnBuilder;

    // Form fields (wrapped in panels for labels)
    // Selector at top
    // Only show options if a value is selected, otherwise show empty to indicate "new"
    let cred_select_options = if state.cred_selector.value().is_some() {
        state.credentials.clone()
    } else {
        Vec::new()
    };
    let cred_select = Element::select(
        "cred-selector",
        cred_select_options,
        &mut state.cred_selector.state,
    )
    .on_event(|e| AppMsg::CredSelectorEvent(e).into())
    .build();
    let cred_select_panel = Element::panel(cred_select)
        .title("Select Credential Set")
        .build();
    let name_input = Element::text_input(
        "cred-name",
        state.cred_name_field.value(),
        &state.cred_name_field.state,
    )
    .on_event(|e| AppMsg::CredNameChanged(e).into())
    .build();
    let name_panel = Element::panel(name_input).title("Name").build();

    let type_select = Element::select(
        "cred-type",
        State::get_credential_type_options(),
        &mut state.cred_type_selector.state,
    )
    .on_event(|e| AppMsg::CredTypeEvent(e).into())
    .build();
    let type_panel = Element::panel(type_select).title("Type").build();

    // Build fields using ColumnBuilder to allow dynamic composition
    let mut builder = ColumnBuilder::new()
        .add(cred_select_panel, Length(3))
        .add(name_panel, Length(3))
        .add(type_panel, Length(3));

    let selected_type = state
        .cred_type_selector
        .value()
        .unwrap_or("Username/Password");

    match selected_type {
        "Username/Password" => {
            let username_input = Element::text_input(
                "cred-username",
                state.cred_username_field.value(),
                &state.cred_username_field.state,
            )
            .on_event(|e| AppMsg::CredUsernameChanged(e).into())
            .build();
            let username_panel = Element::panel(username_input).title("Username").build();

            let password_input = Element::text_input(
                "cred-password",
                state.cred_password_field.value(),
                &state.cred_password_field.state,
            )
            .masked(true)
            .on_event(|e| AppMsg::CredPasswordChanged(e).into())
            .build();
            let password_panel = Element::panel(password_input).title("Password").build();

            let client_id_input = Element::text_input(
                "cred-client-id",
                state.cred_client_id_field.value(),
                &state.cred_client_id_field.state,
            )
            .on_event(|e| AppMsg::CredClientIdChanged(e).into())
            .build();
            let client_id_panel = Element::panel(client_id_input).title("Client ID").build();

            let client_secret_input = Element::text_input(
                "cred-client-secret",
                state.cred_client_secret_field.value(),
                &state.cred_client_secret_field.state,
            )
            .masked(true)
            .on_event(|e| AppMsg::CredClientSecretChanged(e).into())
            .build();
            let client_secret_panel = Element::panel(client_secret_input)
                .title("Client Secret")
                .build();

            builder = builder
                .add(username_panel, Length(3))
                .add(password_panel, Length(3))
                .add(client_id_panel, Length(3))
                .add(client_secret_panel, Length(3));
        }
        "Client Credentials" | "Device Code" => {
            let client_id_input = Element::text_input(
                "cred-client-id",
                state.cred_client_id_field.value(),
                &state.cred_client_id_field.state,
            )
            .on_event(|e| AppMsg::CredClientIdChanged(e).into())
            .build();
            let client_id_panel = Element::panel(client_id_input).title("Client ID").build();

            let tenant_id_input = Element::text_input(
                "cred-tenant-id",
                state.cred_tenant_id_field.value(),
                &state.cred_tenant_id_field.state,
            )
            .on_event(|e| AppMsg::CredTenantIdChanged(e).into())
            .build();
            let tenant_id_panel = Element::panel(tenant_id_input).title("Tenant ID").build();

            builder = builder
                .add(client_id_panel, Length(3))
                .add(tenant_id_panel, Length(3));

            if selected_type == "Client Credentials" {
                let client_secret_input = Element::text_input(
                    "cred-client-secret",
                    state.cred_client_secret_field.value(),
                    &state.cred_client_secret_field.state,
                )
                .masked(true)
                .on_event(|e| AppMsg::CredClientSecretChanged(e).into())
                .build();
                let client_secret_panel = Element::panel(client_secret_input)
                    .title("Client Secret")
                    .build();

                builder = builder.add(client_secret_panel, Length(3));
            }
        }
        "Certificate" => {
            let client_id_input = Element::text_input(
                "cred-client-id",
                state.cred_client_id_field.value(),
                &state.cred_client_id_field.state,
            )
            .on_event(|e| AppMsg::CredClientIdChanged(e).into())
            .build();
            let client_id_panel = Element::panel(client_id_input).title("Client ID").build();

            let tenant_id_input = Element::text_input(
                "cred-tenant-id",
                state.cred_tenant_id_field.value(),
                &state.cred_tenant_id_field.state,
            )
            .on_event(|e| AppMsg::CredTenantIdChanged(e).into())
            .build();
            let tenant_id_panel = Element::panel(tenant_id_input).title("Tenant ID").build();

            let cert_path_input = Element::text_input(
                "cred-cert-path",
                state.cred_cert_path_field.value(),
                &state.cred_cert_path_field.state,
            )
            .on_event(|e| AppMsg::CredCertPathChanged(e).into())
            .build();
            let cert_path_panel = Element::panel(cert_path_input)
                .title("Certificate Path")
                .build();

            builder = builder
                .add(client_id_panel, Length(3))
                .add(tenant_id_panel, Length(3))
                .add(cert_path_panel, Length(3));
        }
        _ => {}
    }

    // Action buttons
    let save_btn = if state.cred_panel_dirty {
        Element::button("cred-save-btn", "Save")
            .on_press(AppMsg::SaveCredential.into())
            .build()
    } else {
        Element::button("cred-save-btn", "Save").build()
    };

    let delete_btn = if state.cred_selector.value().is_some() {
        Element::button("cred-delete-btn", "Delete")
            .on_press(AppMsg::DeleteCredential.into())
            .build()
    } else {
        Element::button("cred-delete-btn", "Delete").build()
    };

    let new_btn = Element::button("cred-new-btn", "New")
        .on_press(AppMsg::NewCredential.into())
        .build();

    let button_row = row![
        save_btn => Length(10),
        spacer!() => Length(1),
        delete_btn => Length(10),
        spacer!() => Length(1),
        new_btn => Length(10)
    ];

    builder = builder.add(button_row, Length(3));

    let details_panel = Element::panel(builder.build())
        .title("Credential Details")
        .build();

    // Combine
    col![
        details_panel => Fill(1)
    ]
}
