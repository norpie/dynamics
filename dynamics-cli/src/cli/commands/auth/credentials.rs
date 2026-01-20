//! Credential management operations

use super::{CredentialCommands, CredentialType};
use crate::api::models::CredentialSet;
use crate::cli::ui::with_spinner;
use crate::config::Config;
use anyhow::Result;
use colored::*;
use dialoguer::{Confirm, Input, Password, Select};

/// Handle non-interactive credential commands
pub async fn handle_credential_command(cmd: CredentialCommands) -> Result<()> {
    let client_manager = crate::client_manager();

    match cmd {
        CredentialCommands::Add {
            name,
            r#type,
            username,
            password,
            client_id,
            client_secret,
            tenant_id,
            cert_path,
        } => {
            add_credentials_noninteractive(
                name,
                r#type,
                username,
                password,
                client_id,
                client_secret,
                tenant_id,
                cert_path,
            )
            .await
        }
        CredentialCommands::List => list_credentials_interactive().await,
        CredentialCommands::Test { name, host } => test_credentials_by_name(&name, &host).await,
        CredentialCommands::Rename { old_name, new_name } => {
            rename_credentials_noninteractive(&old_name, new_name).await
        }
        CredentialCommands::Remove { name, force } => {
            remove_credentials_by_name(&name, force).await
        }
    }
}

/// Add credentials non-interactively (CLI args)
async fn add_credentials_noninteractive(
    name: String,
    cred_type: CredentialType,
    username: Option<String>,
    password: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    tenant_id: Option<String>,
    cert_path: Option<String>,
) -> Result<()> {
    let client_manager = crate::client_manager();
    let credentials = match cred_type {
        CredentialType::UsernamePassword => {
            let username = username
                .ok_or_else(|| anyhow::anyhow!("--username required for username-password"))?;
            let password = password
                .ok_or_else(|| anyhow::anyhow!("--password required for username-password"))?;
            let client_id = client_id
                .ok_or_else(|| anyhow::anyhow!("--client-id required for username-password"))?;
            let client_secret = client_secret
                .ok_or_else(|| anyhow::anyhow!("--client-secret required for username-password"))?;

            CredentialSet::UsernamePassword {
                username,
                password,
                client_id,
                client_secret,
            }
        }
        CredentialType::ClientCredentials => {
            let client_id = client_id
                .ok_or_else(|| anyhow::anyhow!("--client-id required for client-credentials"))?;
            let client_secret = client_secret.ok_or_else(|| {
                anyhow::anyhow!("--client-secret required for client-credentials")
            })?;
            let tenant_id = tenant_id
                .ok_or_else(|| anyhow::anyhow!("--tenant-id required for client-credentials"))?;

            CredentialSet::ClientCredentials {
                client_id,
                client_secret,
                tenant_id,
            }
        }
        CredentialType::DeviceCode => {
            let client_id =
                client_id.ok_or_else(|| anyhow::anyhow!("--client-id required for device-code"))?;
            let tenant_id =
                tenant_id.ok_or_else(|| anyhow::anyhow!("--tenant-id required for device-code"))?;

            CredentialSet::DeviceCode {
                client_id,
                tenant_id,
            }
        }
        CredentialType::Certificate => {
            let client_id =
                client_id.ok_or_else(|| anyhow::anyhow!("--client-id required for certificate"))?;
            let tenant_id =
                tenant_id.ok_or_else(|| anyhow::anyhow!("--tenant-id required for certificate"))?;
            let cert_path =
                cert_path.ok_or_else(|| anyhow::anyhow!("--cert-path required for certificate"))?;

            CredentialSet::Certificate {
                client_id,
                tenant_id,
                cert_path,
            }
        }
    };

    client_manager
        .add_credentials(name.clone(), credentials)
        .await?;
    println!(
        "{} Credentials '{}' added successfully",
        "✓".bright_green().bold(),
        name.bright_yellow().bold()
    );
    Ok(())
}

/// List credentials (works for both interactive and non-interactive)
pub async fn list_credentials_interactive() -> Result<()> {
    let client_manager = crate::client_manager();
    let credentials = client_manager.list_credentials().await?;

    if credentials.is_empty() {
        println!(
            "  {}",
            "⚠️  No credentials configured".bright_yellow().bold()
        );
        println!("  {}", "Add some credentials to get started.".dimmed());
        return Ok(());
    }

    println!();
    println!("  {}", "Configured credentials:".bright_white().bold());
    for cred_name in &credentials {
        if let Some(creds) = client_manager.get_credentials(cred_name).await? {
            let cred_type = match creds {
                CredentialSet::UsernamePassword { .. } => "Username/Password".bright_blue(),
                CredentialSet::ClientCredentials { .. } => "Client Credentials".bright_blue(),
                CredentialSet::DeviceCode { .. } => "Device Code".bright_blue(),
                CredentialSet::Certificate { .. } => "Certificate".bright_blue(),
            };
            println!(
                "  {} {} ({})",
                "•".bright_green(),
                cred_name.bright_yellow().bold(),
                cred_type
            );
        }
    }
    println!();

    Ok(())
}

