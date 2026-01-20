//! Authentication management commands with TTY detection

use anyhow::Result;
use clap::{Args, Subcommand};
use is_terminal::IsTerminal;

mod credentials;
mod environments;
mod interactive;
mod status;

#[derive(Args)]
pub struct AuthCommands {
    #[command(subcommand)]
    pub command: Option<AuthSubcommands>,
}

#[derive(Subcommand)]
pub enum AuthSubcommands {
    /// Display authentication status
    Status,
    /// Credential management
    #[command(subcommand)]
    Creds(CredentialCommands),
    /// Environment management
    #[command(subcommand)]
    Env(EnvironmentCommands),
}

#[derive(Subcommand)]
pub enum CredentialCommands {
    /// Add new credentials
    Add {
        /// Name for the credential set
        #[arg(long)]
        name: String,
        /// Credential type
        #[arg(long, value_enum)]
        r#type: CredentialType,
        /// Username (for username-password)
        #[arg(long)]
        username: Option<String>,
        /// Password (for username-password)
        #[arg(long)]
        password: Option<String>,
        /// Client ID
        #[arg(long)]
        client_id: Option<String>,
        /// Client secret
        #[arg(long)]
        client_secret: Option<String>,
        /// Tenant ID (for client-credentials/device-code/certificate)
        #[arg(long)]
        tenant_id: Option<String>,
        /// Certificate path (for certificate auth)
        #[arg(long)]
        cert_path: Option<String>,
    },
    /// List all credentials
    List,
    /// Test credentials
    Test {
        /// Credential name to test
        name: String,
        /// Host URL to test against
        #[arg(long)]
        host: String,
    },
    /// Rename credentials
    Rename {
        /// Current name
        old_name: String,
        /// New name
        new_name: String,
    },
    /// Remove credentials
    Remove {
        /// Credential name to remove
        name: String,
        /// Force removal without confirmation
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Subcommand)]
pub enum EnvironmentCommands {
    /// Add new environment
    Add {
        /// Environment name
        #[arg(long)]
        name: String,
        /// Host URL
        #[arg(long)]
        host: String,
        /// Credentials to use
        #[arg(long)]
        credentials: String,
        /// Set as current environment
        #[arg(long)]
        set_current: bool,
    },
    /// List all environments
    List,
    /// Set credentials for an environment
    SetCredentials {
        /// Environment name
        name: String,
        /// Credentials to use
        credentials: String,
    },
    /// Select current environment
    Select {
        /// Environment name to select
        name: Option<String>,
    },
    /// Rename environment
    Rename {
        /// Current name
        old_name: String,
        /// New name
        new_name: String,
    },
    /// Remove environment
    Remove {
        /// Environment name to remove
        name: String,
        /// Force removal without confirmation
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum CredentialType {
    #[value(name = "username-password")]
    UsernamePassword,
    #[value(name = "client-credentials")]
    ClientCredentials,
    #[value(name = "device-code")]
    DeviceCode,
    #[value(name = "certificate")]
    Certificate,
}

/// Main auth command handler with TTY detection
pub async fn auth_command(args: AuthCommands) -> Result<()> {
    let client_manager = crate::client_manager();

    // If no subcommand and we're in an interactive terminal, show the menu
    if args.command.is_none() && std::io::stdin().is_terminal() {
        interactive::run_main_menu().await
    } else {
        // Non-interactive mode or subcommand provided
        match args.command {
            Some(AuthSubcommands::Status) => status::status_command().await,
            Some(AuthSubcommands::Creds(cmd)) => credentials::handle_credential_command(cmd).await,
            Some(AuthSubcommands::Env(cmd)) => environments::handle_environment_command(cmd).await,
            None => {
                // Non-interactive mode without subcommand - show help
                println!("Authentication management for Dynamics CLI");
                println!();
                println!("Run in an interactive terminal for menu-driven interface,");
                println!("or use one of the subcommands:");
                println!();
                println!("  dynamics-cli auth status              # Show current status");
                println!("  dynamics-cli auth creds --help        # Credential management");
                println!("  dynamics-cli auth env --help          # Environment management");
                Ok(())
            }
        }
    }
}
