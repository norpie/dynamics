//! LuaScriptApp - TUI app for managing Lua transform scripts

use std::path::PathBuf;

use crossterm::event::KeyCode;

use crate::config::repository::transfer::{get_transfer_config, save_transfer_config};
use crate::transfer::lua::{validate_script, ValidationResult};
use crate::transfer::TransferConfig;
use crate::tui::element::FocusId;
use crate::tui::resource::Resource;
use crate::tui::widgets::{FileBrowserEvent, FileBrowserAction};
use crate::tui::{App, AppId, Command, LayeredView, Subscription};

use super::state::{LuaScriptParams, Msg, State, StatusMessage};
use super::view;

pub struct LuaScriptApp;

impl crate::tui::AppState for State {}

impl App for LuaScriptApp {
    type State = State;
    type Msg = Msg;
    type InitParams = LuaScriptParams;

    fn init(params: LuaScriptParams) -> (State, Command<Msg>) {
        let mut state = State::default();
        state.config_name = params.config_name.clone();
        state.config = Resource::Loading;

        let config_name = params.config_name;
        let cmd = Command::perform(load_config(config_name), Msg::ConfigLoaded);

        (state, cmd)
    }

    fn update(state: &mut State, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::ConfigLoaded(result) => {
                match result {
                    Ok(config) => {
                        // If script is loaded, auto-validate
                        let should_validate = config.lua_script.is_some();
                        state.config = Resource::Success(config);
                        if should_validate {
                            state.validation = Resource::Loading;
                            let script = state.config.to_option()
                                .and_then(|c| c.lua_script.clone())
                                .unwrap_or_default();
                            return Command::perform(
                                async move { validate_script_async(script).await },
                                Msg::ValidationComplete,
                            );
                        }
                    }
                    Err(e) => {
                        state.config = Resource::Failure(e);
                    }
                }
                Command::None
            }

            Msg::OpenFileBrowser => {
                state.show_file_browser = true;
                // Auto-select first Lua file
                state.file_browser.select_first_matching(|e| e.name.to_lowercase().ends_with(".lua"));
                Command::set_focus(FocusId::new("file-browser"))
            }

            Msg::CloseFileBrowser => {
                state.show_file_browser = false;
                Command::None
            }

            Msg::FileBrowserNavigate(key) => {
                match key {
                    KeyCode::Up => {
                        state.file_browser.navigate_up();
                    }
                    KeyCode::Down => {
                        state.file_browser.navigate_down();
                    }
                    KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End => {
                        state.file_browser.handle_navigation_key(key);
                    }
                    KeyCode::Enter => {
                        if let Some(action) = state.file_browser.handle_event(FileBrowserEvent::Activate) {
                            match action {
                                FileBrowserAction::FileSelected(path) => {
                                    return Command::perform(
                                        async move { path },
                                        Msg::FileSelected,
                                    );
                                }
                                FileBrowserAction::DirectoryEntered(path) => {
                                    return Command::perform(
                                        async move { path },
                                        Msg::DirectoryEntered,
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        let _ = state.file_browser.go_to_parent();
                    }
                    _ => {}
                }
                Command::None
            }

            Msg::FileSelected(path) => {
                state.show_file_browser = false;
                let path_str = path.to_string_lossy().to_string();
                
                // Update config with script path
                if let Resource::Success(config) = &mut state.config {
                    config.lua_script_path = Some(path_str);
                }
                
                // Load the script file
                Command::perform(
                    load_script_file(path),
                    Msg::ScriptLoaded,
                )
            }

            Msg::DirectoryEntered(_path) => {
                // Auto-select first Lua file after entering directory
                state.file_browser.select_first_matching(|e| e.name.to_lowercase().ends_with(".lua"));
                Command::None
            }

            Msg::SetViewportHeight(height) => {
                let item_count = state.file_browser.entries().len();
                let list_state = state.file_browser.list_state_mut();
                list_state.set_viewport_height(height);
                list_state.update_scroll(height, item_count);
                Command::None
            }

            Msg::ScriptLoaded(result) => {
                match result {
                    Ok(content) => {
                        // Update config with script content
                        if let Resource::Success(config) = &mut state.config {
                            config.lua_script = Some(content.clone());
                            // Save the config
                            let config_clone = config.clone();
                            state.status_message = Some(StatusMessage::info("Script loaded, saving..."));
                            return Command::perform(
                                save_config(config_clone),
                                Msg::ScriptSaved,
                            );
                        }
                    }
                    Err(e) => {
                        state.status_message = Some(StatusMessage::error(format!("Failed to load script: {}", e)));
                    }
                }
                Command::None
            }

            Msg::ScriptSaved(result) => {
                match result {
                    Ok(()) => {
                        state.status_message = Some(StatusMessage::info("Script saved. Validating..."));
                        // Auto-validate after saving
                        state.validation = Resource::Loading;
                        let script = state.config.to_option()
                            .and_then(|c| c.lua_script.clone())
                            .unwrap_or_default();
                        return Command::perform(
                            async move { validate_script_async(script).await },
                            Msg::ValidationComplete,
                        );
                    }
                    Err(e) => {
                        state.status_message = Some(StatusMessage::error(format!("Failed to save: {}", e)));
                    }
                }
                Command::None
            }

            Msg::Validate => {
                if let Resource::Success(config) = &state.config {
                    if let Some(script) = &config.lua_script {
                        state.validation = Resource::Loading;
                        let script = script.clone();
                        return Command::perform(
                            async move { validate_script_async(script).await },
                            Msg::ValidationComplete,
                        );
                    } else {
                        state.status_message = Some(StatusMessage::error("No script to validate"));
                    }
                }
                Command::None
            }

            Msg::ValidationComplete(result) => {
                match result {
                    Ok(validation_result) => {
                        let is_valid = validation_result.is_valid;
                        state.validation = Resource::Success(validation_result);
                        if is_valid {
                            state.status_message = Some(StatusMessage::info("Validation passed"));
                        } else {
                            state.status_message = Some(StatusMessage::error("Validation failed"));
                        }
                    }
                    Err(e) => {
                        state.validation = Resource::Failure(e.clone());
                        state.status_message = Some(StatusMessage::error(format!("Validation error: {}", e)));
                    }
                }
                Command::None
            }

            Msg::StartPreview => {
                // Check if valid first
                if let Resource::Success(validation) = &state.validation {
                    if !validation.is_valid {
                        state.status_message = Some(StatusMessage::error("Cannot preview: script has validation errors"));
                        return Command::None;
                    }
                } else {
                    state.status_message = Some(StatusMessage::error("Please validate the script first"));
                    return Command::None;
                }

                // Navigate to preview app
                if let Resource::Success(config) = &state.config {
                    use crate::tui::apps::transfer::PreviewParams;
                    return Command::start_app(
                        AppId::TransferPreview,
                        PreviewParams {
                            config_name: config.name.clone(),
                            source_env: config.source_env.clone(),
                            target_env: config.target_env.clone(),
                        },
                    );
                }
                Command::None
            }

            Msg::GoBack => {
                Command::navigate_to(AppId::TransferConfigList)
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
        "Lua Script Editor"
    }
}

// Async helper functions

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

async fn load_script_file(path: PathBuf) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read file: {}", e))
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}

async fn validate_script_async(script: String) -> Result<ValidationResult, String> {
    tokio::task::spawn_blocking(move || {
        Ok(validate_script(&script))
    })
    .await
    .map_err(|e| format!("Validation task failed: {}", e))?
}
