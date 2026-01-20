//! Environment management operations

use super::EnvironmentCommands;
use crate::api::models::Environment;
use crate::config::Config;
use anyhow::Result;
use colored::*;
use dialoguer::{Confirm, Input, Select};

/// Handle non-interactive environment commands
pub async fn handle_environment_command(cmd: EnvironmentCommands) -> Result<()> {
    let client_manager = crate::client_manager();

    match cmd {
        EnvironmentCommands::Add {
            name,
            host,
            credentials,
            set_current,
        } => add_environment_noninteractive(name, host, credentials, set_current).await,
        EnvironmentCommands::List => list_environments_interactive().await,
        EnvironmentCommands::SetCredentials { name, credentials } => {
            set_credentials_by_name(&name, &credentials).await
        }
        EnvironmentCommands::Select { name } => {
            if let Some(name) = name {
                select_environment_by_name(&name).await
            } else {
                select_environment_interactive().await
            }
        }
        EnvironmentCommands::Rename { old_name, new_name } => {
            rename_environment_noninteractive(&old_name, new_name).await
        }
        EnvironmentCommands::Remove { name, force } => {
            remove_environment_by_name(&name, force).await
        }
    }
}

/// Add environment non-interactively (CLI args)
async fn add_environment_noninteractive(
    name: String,
    host: String,
    credentials: String,
    set_current: bool,
) -> Result<()> {
    let client_manager = crate::client_manager();
    // Validate that credentials exist
    if client_manager
        .get_credentials(&credentials)
        .await?
        .is_none()
    {
        anyhow::bail!(
            "Credentials '{}' not found. Create them first with 'dynamics-cli auth creds add'",
            credentials
        );
    }

    let environment = Environment {
        name: name.clone(),
        host,
        credentials_ref: credentials,
    };

    client_manager
        .add_environment_to_config(name.clone(), environment)
        .await?;
    println!(
        "{} Environment '{}' added successfully",
        "✓".bright_green().bold(),
        name.bright_green().bold()
    );

    if set_current {
        client_manager
            .set_current_environment_in_config(name.clone())
            .await?;
        println!(
            "{} Set '{}' as current environment",
            "✓".bright_green().bold(),
            name.bright_green().bold()
        );
    }

    Ok(())
}

/// List environments (works for both interactive and non-interactive)
pub async fn list_environments_interactive() -> Result<()> {
    let client_manager = crate::client_manager();
    let environments = client_manager.list_environments().await;
    let current_env = client_manager.get_current_environment_name().await?;

    if environments.is_empty() {
        println!(
            "  {}",
            "⚠️  No environments configured".bright_yellow().bold()
        );
        println!("  {}", "Add an environment to get started.".dimmed());
        return Ok(());
    }

    println!();
    println!("  {}", "Configured environments:".bright_white().bold());
    for env_name in &environments {
        if let Some(environment) = client_manager.get_environment(env_name).await? {
            let (marker, env_color, current_text) =
                if current_env.as_ref() == Some(&env_name.to_string()) {
                    (
                        "●",
                        env_name.bright_green().bold(),
                        " (current)".bright_green(),
                    )
                } else {
                    ("○", env_name.white(), "".white())
                };
            println!(
                "  {} {} → {} ({}){}",
                marker.bright_green(),
                env_color,
                environment.host.cyan(),
                environment.credentials_ref.bright_yellow(),
                current_text
            );
        }
    }
    println!();

    Ok(())
}

