use std::time::SystemTime;

/// Environment configuration linking to credentials
#[derive(Debug, Clone)]
pub struct Environment {
    pub name: String,
    pub host: String,
    pub credentials_ref: String,
}

/// Set of credentials that can be shared across environments
#[derive(Debug, Clone)]
pub enum CredentialSet {
    UsernamePassword {
        username: String,
        password: String,
        client_id: String,
        client_secret: String,
    },
    ClientCredentials {
        client_id: String,
        client_secret: String,
        tenant_id: String,
    },
    DeviceCode {
        client_id: String,
        tenant_id: String,
    },
    Certificate {
        client_id: String,
        tenant_id: String,
        cert_path: String,
    },
}

/// Cached token information for an environment
#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub access_token: String,
    pub expires_at: SystemTime,
    pub refresh_token: Option<String>,
}
