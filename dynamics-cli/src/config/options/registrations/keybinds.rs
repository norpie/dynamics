//! Keybind options registration

use crate::config::options::{OptionDefBuilder, OptionsRegistry};
use crate::tui::KeyBinding;
use anyhow::Result;
use crossterm::event::KeyCode;

/// List all apps that have registered keybinds
pub fn list_apps(registry: &OptionsRegistry) -> Vec<String> {
    let keybind_opts = registry.list_namespace("keybind");
    let mut apps = std::collections::HashSet::new();

    for opt_def in keybind_opts {
        let parts: Vec<&str> = opt_def.key.split('.').collect();
        if parts.len() >= 3 {
            // Only include keys with 3+ parts (keybind.APP.action)
            apps.insert(parts[1].to_string());
        }
    }

    let mut sorted: Vec<String> = apps.into_iter().collect();
    sorted.sort();
    sorted
}

/// Get all action names for a specific app
pub fn list_actions_for_app(registry: &OptionsRegistry, app: &str) -> Vec<String> {
    let keybind_opts = registry.list_namespace("keybind");

    keybind_opts
        .into_iter()
        .filter_map(|opt_def| {
            let parts: Vec<&str> = opt_def.key.split('.').collect();
            if parts.len() >= 3 && parts[1] == app {
                Some(parts[2].to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Get human-readable description for each action by reading from registry
pub fn get_action_description(registry: &OptionsRegistry, app: &str, action: &str) -> String {
    let key = format!("keybind.{}.{}", app, action);
    registry
        .get(&key)
        .map(|def| def.description.clone())
        .unwrap_or_else(|| "Keybind action".to_string())
}

/// Get display name for each action by reading from registry
pub fn get_action_display_name(registry: &OptionsRegistry, app: &str, action: &str) -> String {
    let key = format!("keybind.{}.{}", app, action);
    registry
        .get(&key)
        .map(|def| def.display_name.clone())
        .unwrap_or_else(|| "Unknown Action".to_string())
}

/// Get all actions across all apps (for UI compatibility)
/// Returns actions in "app.action" format
pub fn list_all_actions(registry: &OptionsRegistry) -> Vec<String> {
    let apps = list_apps(registry);
    let mut all_actions = Vec::new();

    for app in apps {
        let actions = list_actions_for_app(registry, &app);
        for action in actions {
            all_actions.push(format!("{}.{}", app, action));
        }
    }

    all_actions.sort();
    all_actions
}

/// Register all keybind-related options
pub fn register(registry: &OptionsRegistry) -> Result<()> {
    // Global keybinds
    registry.register(
        OptionDefBuilder::new("keybind", "global.help")
            .display_name("Help Menu")
            .description("Toggle help menu showing all keyboard shortcuts")
            .keybind_type(KeyCode::F(1))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "global.app_launcher")
            .display_name("App Launcher")
            .description("Open the app launcher to switch between apps")
            .keybind_type(KeyBinding::ctrl(KeyCode::Char('a')))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "global.app_overview")
            .display_name("App Overview")
            .description("Show overview of all running apps and their states")
            .keybind_type(KeyBinding::ctrl(KeyCode::Char('o')))
            .build()?,
    )?;

    // Migration Environment app keybinds
    registry.register(
        OptionDefBuilder::new("keybind", "migration_env.create")
            .display_name("Create Migration")
            .description("Create a new migration environment")
            .keybind_type(KeyCode::Char('n'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "migration_env.delete")
            .display_name("Delete Migration")
            .description("Delete the selected migration environment")
            .keybind_type(KeyCode::Char('d'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "migration_env.rename")
            .display_name("Rename Migration")
            .description("Rename the selected migration environment")
            .keybind_type(KeyCode::Char('r'))
            .build()?,
    )?;

    // Migration Comparison Select app keybinds
    registry.register(
        OptionDefBuilder::new("keybind", "migration_comparison.create")
            .display_name("Create Comparison")
            .description("Create a new entity comparison")
            .keybind_type(KeyCode::Char('n'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "migration_comparison.delete")
            .display_name("Delete Comparison")
            .description("Delete the selected comparison")
            .keybind_type(KeyCode::Char('d'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "migration_comparison.rename")
            .display_name("Rename Comparison")
            .description("Rename the selected comparison")
            .keybind_type(KeyCode::Char('r'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "migration_comparison.back")
            .display_name("Back to Migrations")
            .description("Return to migration environment list")
            .keybind_type(KeyCode::Char('b'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "migration_comparison.preload")
            .display_name("Preload All")
            .description("Preload metadata for all comparisons")
            .keybind_type(KeyCode::Char('P'))
            .build()?,
    )?;

    // Entity Comparison app keybinds
    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.back")
            .display_name("Back to Comparisons")
            .description("Return to comparison list")
            .keybind_type(KeyCode::Char('b'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.tab_fields")
            .display_name("Switch to Fields Tab")
            .description("Switch to the Fields comparison tab")
            .keybind_type(KeyCode::Char('1'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.tab_relationships")
            .display_name("Switch to Relationships Tab")
            .description("Switch to the Relationships comparison tab")
            .keybind_type(KeyCode::Char('2'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.tab_views")
            .display_name("Switch to Views Tab")
            .description("Switch to the Views comparison tab")
            .keybind_type(KeyCode::Char('3'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.tab_forms")
            .display_name("Switch to Forms Tab")
            .description("Switch to the Forms comparison tab")
            .keybind_type(KeyCode::Char('4'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.tab_entities")
            .display_name("Switch to Entities Tab")
            .description("Switch to the Entities comparison tab")
            .keybind_type(KeyCode::Char('5'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.refresh")
            .display_name("Refresh Metadata")
            .description("Reload metadata from API")
            .keybind_type(KeyCode::F(5))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.create_mapping")
            .display_name("Create Manual Mapping")
            .description("Create a manual field mapping")
            .keybind_type(KeyCode::Char('m'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.delete_mapping")
            .display_name("Delete Manual Mapping")
            .description("Delete a manual field mapping")
            .keybind_type(KeyCode::Char('d'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.toggle_hide_matched")
            .display_name("Toggle Hide Matched")
            .description("Show/hide already matched fields")
            .keybind_type(KeyCode::Char('h'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.toggle_sort")
            .display_name("Toggle Sort Mode")
            .description("Cycle through sort modes")
            .keybind_type(KeyCode::Char('s'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.toggle_technical_names")
            .display_name("Toggle Technical Names")
            .description("Switch between technical and display names")
            .keybind_type(KeyCode::Char('n'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.toggle_mirror")
            .display_name("Toggle Mirror Mode")
            .description("Cycle through mirror modes (Off/Source/Target)")
            .keybind_type(KeyCode::Char('r'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.cycle_source_type_filter")
            .display_name("Cycle Source Type Filter")
            .description("Cycle through source field types (All -> String -> Integer -> ...)")
            .keybind_type(KeyCode::Char('t'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.cycle_target_type_filter")
            .display_name("Cycle Target Type Filter")
            .description("Cycle through target field types (All -> String -> Integer -> ...)")
            .keybind_type(KeyCode::Char('T'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.toggle_type_filter_mode")
            .display_name("Toggle Type Filter Mode")
            .description("Toggle between Unified and Independent type filter modes")
            .keybind_type(KeyCode::Char('Y'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.cycle_example")
            .display_name("Cycle Example Pairs")
            .description("Cycle through configured example data pairs")
            .keybind_type(KeyCode::Char('e'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.open_examples")
            .display_name("Manage Examples")
            .description("Open examples management modal")
            .keybind_type(KeyCode::Char('x'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.open_prefix_mappings")
            .display_name("Manage Prefix Mappings")
            .description("Open prefix mappings modal")
            .keybind_type(KeyCode::Char('p'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.open_manual_mappings")
            .display_name("View Manual Mappings")
            .description("Open manual mappings modal")
            .keybind_type(KeyCode::Char('M'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.import_cs")
            .display_name("Import C# Mappings")
            .description("Import field mappings from C# file")
            .keybind_type(KeyCode::Char('c'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.ignore_item")
            .display_name("Ignore Item")
            .description("Ignore currently selected item")
            .keybind_type(KeyCode::Char('i'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.ignore_manager")
            .display_name("Ignore Manager")
            .description("Open ignore manager modal")
            .keybind_type(KeyCode::Char('I'))
            .build()?,
    )?;

    registry.register(
        OptionDefBuilder::new("keybind", "entity_comparison.export")
            .display_name("Export to Excel")
            .description("Export comparison data to Excel file")
            .keybind_type(KeyCode::F(10))
            .build()?,
    )?;

    log::info!(
        "Registered keybind options for {} apps",
        list_apps(registry).len()
    );
    Ok(())
}