/// Add environment interactively
pub async fn add_environment_interactive() -> Result<()> {
    let client_manager = crate::client_manager();
    println!();
    println!("Add New Environment");
    println!("==================");

    // Get environment name
    let name: String = Input::new()
        .with_prompt("Environment name (e.g., 'production', 'staging')")
        .interact()?;

    // Check if exists
    if client_manager.get_environment(&name).await?.is_some() {
        let overwrite = Confirm::new()
            .with_prompt(format!("Environment '{}' already exists. Overwrite?", name))
            .default(false)
            .interact()?;

        if !overwrite {
            println!("{} Cancelled.", "❌".bright_red().bold());
            return Ok(());
        }
    }

    // Get host URL
    let host: String = Input::new()
        .with_prompt("Host URL (e.g., https://yourorg.crm.dynamics.com)")
        .interact()?;

    // Select credentials
    let credentials_list = client_manager.list_credentials().await?;
    if credentials_list.is_empty() {
        println!(
            "  {} No credentials configured. Please add credentials first.",
            "⚠️".bright_yellow().bold()
        );
        return Ok(());
    }

    let cred_selection = Select::new()
        .with_prompt("Select credentials to use")
        .items(&credentials_list)
        .default(0)
        .interact()?;

    let credentials_ref = credentials_list[cred_selection].clone();

    // Ask if this should be the current environment
    let set_current = if client_manager
        .get_current_environment_name()
        .await?
        .is_none()
    {
        // No current environment, default to yes
        Confirm::new()
            .with_prompt("Set as current environment?")
            .default(true)
            .interact()?
    } else {
        Confirm::new()
            .with_prompt("Set as current environment?")
            .default(false)
            .interact()?
    };

    // Create and save environment
    let environment = Environment {
        name: name.clone(),
        host,
        credentials_ref,
    };

    client_manager
        .add_environment_to_config(name.clone(), environment)
        .await?;
    println!(
        "{} Environment '{}' saved successfully",
        "✓".bright_green().bold(),
        name.bright_green().bold()
    );

    if set_current {
        client_manager
            .set_current_environment_in_config(name.clone())
            .await?;
        println!(
            "{} Set '{}' as current environment",
            "✓".bright_green().bold(),
            name.bright_green().bold()
        );
    }

    Ok(())
}

/// Select environment interactively
pub async fn select_environment_interactive() -> Result<()> {
    let client_manager = crate::client_manager();
    let environments = client_manager.list_environments().await;
    let current_env = client_manager.get_current_environment_name().await?;

    if environments.is_empty() {
        println!(
            "  {} No environments configured to select.",
            "⚠️".bright_yellow().bold()
        );
        return Ok(());
    }

    println!();

    // Build display items with current marker
    let mut display_items = Vec::new();
    for env_name in &environments {
        if current_env.as_ref() == Some(&env_name.to_string()) {
            display_items.push(format!("{} (current)", env_name));
        } else {
            display_items.push(env_name.to_string());
        }
    }

    let env_selection = Select::new()
        .with_prompt("Select environment")
        .items(&display_items)
        .default(0)
        .interact()?;

    let selected_env = environments[env_selection].clone();

    select_environment_by_name(&selected_env).await
}

/// Select environment by name
async fn select_environment_by_name(name: &str) -> Result<()> {
    let client_manager = crate::client_manager();
    // Validate environment exists
    if client_manager.get_environment(name).await?.is_none() {
        anyhow::bail!("Environment '{}' not found", name);
    }

    client_manager
        .set_current_environment_in_config(name.to_string())
        .await?;
    println!(
        "{} Selected environment: {}",
        "✓".bright_cyan().bold(),
        name.bright_green().bold()
    );
    Ok(())
}

/// Rename environment interactively
pub async fn rename_environment_interactive() -> Result<()> {
    let client_manager = crate::client_manager();
    let environments = client_manager.list_environments().await;

    if environments.is_empty() {
        println!(
            "  {} No environments configured to rename.",
            "⚠️".bright_yellow().bold()
        );
        return Ok(());
    }

    println!();
    let env_selection = Select::new()
        .with_prompt("Select environment to rename")
        .items(&environments)
        .default(0)
        .interact()?;

    let old_name = environments[env_selection].clone();

    let new_name: String = Input::new()
        .with_prompt(format!("New name for '{}'", old_name))
        .interact()?;

    rename_environment_noninteractive(&old_name, new_name).await
}

/// Rename environment non-interactively
async fn rename_environment_noninteractive(old_name: &str, new_name: String) -> Result<()> {
    let client_manager = crate::client_manager();
    client_manager
        .rename_environment_in_config(old_name, new_name.clone())
        .await?;
    println!(
        "{} Environment renamed from '{}' to '{}'",
        "✓".bright_green().bold(),
        old_name.bright_green(),
        new_name.bright_green().bold()
    );
    Ok(())
}

