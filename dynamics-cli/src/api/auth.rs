use super::models::{CredentialSet, TokenInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages authentication tokens and credentials for multiple environments
pub struct AuthManager {
    credentials: Arc<RwLock<HashMap<String, CredentialSet>>>,
    tokens: Arc<RwLock<HashMap<String, TokenInfo>>>,
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            credentials: Arc::new(RwLock::new(HashMap::new())),
            tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_credentials(&self, name: String, credentials: CredentialSet) {
        self.credentials.write().await.insert(name, credentials);
    }

    pub async fn try_select_credentials(&self, name: &str) -> anyhow::Result<CredentialSet> {
        self.credentials
            .read()
            .await
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Credentials '{}' not found", name))
    }

    // List methods
    pub async fn list_credentials(&self) -> Vec<String> {
        self.credentials.read().await.keys().cloned().collect()
    }

    // Delete methods
    pub async fn delete_credentials(&self, name: &str) -> anyhow::Result<()> {
        self.credentials
            .write()
            .await
            .remove(name)
            .ok_or_else(|| anyhow::anyhow!("Credentials '{}' not found", name))?;
        Ok(())
    }

    // Rename methods
    pub async fn rename_credentials(&self, old_name: &str, new_name: String) -> anyhow::Result<()> {
        let mut credentials = self.credentials.write().await;
        let creds = credentials
            .remove(old_name)
            .ok_or_else(|| anyhow::anyhow!("Credentials '{}' not found", old_name))?;
        credentials.insert(new_name, creds);
        Ok(())
    }

    // Authentication methods
    pub async fn authenticate(
        &self,
        env_name: &str,
        host: &str,
        credentials: &CredentialSet,
    ) -> anyhow::Result<()> {
        use std::time::{Duration, SystemTime};

        log::info!("Authenticating to {} for environment {}", host, env_name);

        match credentials {
            CredentialSet::UsernamePassword {
                username,
                password,
                client_id,
                client_secret,
            } => {
                let token_url = "https://login.microsoftonline.com/common/oauth2/token";

                let client = reqwest::Client::new();
                let response = client
                    .post(token_url)
                    .form(&[
                        ("grant_type", "password"),
                        ("client_id", client_id),
                        ("client_secret", client_secret),
                        ("username", username),
                        ("password", password),
                        ("resource", host),
                    ])
                    .send()
                    .await?;

                log::debug!("Token request status: {}", response.status());

                if response.status().is_success() {
                    let token_data: serde_json::Value = response.json().await?;

                    if let Some(access_token) =
                        token_data.get("access_token").and_then(|t| t.as_str())
                    {
                        // Calculate expiration (default to 1 hour if not provided)
                        let expires_in = token_data
                            .get("expires_in")
                            .and_then(|e| e.as_u64())
                            .unwrap_or(3600);

                        let expires_at = SystemTime::now() + Duration::from_secs(expires_in);

                        let refresh_token = token_data
                            .get("refresh_token")
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string());

                        let token_info = TokenInfo {
                            access_token: access_token.to_string(),
                            expires_at,
                            refresh_token,
                        };

                        self.tokens
                            .write()
                            .await
                            .insert(env_name.to_string(), token_info);

                        log::info!("Successfully authenticated for environment {}", env_name);
                        Ok(())
                    } else {
                        anyhow::bail!("No access token in response")
                    }
                } else {
                    let error_text = response.text().await?;
                    anyhow::bail!("Authentication failed: {}", error_text)
                }
            }
            _ => {
                anyhow::bail!(
                    "Authentication method not yet implemented: {:?}",
                    credentials
                )
            }
        }
    }

    /// Get token for environment (used by ClientManager)
    pub async fn get_token(&self, env_name: &str) -> anyhow::Result<TokenInfo> {
        self.tokens
            .read()
            .await
            .get(env_name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No token found for environment '{}'", env_name))
    }

    /// Check if token exists and is valid for environment
    pub async fn has_valid_token(&self, env_name: &str) -> bool {
        if let Some(token_info) = self.tokens.read().await.get(env_name) {
            // Check if token is still valid
            if let Ok(elapsed) = token_info.expires_at.elapsed() {
                elapsed.as_secs() == 0 // Token is valid if it hasn't elapsed
            } else {
                true // If we can't determine elapsed time, assume valid
            }
        } else {
            false
        }
    }
}
