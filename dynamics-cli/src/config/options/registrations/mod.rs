//! Registration of all application options

pub mod api;
pub mod keybinds;
pub mod keys;
pub mod themes;
pub mod tui;
pub mod update;

use super::OptionsRegistry;
use anyhow::Result;

/// Register all options from all modules
pub fn register_all(registry: &OptionsRegistry) -> Result<()> {
    api::register(registry)?;
    tui::register(registry)?;
    themes::register(registry)?;
    keybinds::register(registry)?;
    keys::register(registry)?;
    update::register(registry)?;
    Ok(())
}
