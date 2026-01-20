//! Theme color options registration

use crate::config::options::{OptionDefBuilder, OptionsRegistry};
use crate::tui::color::color_to_hex;
use crate::tui::state::theme::{COLOR_NAMES, Theme};
use anyhow::Result;

/// Register all theme-related options
pub fn register(registry: &OptionsRegistry) -> Result<()> {
    // Active theme control option
    registry.register(
        OptionDefBuilder::new("theme", "active")
            .display_name("Active Theme")
            .description("The currently active color theme")
            .string_type("mocha", Some(32))
            .build()?,
    )?;

    // Register built-in themes
    register_theme(registry, "mocha", &Theme::mocha())?;
    register_theme(registry, "latte", &Theme::latte())?;

    log::info!(
        "Registered {} theme options (1 control + 2 themes × 21 colors)",
        43
    );
    Ok(())
}

/// Register all 21 color options for a single theme
fn register_theme(registry: &OptionsRegistry, name: &str, theme: &Theme) -> Result<()> {
    let colors = theme.colors();

    for (color_name, color) in colors {
        let hex = color_to_hex(color);
        let description = get_color_description(color_name);

        registry.register(
            OptionDefBuilder::new("theme", &format!("{}.{}", name, color_name))
                .display_name(color_name)
                .description(description)
                .string_type(&hex, Some(7)) // "#RRGGBB"
                .build()?,
        )?;
    }

    Ok(())
}

/// Get human-readable description for each color semantic name
fn get_color_description(color_name: &str) -> &'static str {
    match color_name {
        // Accent colors
        "accent_primary" => "Focus, selection, primary highlight",
        "accent_secondary" => "Info, links, secondary actions",
        "accent_tertiary" => "Special emphasis, modal headers",
        "accent_error" => "Errors, failures, destructive actions",
        "accent_warning" => "Warnings, cautions, pending state",
        "accent_success" => "Success, completion, validation",
        "accent_info" => "Informational messages",
        "accent_muted" => "Labels, keys, subtle highlights",

        // Text hierarchy
        "text_primary" => "Main content text",
        "text_secondary" => "Less important content",
        "text_tertiary" => "Labels, hints, placeholders",

        // UI structure
        "border_primary" => "Main borders, separators",
        "border_secondary" => "Subtle borders, disabled state",
        "border_tertiary" => "Very subtle, scrollbar track",
        "bg_base" => "Main background",
        "bg_surface" => "Elevated surfaces, selection background",
        "bg_elevated" => "Modals, floating elements",

        // Flexible palette
        "palette_1" => "User customization slot 1",
        "palette_2" => "User customization slot 2",
        "palette_3" => "User customization slot 3",
        "palette_4" => "User customization slot 4",

        _ => "Theme color",
    }
}

/// List all registered theme names
pub fn list_themes(registry: &OptionsRegistry) -> Vec<String> {
    let theme_opts = registry.list_namespace("theme");

    // Extract unique theme names from keys like "theme.mocha.accent_primary"
    // Ignore "theme.active" which is the control option, not a theme
    let mut names = std::collections::HashSet::new();
    for opt_def in theme_opts {
        let parts: Vec<&str> = opt_def.key.split('.').collect();
        if parts.len() >= 3 {
            // Only include keys with 3+ parts (theme.NAME.color)
            names.insert(parts[1].to_string());
        }
    }

    let mut result: Vec<String> = names.into_iter().collect();
    result.sort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_names_coverage() {
        // Ensure we have descriptions for all 21 colors
        assert_eq!(COLOR_NAMES.len(), 21);

        for color_name in COLOR_NAMES {
            let desc = get_color_description(color_name);
            assert!(!desc.is_empty());
            assert_ne!(desc, "Theme color"); // All should have specific descriptions
        }
    }

    #[test]
    fn test_theme_registration_count() {
        // Each theme should register 21 colors
        let mocha = Theme::mocha();
        assert_eq!(mocha.colors().len(), 21);

        let latte = Theme::latte();
        assert_eq!(latte.colors().len(), 21);
    }
}
