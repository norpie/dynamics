//! Update-related options registration

use crate::config::options::{OptionDefBuilder, OptionsRegistry};
use anyhow::Result;

/// Register all update-related options
pub fn register(registry: &OptionsRegistry) -> Result<()> {
    // Auto-check option
    registry.register(
        OptionDefBuilder::new("update", "auto_check")
            .display_name("Auto-Check")
            .description("Automatically check for updates every hour")
            .bool_type(false)
            .build()?,
    )?;

    // Auto-install option
    registry.register(
        OptionDefBuilder::new("update", "auto_install")
            .display_name("Auto-Install")
            .description("Automatically install updates when found")
            .bool_type(false)
            .build()?,
    )?;

    log::info!("Registered {} update options", 2);
    Ok(())
}