/// Add credentials interactively
pub async fn add_credentials_interactive() -> Result<()> {
    let client_manager = crate::client_manager();
    println!();
    println!("Add New Credentials");
    println!("==================");

    // Get credential name
    let name: String = Input::new()
        .with_prompt("Credential name (e.g., 'prod-creds', 'dev-auth')")
        .interact()?;

    // Check if exists
    if client_manager.get_credentials(&name).await?.is_some() {
        let overwrite = Confirm::new()
            .with_prompt(format!("Credentials '{}' already exist. Overwrite?", name))
            .default(false)
            .interact()?;

        if !overwrite {
            println!("{} Cancelled.", "❌".bright_red().bold());
            return Ok(());
        }
    }

    // Select credential type
    let types = vec![
        ("Username/Password", CredentialType::UsernamePassword),
        ("Client Credentials", CredentialType::ClientCredentials),
        ("Device Code", CredentialType::DeviceCode),
        ("Certificate", CredentialType::Certificate),
    ];

    let type_selection = Select::new()
        .with_prompt("Credential type")
        .items(&types.iter().map(|(name, _)| name).collect::<Vec<_>>())
        .default(0)
        .interact()?;

    let (_, cred_type) = &types[type_selection];

    // Collect credentials based on type
    let credentials = match cred_type {
        CredentialType::UsernamePassword => {
            let username: String = Input::new().with_prompt("Username").interact()?;
            let password: String = Password::new().with_prompt("Password").interact()?;
            let client_id: String = Input::new().with_prompt("Client ID").interact()?;
            let client_secret: String = Password::new().with_prompt("Client Secret").interact()?;

            CredentialSet::UsernamePassword {
                username,
                password,
                client_id,
                client_secret,
            }
        }
        CredentialType::ClientCredentials => {
            let client_id: String = Input::new().with_prompt("Client ID").interact()?;
            let client_secret: String = Password::new().with_prompt("Client Secret").interact()?;
            let tenant_id: String = Input::new().with_prompt("Tenant ID").interact()?;

            CredentialSet::ClientCredentials {
                client_id,
                client_secret,
                tenant_id,
            }
        }
        CredentialType::DeviceCode => {
            let client_id: String = Input::new().with_prompt("Client ID").interact()?;
            let tenant_id: String = Input::new().with_prompt("Tenant ID").interact()?;

            CredentialSet::DeviceCode {
                client_id,
                tenant_id,
            }
        }
        CredentialType::Certificate => {
            let client_id: String = Input::new().with_prompt("Client ID").interact()?;
            let tenant_id: String = Input::new().with_prompt("Tenant ID").interact()?;
            let cert_path: String = Input::new().with_prompt("Certificate path").interact()?;

            CredentialSet::Certificate {
                client_id,
                tenant_id,
                cert_path,
            }
        }
    };

    // Save credentials
    client_manager
        .add_credentials(name.clone(), credentials)
        .await?;
    println!(
        "{} Credentials '{}' saved successfully",
        "✓".bright_green().bold(),
        name.bright_yellow().bold()
    );

    Ok(())
}

/// Test credentials interactively
pub async fn test_credentials_interactive() -> Result<()> {
    let client_manager = crate::client_manager();
    let credentials = client_manager.list_credentials().await?;

    if credentials.is_empty() {
        println!(
            "  {}",
            "⚠️  No credentials configured to test"
                .bright_yellow()
                .bold()
        );
        return Ok(());
    }

    println!();
    let cred_selection = Select::new()
        .with_prompt("Select credentials to test")
        .items(&credentials)
        .default(0)
        .interact()?;

    let cred_name = &credentials[cred_selection];

    // Check if there are any environments
    let environments = client_manager.list_environments().await;
    let host = if environments.is_empty() {
        // No environments, ask for URL manually
        Input::new()
            .with_prompt("Host URL to test against")
            .interact()?
    } else {
        // Offer choice between existing environment or manual URL
        let choices = vec!["Use existing environment", "Enter URL manually"];
        let choice_selection = Select::new()
            .with_prompt("Test against")
            .items(&choices)
            .default(0)
            .interact()?;

        if choice_selection == 0 {
            // Use existing environment
            let env_selection = Select::new()
                .with_prompt("Select environment")
                .items(&environments)
                .default(0)
                .interact()?;

            let env_name = &environments[env_selection];
            let environment = client_manager.try_select_env(env_name).await?;

            environment.host.clone()
        } else {
            // Manual URL entry
            Input::new()
                .with_prompt("Host URL to test against")
                .interact()?
        }
    };

    test_credentials_by_name(cred_name, &host).await
}

