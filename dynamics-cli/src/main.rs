#![allow(warnings)]

use anyhow::Result;
use arc_swap::ArcSwap;
use clap::Parser;
use log::{debug, info};
use once_cell::sync::OnceCell;
use std::sync::Arc;

mod api;
mod auth;
mod cli;
// mod commands; // Disabled during config rewrite
mod config;
mod cs_parser;
mod csv_parser;
// mod dynamics; // Disabled during config rewrite
mod fql;
mod services;
mod transfer;
mod tui;
mod ui;
mod update;

// Global ClientManager instance
static CLIENT_MANAGER: OnceCell<api::ClientManager> = OnceCell::new();

/// Get a reference to the global ClientManager
pub fn client_manager() -> &'static api::ClientManager {
    CLIENT_MANAGER.get().expect("ClientManager not initialized")
}

// Global Config instance
static CONFIG: OnceCell<config::Config> = OnceCell::new();

/// Get a reference to the global Config
pub fn global_config() -> &'static config::Config {
    CONFIG.get().expect("Config not initialized")
}

// Global Options Registry (wrapped in Arc for sharing)
static OPTIONS_REGISTRY: OnceCell<Arc<config::options::OptionsRegistry>> = OnceCell::new();

/// Get a reference to the global OptionsRegistry Arc
pub fn options_registry() -> Arc<config::options::OptionsRegistry> {
    OPTIONS_REGISTRY
        .get()
        .expect("OptionsRegistry not initialized")
        .clone()
}

// Global RuntimeConfig instance (using ArcSwap for lock-free atomic updates)
static RUNTIME_CONFIG: OnceCell<ArcSwap<tui::state::RuntimeConfig>> = OnceCell::new();

/// Get a clone of the current RuntimeConfig Arc
pub fn global_runtime_config() -> Arc<tui::state::RuntimeConfig> {
    RUNTIME_CONFIG
        .get()
        .expect("RuntimeConfig not initialized")
        .load_full()
}

/// Initialize the global RuntimeConfig (called once at startup)
pub fn init_runtime_config(config: tui::state::RuntimeConfig) {
    RUNTIME_CONFIG
        .set(ArcSwap::from_pointee(config))
        .expect("RuntimeConfig already initialized");
}

/// Reload the global RuntimeConfig (called when settings change)
pub fn reload_runtime_config(config: tui::state::RuntimeConfig) {
    RUNTIME_CONFIG
        .get()
        .expect("RuntimeConfig not initialized")
        .store(Arc::new(config));
}

use cli::Cli;
// use cli::app::Commands;
// use cli::commands::auth::AuthSubcommands;
// use cli::commands::entity::EntitySubcommands;
// use cli::commands::query::QuerySubcommands;
// use cli::commands::settings::SettingsSubcommands;
// use commands::auth::{SetupOptions, remove_command, select_command, setup_command, status_command};
// #[cfg(feature = "deadlines")]
// use commands::deadlines;
// use commands::entity::{
//     add_command, list_command, remove_command as entity_remove_command, update_command,
// };
// #[cfg(feature = "migration")]
// use commands::migration;
// use commands::query::{file_command, run_command};
// use commands::settings::{
//     get_command, list_mappings_command, reset_all_command, reset_command, set_command, show_command,
// };

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger to file (truncate on each run)
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("dynamics-cli.log")?;
    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    let cli = Cli::parse();
    info!("Starting dynamics-cli");

    // Initialize global OptionsRegistry first (needed by Config)
    let registry = config::options::OptionsRegistry::new();
    config::options::registrations::register_all(&registry)?;
    let registry_arc = Arc::new(registry);
    let count = registry_arc.count();
    OPTIONS_REGISTRY
        .set(registry_arc)
        .map_err(|_| anyhow::anyhow!("Failed to initialize global OptionsRegistry"))?;
    debug!("Initialized options registry with {} options", count);

    // Initialize global Config once
    let config = config::Config::load().await?;
    CONFIG
        .set(config)
        .map_err(|_| anyhow::anyhow!("Failed to initialize global Config"))?;

    // Initialize global RuntimeConfig from options
    let runtime_config = tui::state::RuntimeConfig::load_from_options().await?;
    init_runtime_config(runtime_config);
    debug!("Initialized runtime config from options");

    // Initialize global ClientManager once
    let client_manager = api::ClientManager::new().await?;
    CLIENT_MANAGER
        .set(client_manager)
        .map_err(|_| anyhow::anyhow!("Failed to initialize global ClientManager"))?;

    // Handle commands
    use cli::app::Commands;
    match cli.command {
        Commands::Auth(auth_args) => {
            cli::commands::auth_command(auth_args).await?;
        }
        Commands::Query(query_args) => {
            cli::commands::handle_query_command(query_args).await?;
        }
        Commands::Raw(raw_args) => {
            cli::commands::handle_raw_command(raw_args).await?;
        }
        Commands::Tui(tui_args) => {
            cli::commands::tui_command(tui_args).await?;
        }
        Commands::Update(update_args) => {
            cli::commands::handle_update_command(update_args).await?;
        }
        _ => {
            println!("Some commands are temporarily disabled during the config system rewrite.");
            println!("Available commands: auth, query, raw, tui, update");
            println!("Use --help with any command for more information.");
        }
    }

    Ok(())
}
