use super::auth::AuthManager;
use super::client::DynamicsClient;
use super::models::{CredentialSet, Environment, TokenInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages multiple Dynamics client instances for different environments
pub struct ClientManager {
    clients: Arc<RwLock<HashMap<String, DynamicsClient>>>,
    auth_manager: AuthManager,
    environments: Arc<RwLock<HashMap<String, Environment>>>,
    current_env: Arc<RwLock<Option<String>>>,
    tokens: Arc<RwLock<HashMap<String, TokenInfo>>>,
}

impl ClientManager {
    pub async fn from_env() -> anyhow::Result<Self> {
        // Load .env file if it exists
        dotenvy::dotenv().ok();

        let host = std::env::var("DYNAMICS_HOST")?;
        let username = std::env::var("DYNAMICS_USERNAME")?;
        let password = std::env::var("DYNAMICS_PASSWORD")?;
        let client_id = std::env::var("DYNAMICS_CLIENT_ID")?;
        let client_secret = std::env::var("DYNAMICS_CLIENT_SECRET")?;

        let mut auth_manager = AuthManager::new();

        // Add test credentials to auth manager
        let credentials = CredentialSet::UsernamePassword {
            username,
            password,
            client_id,
            client_secret,
        };

        auth_manager.add_credentials(".env".to_string(), credentials);

        // Add test environment
        let environment = Environment {
            name: ".env".to_string(),
            host,
            credentials_ref: ".env".to_string(),
        };

        let mut environments = HashMap::new();
        environments.insert(".env".to_string(), environment);

        Ok(Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            auth_manager,
            environments: Arc::new(RwLock::new(environments)),
            current_env: Arc::new(RwLock::new(Some(".env".to_string()))),
            tokens: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create a new ClientManager from the global config
    pub async fn new() -> anyhow::Result<Self> {
        let config = crate::global_config();
        let mut auth_manager = AuthManager::new();
        let mut environments = HashMap::new();

        // Load environments from config
        let env_names = config.list_environments().await?;
        for env_name in env_names {
            if let Some(environment) = config.get_environment(&env_name).await? {
                // Load credentials for this environment
                if let Some(credentials) =
                    config.get_credentials(&environment.credentials_ref).await?
                {
                    auth_manager
                        .add_credentials(environment.credentials_ref.clone(), credentials)
                        .await;
                }
                environments.insert(env_name, environment);
            }
        }

        // Get current environment
        let current_env = config.get_current_environment().await?;

        // Load valid tokens from database
        let mut tokens = HashMap::new();
        for env_name in environments.keys() {
            if let Some(token) = config.get_token(env_name).await? {
                // Only load non-expired tokens
                if let Ok(elapsed) = token.expires_at.elapsed() {
                    if elapsed.as_secs() == 0 {
                        log::debug!("Loaded valid token for environment: {}", env_name);
                        tokens.insert(env_name.clone(), token);
                    } else {
                        log::debug!("Skipping expired token for environment: {}", env_name);
                        // Clean up expired token from database
                        let _ = config.delete_token(env_name).await;
                    }
                } else {
                    // If we can't determine expiration, assume it's valid
                    log::debug!(
                        "Loaded token for environment (unknown expiration): {}",
                        env_name
                    );
                    tokens.insert(env_name.clone(), token);
                }
            }
        }

        Ok(Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            auth_manager,
            environments: Arc::new(RwLock::new(environments)),
            current_env: Arc::new(RwLock::new(current_env)),
            tokens: Arc::new(RwLock::new(tokens)),
        })
    }

    pub async fn authenticate(&self) -> anyhow::Result<()> {
        let current_env = self
            .current_env
            .read()
            .await
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No environment selected"))?
            .clone();

        let environment = self.try_select_env(&current_env).await?;
        let host = environment.host.clone();
        let credentials_ref = environment.credentials_ref.clone();
        let credentials = self
            .auth_manager
            .try_select_credentials(&credentials_ref)
            .await?;

        // Get the token from auth_manager and store it in our tokens cache
        self.auth_manager
            .authenticate(&current_env, &host, &credentials)
            .await?;

        // Copy token from auth_manager to our local cache and save to database
        if let Ok(token) = self.auth_manager.get_token(&current_env).await {
            // Save to memory cache
            self.tokens
                .write()
                .await
                .insert(current_env.clone(), token.clone());

            // Save to database for persistence
            crate::global_config()
                .save_token(current_env, token)
                .await?;
        }

        Ok(())
    }

    // Environment management
    pub async fn add_environment(
        &self,
        name: String,
        environment: Environment,
    ) -> anyhow::Result<()> {
        // Save to config database
        crate::global_config()
            .add_environment(environment.clone())
            .await?;

        // Update local cache
        self.environments.write().await.insert(name, environment);
        Ok(())
    }

    pub async fn add_credentials(
        &self,
        name: String,
        credentials: CredentialSet,
    ) -> anyhow::Result<()> {
        // Save to config database
        crate::global_config()
            .add_credentials(name.clone(), credentials.clone())
            .await?;

        // Update auth manager
        self.auth_manager.add_credentials(name, credentials).await;
        Ok(())
    }

    /// Test authentication with specific credentials and environment
    pub async fn test_auth_with_credentials(
        &self,
        environment: &Environment,
        credentials: &CredentialSet,
    ) -> anyhow::Result<()> {
        // Create a temporary auth manager for testing
        let mut temp_auth_manager = AuthManager::new();
        temp_auth_manager.add_credentials("test".to_string(), credentials.clone());

        // Test authentication
        temp_auth_manager
            .authenticate("test", &environment.host, credentials)
            .await
    }

    /// List all available credentials
    pub async fn list_credentials(&self) -> anyhow::Result<Vec<String>> {
        crate::global_config().list_credentials().await
    }

    /// Remove credentials
    pub async fn remove_credentials(&self, name: &str) -> anyhow::Result<()> {
        // Remove from config database
        crate::global_config().delete_credentials(name).await?;
        // Remove from auth manager
        self.auth_manager.delete_credentials(name).await?;
        Ok(())
    }

    /// Rename credentials
    pub async fn rename_credentials(&self, old_name: &str, new_name: String) -> anyhow::Result<()> {
        // Rename in config database
        crate::global_config()
            .rename_credentials(old_name, new_name.clone())
            .await?;
        // Rename in auth manager
        self.auth_manager
            .rename_credentials(old_name, new_name)
            .await?;
        Ok(())
    }

    /// Get credentials by name
    pub async fn get_credentials(&self, name: &str) -> anyhow::Result<Option<CredentialSet>> {
        crate::global_config().get_credentials(name).await
    }

    /// Add environment
    pub async fn add_environment_to_config(
        &self,
        name: String,
        environment: Environment,
    ) -> anyhow::Result<()> {
        // Save to config database
        let mut api_env = environment.clone();
        api_env.name = name.clone(); // Ensure name is set
        crate::global_config().add_environment(api_env).await?;
        // Update local cache
        self.environments.write().await.insert(name, environment);
        Ok(())
    }

    /// Remove environment from config
    pub async fn remove_environment_from_config(&self, name: &str) -> anyhow::Result<()> {
        // Remove from config database
        crate::global_config().delete_environment(name).await?;
        // Remove from local cache
        self.environments.write().await.remove(name);
        Ok(())
    }

    /// Rename environment in config
    pub async fn rename_environment_in_config(
        &self,
        old_name: &str,
        new_name: String,
    ) -> anyhow::Result<()> {
        // Rename in config database
        crate::global_config()
            .rename_environment(old_name, new_name.clone())
            .await?;
        // Update local cache
        let mut environments = self.environments.write().await;
        if let Some(env) = environments.remove(old_name) {
            environments.insert(new_name, env);
        }
        Ok(())
    }

    /// Set current environment in config
    pub async fn set_current_environment_in_config(&self, name: String) -> anyhow::Result<()> {
        crate::global_config()
            .set_current_environment(name.clone())
            .await?;
        *self.current_env.write().await = Some(name);
        Ok(())
    }

    /// Set credentials for environment
    pub async fn set_environment_credentials(
        &self,
        env_name: &str,
        credentials_name: String,
    ) -> anyhow::Result<()> {
        // This functionality might need to be implemented in Config
        // For now, we'll need to get the environment, update it, and save it back
        if let Some(mut api_env) = crate::global_config().get_environment(env_name).await? {
            api_env.credentials_ref = credentials_name;
            crate::global_config().add_environment(api_env).await // This overwrites the existing one
        } else {
            anyhow::bail!("Environment '{}' not found", env_name)
        }
    }

    /// Get environment by name
    pub async fn get_environment(&self, name: &str) -> anyhow::Result<Option<Environment>> {
        Ok(crate::global_config().get_environment(name).await?)
    }

    /// Get current environment name (returns String for compatibility)
    pub async fn get_current_environment_name(&self) -> anyhow::Result<Option<String>> {
        crate::global_config().get_current_environment().await
    }

    pub async fn try_select_env(&self, name: &str) -> anyhow::Result<Environment> {
        self.environments
            .read()
            .await
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", name))
    }

    pub async fn list_environments(&self) -> Vec<String> {
        self.environments.read().await.keys().cloned().collect()
    }

    pub async fn delete_environment(&self, name: &str) -> anyhow::Result<()> {
        self.environments
            .write()
            .await
            .remove(name)
            .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", name))?;
        Ok(())
    }

    pub async fn rename_environment(&self, old_name: &str, new_name: String) -> anyhow::Result<()> {
        let mut environments = self.environments.write().await;
        let mut environment = environments
            .remove(old_name)
            .ok_or_else(|| anyhow::anyhow!("Environment '{}' not found", old_name))?;
        environment.name = new_name.clone();
        environments.insert(new_name, environment);
        Ok(())
    }

    // Selection methods
    pub async fn select_environment(&self, name: &str) -> anyhow::Result<()> {
        // Verify environment exists
        self.try_select_env(name).await?;
        *self.current_env.write().await = Some(name.to_string());
        Ok(())
    }

    pub async fn get_current_environment(&self) -> Option<String> {
        self.current_env.read().await.clone()
    }

    // Expose auth_manager for testing
    pub fn auth_manager(&self) -> &AuthManager {
        &self.auth_manager
    }

    /// Check if a token is expired
    fn is_expired(token: &TokenInfo) -> bool {
        if let Ok(elapsed) = token.expires_at.elapsed() {
            elapsed.as_secs() > 0
        } else {
            false // If we can't determine elapsed time, assume valid
        }
    }

    /// Get or refresh token for environment, with automatic authentication
    async fn get_or_refresh_token(&self, env_name: &str) -> anyhow::Result<TokenInfo> {
        // 1. Check memory cache first
        if let Some(token) = self.tokens.read().await.get(env_name) {
            if !Self::is_expired(token) {
                log::debug!("Using cached token for environment: {}", env_name);
                return Ok(token.clone());
            } else {
                log::debug!("Cached token expired for environment: {}", env_name);
            }
        }

        // 2. Check database for persisted token
        if let Some(token) = crate::global_config().get_token(env_name).await? {
            if !Self::is_expired(&token) {
                log::debug!(
                    "Found valid token in database for environment: {}",
                    env_name
                );
                // Update memory cache
                self.tokens
                    .write()
                    .await
                    .insert(env_name.to_string(), token.clone());
                return Ok(token);
            } else {
                log::debug!("Database token expired for environment: {}", env_name);
                // Clean up expired token
                let _ = crate::global_config().delete_token(env_name).await;
            }
        }

        // 3. Auto-authenticate
        log::info!("Auto-authenticating for environment: {}", env_name);
        self.authenticate_environment(env_name).await?;

        // 4. Get the newly created token
        self.tokens
            .read()
            .await
            .get(env_name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Authentication succeeded but token not found"))
    }

    /// Authenticate a specific environment
    async fn authenticate_environment(&self, env_name: &str) -> anyhow::Result<()> {
        let environment = self.try_select_env(env_name).await?;
        let credentials = self
            .auth_manager
            .try_select_credentials(&environment.credentials_ref)
            .await?;

        // Authenticate
        self.auth_manager
            .authenticate(env_name, &environment.host, &credentials)
            .await?;

        // Get token and save to both memory and database
        if let Ok(token) = self.auth_manager.get_token(env_name).await {
            // Save to memory cache
            self.tokens
                .write()
                .await
                .insert(env_name.to_string(), token.clone());

            // Save to database for persistence
            crate::global_config()
                .save_token(env_name.to_string(), token)
                .await?;

            log::info!(
                "Successfully authenticated and cached token for environment: {}",
                env_name
            );
        }

        Ok(())
    }

    /// Get a configured DynamicsClient for the specified environment
    pub async fn get_client(&self, env_name: &str) -> anyhow::Result<DynamicsClient> {
        let environment = self.try_select_env(env_name).await?;

        // Get or refresh token with automatic authentication
        let token_info = self.get_or_refresh_token(env_name).await?;

        Ok(DynamicsClient::new(
            environment.host.clone(),
            token_info.access_token,
        ))
    }

    /// Get a configured DynamicsClient for the current environment
    pub async fn get_current_client(&self) -> anyhow::Result<DynamicsClient> {
        let current_env = self
            .current_env
            .read()
            .await
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No environment selected"))?
            .clone();
        self.get_client(&current_env).await
    }
}