/// Test specific credentials by name
async fn test_credentials_by_name(name: &str, host: &str) -> Result<()> {
    let client_manager = crate::client_manager();
    let credentials = client_manager
        .get_credentials(name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Credentials '{}' not found", name))?;

    let result = with_spinner("Testing authentication...", async {
        // Use AuthManager directly for testing
        let mut auth_manager = crate::api::auth::AuthManager::new();
        auth_manager.add_credentials("test".to_string(), credentials.clone());

        // Test the authentication with error wrapping
        match auth_manager.authenticate("test", host, &credentials).await {
            Ok(()) => Ok(()),
            Err(e) => {
                // Wrap any potential panics or network errors
                log::debug!("Authentication error: {:?}", e);
                Err(e)
            }
        }
    })
    .await;

    match result {
        Ok(()) => {
            println!(
                "{} Authentication test successful for '{}'",
                "✓".bright_green().bold(),
                name.bright_yellow().bold()
            );
        }
        Err(e) => {
            println!(
                "{} Authentication test failed for '{}': {}",
                "✗".bright_red().bold(),
                name.bright_yellow().bold(),
                e.to_string().red()
            );
        }
    }

    Ok(())
}

/// Rename credentials interactively
pub async fn rename_credentials_interactive() -> Result<()> {
    let client_manager = crate::client_manager();
    let credentials = client_manager.list_credentials().await?;

    if credentials.is_empty() {
        println!("No credentials configured to rename.");
        return Ok(());
    }

    println!();
    let cred_selection = Select::new()
        .with_prompt("Select credentials to rename")
        .items(&credentials)
        .default(0)
        .interact()?;

    let old_name = &credentials[cred_selection];

    let new_name: String = Input::new()
        .with_prompt(format!("New name for '{}'", old_name))
        .interact()?;

    rename_credentials_noninteractive(old_name, new_name).await
}

/// Rename credentials non-interactively
async fn rename_credentials_noninteractive(old_name: &str, new_name: String) -> Result<()> {
    let client_manager = crate::client_manager();
    client_manager
        .rename_credentials(old_name, new_name.clone())
        .await?;
    println!(
        "{} Credentials renamed from '{}' to '{}'",
        "✓".bright_green().bold(),
        old_name.bright_yellow(),
        new_name.bright_yellow().bold()
    );
    Ok(())
}

/// Remove credentials interactively
pub async fn remove_credentials_interactive() -> Result<()> {
    let client_manager = crate::client_manager();
    let credentials = client_manager.list_credentials().await?;

    if credentials.is_empty() {
        println!("No credentials configured to remove.");
        return Ok(());
    }

    println!();
    let cred_selection = Select::new()
        .with_prompt("Select credentials to remove")
        .items(&credentials)
        .default(0)
        .interact()?;

    let cred_name = &credentials[cred_selection];

    let confirm = Confirm::new()
        .with_prompt(format!("Remove credentials '{}'?", cred_name))
        .default(false)
        .interact()?;

    if !confirm {
        println!("{} Cancelled.", "❌".bright_red().bold());
        return Ok(());
    }

    remove_credentials_by_name(cred_name, true).await
}

/// Remove credentials by name
async fn remove_credentials_by_name(name: &str, force: bool) -> Result<()> {
    let client_manager = crate::client_manager();
    // Check if credentials are in use by any environments
    let environments = client_manager.list_environments().await;
    let mut using_environments = Vec::new();

    for env_name in environments {
        if let Some(environment) = client_manager.try_select_env(&env_name).await.ok() {
            if environment.credentials_ref == name {
                using_environments.push(env_name);
            }
        }
    }

    if !using_environments.is_empty() {
        println!(
            "{} Cannot remove credentials '{}' - currently in use by environments:",
            "⚠️".bright_yellow().bold(),
            name.bright_yellow().bold()
        );
        for env in &using_environments {
            println!("  {} {}", "•".bright_red(), env.white());
        }
        println!(
            "  {}",
            "Remove or change credentials for these environments first.".dimmed()
        );
        return Ok(());
    }

    if !force {
        let confirm = Confirm::new()
            .with_prompt(format!("Remove credentials '{}'?", name))
            .default(false)
            .interact()?;

        if !confirm {
            println!("{} Cancelled.", "❌".bright_red().bold());
            return Ok(());
        }
    }

    client_manager.remove_credentials(name).await?;
    println!(
        "{} Credentials '{}' removed successfully",
        "✓".bright_green().bold(),
        name.bright_yellow().bold()
    );
    Ok(())
}
