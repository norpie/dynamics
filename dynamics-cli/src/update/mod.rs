//! Update module - wrapper around self_update for checking and installing updates
//!
//! Provides simple functions for:
//! - Checking current version
//! - Checking for available updates
//! - Installing updates
//!
//! Used by both CLI and TUI (future)

use anyhow::{Context, Result};
use semver::Version;
use serde::{Deserialize, Serialize};

/// GitHub repository information
const REPO_OWNER: &str = "norpie";
const REPO_NAME: &str = "dynamics";
const BIN_NAME: &str = "dynamics-cli";

/// Information about available updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    /// Current installed version
    pub current: String,
    /// Latest available version
    pub latest: String,
    /// Whether an update is needed
    pub needs_update: bool,
    /// URL to the release page
    pub release_url: String,
}

/// Progress information during update installation
#[derive(Debug, Clone)]
pub enum UpdateProgress {
    Checking,
    Downloading { bytes: u64, total: u64 },
    Verifying,
    Installing,
    Complete,
}

/// Get the current version of this binary
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Check for available updates from GitHub releases
pub async fn check_for_updates() -> Result<UpdateInfo> {
    let current = current_version();

    // Run blocking self_update calls in a blocking thread pool
    tokio::task::spawn_blocking(move || {
        // Build the updater
        let updater = self_update::backends::github::Update::configure()
            .repo_owner(REPO_OWNER)
            .repo_name(REPO_NAME)
            .bin_name(BIN_NAME)
            .current_version(current)
            .build()
            .context("Failed to configure update checker")?;

        // Get the latest release
        let latest_release = updater
            .get_latest_release()
            .context("Failed to fetch latest release from GitHub")?;

        let latest = latest_release.version.trim_start_matches('v');
        let current_ver = Version::parse(current).context("Invalid current version")?;
        let latest_ver = Version::parse(latest).context("Invalid latest version")?;

        Ok(UpdateInfo {
            current: current.to_string(),
            latest: latest.to_string(),
            needs_update: latest_ver > current_ver,
            release_url: format!(
                "https://github.com/{}/{}/releases/tag/v{}",
                REPO_OWNER, REPO_NAME, latest
            ),
        })
    })
    .await
    .context("Task panicked while checking for updates")?
}

/// Install the latest update
///
/// # Arguments
/// * `show_progress` - Whether to show download progress (for CLI)
///
/// # Returns
/// The version that was installed
pub async fn install_update(show_progress: bool) -> Result<String> {
    let current = current_version();

    log::info!("Installing update from {}/{}", REPO_OWNER, REPO_NAME);

    // Run blocking self_update calls in a blocking thread pool
    tokio::task::spawn_blocking(move || {
        // On Windows, zip archives have flat structure (no subdirectory)
        // Use .identifier(".zip") to prefer zip over msi
        #[cfg(target_os = "windows")]
        let status = self_update::backends::github::Update::configure()
            .repo_owner(REPO_OWNER)
            .repo_name(REPO_NAME)
            .bin_name(BIN_NAME)
            .show_download_progress(show_progress)
            .current_version(current)
            .bin_path_in_archive("{{ bin }}") // Windows: flat structure
            .identifier(".zip")
            .build()
            .context("Failed to configure updater")?
            .update()
            .map_err(|e| {
                log::error!("self_update failed: {}", e);
                anyhow::anyhow!("Failed to install update: {}", e)
            })?;

        // On Unix, tar.gz archives have subdirectory structure
        #[cfg(not(target_os = "windows"))]
        let status = self_update::backends::github::Update::configure()
            .repo_owner(REPO_OWNER)
            .repo_name(REPO_NAME)
            .bin_name(BIN_NAME)
            .show_download_progress(show_progress)
            .current_version(current)
            .bin_path_in_archive("{{ bin }}-{{ target }}/{{ bin }}") // Unix: has subdirectory
            .build()
            .context("Failed to configure updater")?
            .update()
            .context("Failed to install update")?;

        log::info!("Updated to version: {}", status.version());
        Ok(status.version().to_string())
    })
    .await
    .context("Task panicked while installing update")?
}

/// Install a specific version
///
/// # Arguments
/// * `version` - The version to install (e.g., "0.2.0")
/// * `show_progress` - Whether to show download progress
pub async fn install_version(version: &str, show_progress: bool) -> Result<String> {
    let current = current_version();
    let version = version.to_string();

    log::info!(
        "Installing version {} from {}/{}",
        version,
        REPO_OWNER,
        REPO_NAME
    );

    // Run blocking self_update calls in a blocking thread pool
    tokio::task::spawn_blocking(move || {
        // On Windows, zip archives have flat structure (no subdirectory)
        // Use .identifier(".zip") to prefer zip over msi
        #[cfg(target_os = "windows")]
        let status = self_update::backends::github::Update::configure()
            .repo_owner(REPO_OWNER)
            .repo_name(REPO_NAME)
            .bin_name(BIN_NAME)
            .show_download_progress(show_progress)
            .current_version(current)
            .target_version_tag(&format!("v{}", version))
            .bin_path_in_archive("{{ bin }}") // Windows: flat structure
            .identifier(".zip")
            .build()
            .context("Failed to configure updater")?
            .update()
            .map_err(|e| {
                log::error!("self_update failed: {}", e);
                anyhow::anyhow!("Failed to install version: {}", e)
            })?;

        // On Unix, tar.gz archives have subdirectory structure
        #[cfg(not(target_os = "windows"))]
        let status = self_update::backends::github::Update::configure()
            .repo_owner(REPO_OWNER)
            .repo_name(REPO_NAME)
            .bin_name(BIN_NAME)
            .show_download_progress(show_progress)
            .current_version(current)
            .target_version_tag(&format!("v{}", version))
            .bin_path_in_archive("{{ bin }}-{{ target }}/{{ bin }}") // Unix: has subdirectory
            .build()
            .context("Failed to configure updater")?
            .update()
            .context("Failed to install version")?;

        log::info!("Installed version: {}", status.version());
        Ok(status.version().to_string())
    })
    .await
    .context("Task panicked while installing version")?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_version() {
        let version = current_version();
        assert!(!version.is_empty());
        // Should be a valid semver
        Version::parse(version).expect("Current version should be valid semver");
    }
}
