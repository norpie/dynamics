//! Authentication status display

use crate::api::models::CredentialSet;
use crate::cli::ui::with_spinner;
use crate::config::Config;
use anyhow::Result;
use colored::*;

/// Display authentication status
pub async fn status_command() -> Result<()> {
    let config = Config::load().await?;

    println!();
    println!(
        "  {}",
        "📊 Dynamics CLI Authentication Status".bright_blue().bold()
    );
    println!(
        "  {}",
        "═════════════════════════════════════".bright_blue()
    );

    // Show environments
    let environments = config.list_environments().await?;
    if environments.is_empty() {
        println!();
        println!(
            "  {}",
            "⚠️  No environments configured".bright_yellow().bold()
        );
        println!(
            "  {}",
            "Run 'dynamics-cli auth' for interactive setup or use CLI commands:".dimmed()
        );
        println!("    {}", "dynamics-cli auth creds add --help".cyan());
        println!("    {}", "dynamics-cli auth env add --help".cyan());
        return Ok(());
    }

    let current_env = config.get_current_environment().await?;

    println!();
    println!("  {}", "Configured environments:".bright_white().bold());
    for env_name in &environments {
        if let Some(environment) = config.get_environment(env_name).await? {
            let (marker, env_color) = if current_env.as_ref() == Some(env_name) {
                ("●", env_name.bright_green().bold())
            } else {
                ("○", env_name.white())
            };
            println!("  {} {}", marker.bright_green(), env_color);
            println!("    {}: {}", "Host".dimmed(), environment.host.cyan());
            println!(
                "    {}: {}",
                "Credentials".dimmed(),
                environment.credentials_ref.bright_yellow()
            );
        }
    }

    // Show current environment details and test authentication
    if let Some(current_env_name) = current_env {
        println!();
        println!(
            "  {} {}",
            "Current Environment:".bright_white().bold(),
            current_env_name.bright_green().bold()
        );

        if let Some(environment) = config.get_environment(&current_env_name).await? {
            if let Some(credentials) = config.get_credentials(&environment.credentials_ref).await? {
                println!("    {}: {}", "Host".dimmed(), environment.host.cyan());
                println!(
                    "    {}: {}",
                    "Credentials".dimmed(),
                    environment.credentials_ref.bright_yellow()
                );

                // Show credential details (without secrets)
                match &credentials {
                    CredentialSet::UsernamePassword { username, .. } => {
                        println!(
                            "    {}: {}",
                            "Type".dimmed(),
                            "Username/Password".bright_blue()
                        );
                        println!("    {}: {}", "Username".dimmed(), username.white());
                    }
                    CredentialSet::ClientCredentials {
                        client_id,
                        tenant_id,
                        ..
                    } => {
                        println!(
                            "    {}: {}",
                            "Type".dimmed(),
                            "Client Credentials".bright_blue()
                        );
                        println!("    {}: {}", "Client ID".dimmed(), client_id.white());
                        println!("    {}: {}", "Tenant ID".dimmed(), tenant_id.white());
                    }
                    CredentialSet::DeviceCode {
                        client_id,
                        tenant_id,
                    } => {
                        println!("    {}: {}", "Type".dimmed(), "Device Code".bright_blue());
                        println!("    {}: {}", "Client ID".dimmed(), client_id.white());
                        println!("    {}: {}", "Tenant ID".dimmed(), tenant_id.white());
                    }
                    CredentialSet::Certificate {
                        client_id,
                        tenant_id,
                        cert_path,
                    } => {
                        println!("    {}: {}", "Type".dimmed(), "Certificate".bright_blue());
                        println!("    {}: {}", "Client ID".dimmed(), client_id.white());
                        println!("    {}: {}", "Tenant ID".dimmed(), tenant_id.white());
                        println!("    {}: {}", "Certificate".dimmed(), cert_path.cyan());
                    }
                }

                // Test authentication
                println!();
                let test_result = with_spinner("Testing authentication...", async {
                    // Use AuthManager directly for testing
                    let mut auth_manager = crate::api::auth::AuthManager::new();
                    auth_manager.add_credentials("test".to_string(), credentials.clone());

                    // Test the authentication with error wrapping
                    match auth_manager
                        .authenticate("test", &environment.host, &credentials)
                        .await
                    {
                        Ok(()) => Ok(()),
                        Err(e) => {
                            // Wrap any potential panics or network errors
                            log::debug!("Authentication error: {:?}", e);
                            Err(e)
                        }
                    }
                })
                .await;

                match test_result {
                    Ok(()) => {
                        println!("  {}", "✓ Authentication successful".bright_green().bold());
                    }
                    Err(e) => {
                        println!(
                            "  {} {}",
                            "✗ Authentication failed:".bright_red().bold(),
                            e.to_string().red()
                        );
                    }
                }
            } else {
                println!(
                    "  {} {}",
                    "✗ Credentials not found:".bright_red().bold(),
                    environment.credentials_ref.bright_yellow()
                );
            }
        }
    } else {
        println!();
        println!(
            "  {}",
            "⚠️  No current environment selected".bright_yellow().bold()
        );
        println!(
            "  {}",
            "Use 'dynamics-cli auth env select' to choose one.".dimmed()
        );
    }

    // Show credentials summary
    let credentials = config.list_credentials().await?;
    if !credentials.is_empty() {
        println!();
        println!("  {}", "Available credentials:".bright_white().bold());
        for cred_name in &credentials {
            if let Some(creds) = config.get_credentials(cred_name).await? {
                let (cred_type, type_color) = match creds {
                    CredentialSet::UsernamePassword { .. } => {
                        ("Username/Password", "Username/Password".bright_blue())
                    }
                    CredentialSet::ClientCredentials { .. } => {
                        ("Client Credentials", "Client Credentials".bright_blue())
                    }
                    CredentialSet::DeviceCode { .. } => {
                        ("Device Code", "Device Code".bright_blue())
                    }
                    CredentialSet::Certificate { .. } => {
                        ("Certificate", "Certificate".bright_blue())
                    }
                };
                println!(
                    "  {} {} ({})",
                    "•".bright_green(),
                    cred_name.bright_yellow().bold(),
                    type_color
                );
            }
        }
    }

    Ok(())
}
