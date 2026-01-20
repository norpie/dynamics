//! State and messages for LuaScriptApp

use std::path::PathBuf;

use crossterm::event::KeyCode;

use crate::transfer::TransferConfig;
use crate::transfer::lua::ValidationResult;
use crate::tui::resource::Resource;
use crate::tui::widgets::FileBrowserState;

/// Parameters to initialize the Lua script editor
#[derive(Clone, Debug)]
pub struct LuaScriptParams {
    pub config_name: String,
}

impl Default for LuaScriptParams {
    fn default() -> Self {
        Self {
            config_name: String::new(),
        }
    }
}

/// State for the Lua script editor
pub struct State {
    /// Name of the transfer config being edited
    pub config_name: String,
    /// The transfer config (loaded from DB)
    pub config: Resource<TransferConfig>,
    /// Validation result from last validation
    pub validation: Resource<ValidationResult>,

    // File browser
    pub show_file_browser: bool,
    pub file_browser: FileBrowserState,

    // Status
    pub status_message: Option<StatusMessage>,
}

impl Default for State {
    fn default() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let mut file_browser = FileBrowserState::new(current_dir);
        // Filter to show only Lua files and directories
        file_browser.set_filter(is_lua_or_dir);

        Self {
            config_name: String::new(),
            config: Resource::NotAsked,
            validation: Resource::NotAsked,
            show_file_browser: false,
            file_browser,
            status_message: None,
        }
    }
}

/// Filter to show only Lua files and directories
fn is_lua_or_dir(entry: &crate::tui::widgets::FileBrowserEntry) -> bool {
    entry.is_dir || entry.name.to_lowercase().ends_with(".lua")
}

/// Status message displayed in the UI
#[derive(Clone, Debug)]
pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
}

impl StatusMessage {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: true,
        }
    }
}

/// Messages for the Lua script editor
#[derive(Clone)]
pub enum Msg {
    // Config loading
    ConfigLoaded(Result<TransferConfig, String>),

    // File browser
    OpenFileBrowser,
    CloseFileBrowser,
    FileBrowserNavigate(KeyCode),
    FileSelected(PathBuf),
    DirectoryEntered(PathBuf),
    SetViewportHeight(usize),

    // Script operations
    ScriptLoaded(Result<String, String>),
    ScriptSaved(Result<(), String>),

    // Validation
    Validate,
    ValidationComplete(Result<ValidationResult, String>),

    // Preview
    StartPreview,

    // Navigation
    GoBack,
}
