use clap::{Args, Subcommand};

#[derive(Args)]
pub struct UpdateCommands {
    #[command(subcommand)]
    pub command: UpdateSubcommands,
}

#[derive(Subcommand)]
pub enum UpdateSubcommands {
    /// Check for available updates
    Status,
    /// Install the latest version
    Install {
        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Install a specific version
    Version {
        /// Version to install (e.g., "0.2.0")
        version: String,
        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
}

/// Handle update command
pub async fn handle_update_command(cmd: UpdateCommands) -> anyhow::Result<()> {
    use colored::Colorize;
    use dialoguer::Confirm;

    match cmd.command {
        UpdateSubcommands::Status => {
            println!("{}", "Checking for updates...".dimmed());

            let current = crate::update::current_version();
            println!("Current version: {}", current.green());

            match crate::update::check_for_updates().await {
                Ok(info) => {
                    println!("Latest version:  {}", info.latest.green());

                    if info.needs_update {
                        println!("\n{}", "🎉 A new version is available!".green().bold());
                        println!("Release notes: {}", info.release_url.blue());
                        println!(
                            "\nRun {} to install the latest version",
                            "dynamics-cli update install".cyan()
                        );
                    } else {
                        println!("\n{}", "✓ You are running the latest version".green());
                    }

                    // Update last check timestamp
                    let config = crate::config::Config::load().await?;
                    let now = chrono::Utc::now();
                    crate::config::repository::update_metadata::set_last_check_time(
                        &config.pool,
                        now,
                    )
                    .await?;
                }
                Err(e) => {
                    eprintln!("{} {}", "Error:".red(), e);
                    eprintln!(
                        "{}",
                        "Failed to check for updates. Please try again later.".yellow()
                    );
                }
            }
        }

        UpdateSubcommands::Install { yes } => {
            println!("{}", "Checking for updates...".dimmed());

            let info = crate::update::check_for_updates().await?;

            if !info.needs_update {
                println!("{}", "✓ You are already running the latest version".green());
                return Ok(());
            }

            println!("Current version: {}", info.current.yellow());
            println!("Latest version:  {}", info.latest.green());
            println!("Release notes:   {}", info.release_url.blue());

            let should_install = if yes {
                true
            } else {
                Confirm::new()
                    .with_prompt(format!("Install version {}?", info.latest))
                    .default(true)
                    .interact()?
            };

            if !should_install {
                println!("{}", "Update cancelled".yellow());
                return Ok(());
            }

            println!("\n{}", "Installing update...".dimmed());
            match crate::update::install_update(true).await {
                Ok(version) => {
                    println!(
                        "{}",
                        format!("✓ Successfully updated to version {}", version).green()
                    );
                    println!(
                        "{}",
                        "Please restart the application to use the new version".cyan()
                    );

                    // Update last check timestamp
                    let config = crate::config::Config::load().await?;
                    let now = chrono::Utc::now();
                    crate::config::repository::update_metadata::set_last_check_time(
                        &config.pool,
                        now,
                    )
                    .await?;
                }
                Err(e) => {
                    eprintln!("{} {}", "Error:".red(), e);
                    eprintln!(
                        "{}",
                        "Failed to install update. Please try again or download manually.".yellow()
                    );
                }
            }
        }

        UpdateSubcommands::Version { version, yes } => {
            println!("{}", format!("Installing version {}...", version).dimmed());

            let current = crate::update::current_version();
            println!("Current version: {}", current.yellow());
            println!("Target version:  {}", version.green());

            let should_install = if yes {
                true
            } else {
                Confirm::new()
                    .with_prompt(format!("Install version {}?", version))
                    .default(true)
                    .interact()?
            };

            if !should_install {
                println!("{}", "Update cancelled".yellow());
                return Ok(());
            }

            println!("\n{}", "Installing...".dimmed());
            match crate::update::install_version(&version, true).await {
                Ok(installed_version) => {
                    println!(
                        "{}",
                        format!("✓ Successfully installed version {}", installed_version).green()
                    );
                    println!(
                        "{}",
                        "Please restart the application to use the new version".cyan()
                    );

                    // Update last check timestamp
                    let config = crate::config::Config::load().await?;
                    let now = chrono::Utc::now();
                    crate::config::repository::update_metadata::set_last_check_time(
                        &config.pool,
                        now,
                    )
                    .await?;
                }
                Err(e) => {
                    eprintln!("{} {}", "Error:".red(), e);
                    eprintln!(
                        "{}",
                        "Failed to install version. Please check the version number and try again."
                            .yellow()
                    );
                }
            }
        }
    }

    Ok(())
}