/// Remove environment interactively
pub async fn remove_environment_interactive() -> Result<()> {
    let client_manager = crate::client_manager();
    let environments = client_manager.list_environments().await;
    let current_env = client_manager.get_current_environment_name().await?;

    if environments.is_empty() {
        println!(
            "  {} No environments configured to remove.",
            "⚠️".bright_yellow().bold()
        );
        return Ok(());
    }

    println!();
    let env_selection = Select::new()
        .with_prompt("Select environment to remove")
        .items(&environments)
        .default(0)
        .interact()?;

    let env_name = environments[env_selection].clone();

    // Warn if removing current environment
    if current_env.as_deref() == Some(&env_name) {
        println!(
            "  {} Warning: '{}' is the current environment",
            "⚠️".bright_yellow().bold(),
            env_name.bright_green().bold()
        );
    }

    let confirm = Confirm::new()
        .with_prompt(format!("Remove environment '{}'?", env_name))
        .default(false)
        .interact()?;

    if !confirm {
        println!("{} Cancelled.", "❌".bright_red().bold());
        return Ok(());
    }

    remove_environment_by_name(&env_name, true).await
}

/// Remove environment by name
async fn remove_environment_by_name(name: &str, force: bool) -> Result<()> {
    let client_manager = crate::client_manager();
    let current_env = client_manager.get_current_environment_name().await?;

    if !force {
        // Warn if removing current environment
        if current_env.as_deref() == Some(name) {
            println!(
                "  {} Warning: '{}' is the current environment",
                "⚠️".bright_yellow().bold(),
                name.bright_green().bold()
            );
        }

        let confirm = Confirm::new()
            .with_prompt(format!("Remove environment '{}'?", name))
            .default(false)
            .interact()?;

        if !confirm {
            println!("Cancelled.");
            return Ok(());
        }
    }

    client_manager.remove_environment_from_config(name).await?;
    println!(
        "{} Environment '{}' removed successfully",
        "✓".bright_green().bold(),
        name.bright_green().bold()
    );

    // Show current status after removal
    if let Some(current) = client_manager.get_current_environment_name().await? {
        if current != name {
            println!("Current environment: {}", current);
        } else {
            println!(
                "No current environment selected. Use 'dynamics-cli auth env select' to choose one."
            );
        }
    } else {
        println!(
            "No current environment selected. Use 'dynamics-cli auth env select' to choose one."
        );
    }

    Ok(())
}

/// Set credentials for environment interactively
pub async fn set_credentials_interactive() -> Result<()> {
    let client_manager = crate::client_manager();
    let environments = client_manager.list_environments().await;

    if environments.is_empty() {
        println!(
            "  {} No environments configured to update.",
            "⚠️".bright_yellow().bold()
        );
        return Ok(());
    }

    // Build display items with current credentials
    let mut display_items = Vec::new();
    for env_name in &environments {
        if let Some(environment) = client_manager.get_environment(env_name).await? {
            display_items.push(format!("{} [{}]", env_name, environment.credentials_ref));
        } else {
            display_items.push(env_name.to_string());
        }
    }

    println!();
    let env_selection = Select::new()
        .with_prompt("Select environment to update credentials")
        .items(&display_items)
        .default(0)
        .interact()?;

    let env_name = environments[env_selection].clone();

    // Select new credentials
    let credentials_list = client_manager.list_credentials().await?;
    if credentials_list.is_empty() {
        println!(
            "  {} No credentials configured. Please add credentials first.",
            "⚠️".bright_yellow().bold()
        );
        return Ok(());
    }

    let cred_selection = Select::new()
        .with_prompt("Select new credentials to use")
        .items(&credentials_list)
        .default(0)
        .interact()?;

    let new_credentials = credentials_list[cred_selection].clone();

    set_credentials_by_name(&env_name, &new_credentials).await
}

/// Set credentials for environment by name
async fn set_credentials_by_name(env_name: &str, credentials: &str) -> Result<()> {
    let client_manager = crate::client_manager();
    // Validate environment exists
    let mut environment = client_manager
        .get_environment(env_name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", env_name))?;

    // Validate credentials exist
    if client_manager.get_credentials(credentials).await?.is_none() {
        anyhow::bail!("Credentials '{}' not found", credentials);
    }

    // Update the environment
    environment.credentials_ref = credentials.to_string();
    client_manager
        .add_environment_to_config(env_name.to_string(), environment)
        .await?;

    println!(
        "{} Environment '{}' now uses credentials '{}'",
        "✓".bright_green().bold(),
        env_name.bright_green().bold(),
        credentials.bright_yellow().bold()
    );
    Ok(())
}
