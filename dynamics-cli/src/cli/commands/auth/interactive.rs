//! Interactive menu system for authentication management

use crate::config::Config;
use anyhow::Result;
use colored::*;
use dialoguer::Select;

/// Clear the screen for a clean interactive experience
fn clear_screen() {
    print!("\x1B[2J\x1B[1;1H");
}

/// Pause and wait for user to press Enter
fn pause_for_user() {
    use std::io::{self, Write};
    print!("Press Enter to continue...");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
}

/// Main interactive menu options
#[derive(Debug)]
enum MainMenuOption {
    Status,
    Credentials,
    Environments,
    Exit,
}

impl std::fmt::Display for MainMenuOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MainMenuOption::Status => write!(
                f,
                "📊 {} - {}",
                "Status".bright_blue().bold(),
                "View current configuration".dimmed()
            ),
            MainMenuOption::Credentials => write!(
                f,
                "🔐 {} - {}",
                "Credentials".bright_yellow().bold(),
                "Manage credential sets".dimmed()
            ),
            MainMenuOption::Environments => write!(
                f,
                "🌍 {} - {}",
                "Environments".bright_green().bold(),
                "Manage environments".dimmed()
            ),
            MainMenuOption::Exit => write!(
                f,
                "🚪 {} - {}",
                "Exit".bright_red().bold(),
                "Leave authentication manager".dimmed()
            ),
        }
    }
}

/// Credential menu options
#[derive(Debug)]
enum CredentialMenuOption {
    List,
    Add,
    Test,
    Rename,
    Remove,
    Back,
}

impl std::fmt::Display for CredentialMenuOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialMenuOption::List => write!(
                f,
                "📋 {} - {}",
                "List".bright_blue().bold(),
                "View all credentials".dimmed()
            ),
            CredentialMenuOption::Add => write!(
                f,
                "➕ {} - {}",
                "Add".bright_green().bold(),
                "Create new credentials".dimmed()
            ),
            CredentialMenuOption::Test => write!(
                f,
                "🧪 {} - {}",
                "Test".bright_cyan().bold(),
                "Verify credential authentication".dimmed()
            ),
            CredentialMenuOption::Rename => write!(
                f,
                "✏️ {} - {}",
                "Rename".bright_yellow().bold(),
                "Change credential name".dimmed()
            ),
            CredentialMenuOption::Remove => write!(
                f,
                "🗑️ {} - {}",
                "Remove".bright_red().bold(),
                "Delete credentials".dimmed()
            ),
            CredentialMenuOption::Back => write!(
                f,
                "🔙 {} - {}",
                "Back".white().bold(),
                "Return to main menu".dimmed()
            ),
        }
    }
}

/// Environment menu options
#[derive(Debug)]
enum EnvironmentMenuOption {
    List,
    Add,
    SetCredentials,
    Select,
    Rename,
    Remove,
    Back,
}

impl std::fmt::Display for EnvironmentMenuOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvironmentMenuOption::List => write!(
                f,
                "📋 {} - {}",
                "List".bright_blue().bold(),
                "View all environments".dimmed()
            ),
            EnvironmentMenuOption::Add => write!(
                f,
                "➕ {} - {}",
                "Add".bright_green().bold(),
                "Create new environment".dimmed()
            ),
            EnvironmentMenuOption::SetCredentials => write!(
                f,
                "🔗 {} - {}",
                "Set Credentials".bright_yellow().bold(),
                "Change environment authentication".dimmed()
            ),
            EnvironmentMenuOption::Select => write!(
                f,
                "🎯 {} - {}",
                "Select".bright_cyan().bold(),
                "Choose current environment".dimmed()
            ),
            EnvironmentMenuOption::Rename => write!(
                f,
                "✏️ {} - {}",
                "Rename".bright_yellow().bold(),
                "Change environment name".dimmed()
            ),
            EnvironmentMenuOption::Remove => write!(
                f,
                "🗑️ {} - {}",
                "Remove".bright_red().bold(),
                "Delete environment".dimmed()
            ),
            EnvironmentMenuOption::Back => write!(
                f,
                "🔙 {} - {}",
                "Back".white().bold(),
                "Return to main menu".dimmed()
            ),
        }
    }
}

/// Run the main interactive menu
pub async fn run_main_menu() -> Result<()> {
    let client_manager = crate::client_manager();

    loop {
        clear_screen();
        println!();
        println!(
            "  {}",
            "🔧 Dynamics CLI - Authentication Manager"
                .bright_blue()
                .bold()
        );
        println!(
            "  {}",
            "═══════════════════════════════════════".bright_blue()
        );
        println!();

        // Show current status at the top
        let current_env = client_manager
            .get_current_environment_name()
            .await
            .unwrap_or_default();
        if let Some(env_name) = &current_env {
            println!(
                "  {} {}",
                "Current Environment:".dimmed(),
                env_name.bright_white().bold()
            );
        } else {
            println!(
                "  {} {}",
                "Current Environment:".dimmed(),
                "None selected".bright_red()
            );
        }
        println!();

        let options = vec![
            MainMenuOption::Status,
            MainMenuOption::Credentials,
            MainMenuOption::Environments,
            MainMenuOption::Exit,
        ];

        let selection = Select::new()
            .with_prompt("What would you like to do?")
            .items(&options)
            .default(0)
            .interact()?;

        match options[selection] {
            MainMenuOption::Status => {
                super::status::status_command().await.unwrap_or_else(|e| {
                    println!("Error: {}", e);
                });
                pause_for_user();
            }
            MainMenuOption::Credentials => {
                if let Err(e) = run_credentials_menu().await {
                    println!("Error: {}", e);
                    pause_for_user();
                }
            }
            MainMenuOption::Environments => {
                if let Err(e) = run_environments_menu().await {
                    println!("Error: {}", e);
                    pause_for_user();
                }
            }
            MainMenuOption::Exit => {
                println!("Goodbye!");
                break;
            }
        }
    }

    Ok(())
}

/// Run the credentials management menu
async fn run_credentials_menu() -> Result<()> {
    let client_manager = crate::client_manager();
    loop {
        clear_screen();
        println!();
        println!("  {}", "🔐 Credential Management".bright_yellow().bold());
        println!("  {}", "═══════════════════════".bright_yellow());
        println!();

        let options = vec![
            CredentialMenuOption::List,
            CredentialMenuOption::Add,
            CredentialMenuOption::Test,
            CredentialMenuOption::Rename,
            CredentialMenuOption::Remove,
            CredentialMenuOption::Back,
        ];

        let selection = Select::new()
            .with_prompt("Credential operations")
            .items(&options)
            .default(0)
            .interact()?;

        match options[selection] {
            CredentialMenuOption::List => {
                super::credentials::list_credentials_interactive()
                    .await
                    .unwrap_or_else(|e| {
                        println!("Error: {}", e);
                    });
                pause_for_user();
            }
            CredentialMenuOption::Add => {
                super::credentials::add_credentials_interactive()
                    .await
                    .unwrap_or_else(|e| {
                        println!("Error: {}", e);
                    });
                pause_for_user();
            }
            CredentialMenuOption::Test => {
                super::credentials::test_credentials_interactive()
                    .await
                    .unwrap_or_else(|e| {
                        println!("Error: {}", e);
                    });
                pause_for_user();
            }
            CredentialMenuOption::Rename => {
                super::credentials::rename_credentials_interactive()
                    .await
                    .unwrap_or_else(|e| {
                        println!("Error: {}", e);
                    });
                pause_for_user();
            }
            CredentialMenuOption::Remove => {
                super::credentials::remove_credentials_interactive()
                    .await
                    .unwrap_or_else(|e| {
                        println!("Error: {}", e);
                    });
                pause_for_user();
            }
            CredentialMenuOption::Back => {
                break;
            }
        }
    }

    Ok(())
}

/// Run the environments management menu
async fn run_environments_menu() -> Result<()> {
    let client_manager = crate::client_manager();
    loop {
        clear_screen();
        println!();
        println!("  {}", "🌍 Environment Management".bright_green().bold());
        println!("  {}", "═══════════════════════════".bright_green());
        println!();

        let options = vec![
            EnvironmentMenuOption::List,
            EnvironmentMenuOption::Add,
            EnvironmentMenuOption::SetCredentials,
            EnvironmentMenuOption::Select,
            EnvironmentMenuOption::Rename,
            EnvironmentMenuOption::Remove,
            EnvironmentMenuOption::Back,
        ];

        let selection = Select::new()
            .with_prompt("Environment operations")
            .items(&options)
            .default(0)
            .interact()?;

        match options[selection] {
            EnvironmentMenuOption::List => {
                super::environments::list_environments_interactive()
                    .await
                    .unwrap_or_else(|e| {
                        println!("Error: {}", e);
                    });
                pause_for_user();
            }
            EnvironmentMenuOption::Add => {
                super::environments::add_environment_interactive()
                    .await
                    .unwrap_or_else(|e| {
                        println!("Error: {}", e);
                    });
                pause_for_user();
            }
            EnvironmentMenuOption::SetCredentials => {
                super::environments::set_credentials_interactive()
                    .await
                    .unwrap_or_else(|e| {
                        println!("Error: {}", e);
                    });
                pause_for_user();
            }
            EnvironmentMenuOption::Select => {
                super::environments::select_environment_interactive()
                    .await
                    .unwrap_or_else(|e| {
                        println!("Error: {}", e);
                    });
                pause_for_user();
            }
            EnvironmentMenuOption::Rename => {
                super::environments::rename_environment_interactive()
                    .await
                    .unwrap_or_else(|e| {
                        println!("Error: {}", e);
                    });
                pause_for_user();
            }
            EnvironmentMenuOption::Remove => {
                super::environments::remove_environment_interactive()
                    .await
                    .unwrap_or_else(|e| {
                        println!("Error: {}", e);
                    });
                pause_for_user();
            }
            EnvironmentMenuOption::Back => {
                break;
            }
        }
    }

    Ok(())
}
