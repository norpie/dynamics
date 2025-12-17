//! API Constants and Configuration for Dynamics 365 Web API

/// Dynamics 365 Web API version
pub const API_VERSION: &str = "v9.2";

/// Base API path for Dynamics 365
pub const API_BASE_PATH: &str = "/api/data";

/// Full API path with version
pub fn api_path() -> String {
    format!("{}/{}", API_BASE_PATH, API_VERSION)
}

/// Batch endpoint for multi-operation requests
pub const BATCH_ENDPOINT: &str = "$batch";

/// Content type for batch requests
pub const BATCH_CONTENT_TYPE: &str = "multipart/mixed";

/// Boundary string for batch requests
pub const BATCH_BOUNDARY: &str = "batch_dynamics_cli";

/// Change set boundary for atomic operations within batch
pub const CHANGESET_BOUNDARY: &str = "changeset_dynamics_cli";

/// Standard headers for Dynamics 365 requests
pub mod headers {
    /// Content type for JSON requests
    pub const CONTENT_TYPE_JSON: &str = "application/json";

    /// OData version header
    pub const ODATA_VERSION: &str = "4.0";

    /// Prefer header for returning representation
    pub const PREFER_RETURN_REPRESENTATION: &str = "return=representation";

    /// Prefer header for minimal metadata
    pub const PREFER_MINIMAL_METADATA: &str = "odata.metadata=minimal";

    /// Header to get entity ID on create
    pub const PREFER_INCLUDE_ANNOTATIONS: &str = "odata.include-annotations=\"*\"";

    /// If-Match header for updates (any version)
    pub const IF_MATCH_ANY: &str = "*";

    /// Correlation ID header for request tracking
    pub const X_CORRELATION_ID: &str = "x-correlation-id";

    // Bypass business logic headers
    /// Bypass synchronous and/or asynchronous custom plugins and workflows
    /// Values: "CustomSync", "CustomAsync", or "CustomSync,CustomAsync"
    pub const BYPASS_BUSINESS_LOGIC: &str = "MSCRM.BypassBusinessLogicExecution";

    /// Bypass specific plugin steps by GUID (comma-separated, max 3 by default)
    pub const BYPASS_STEP_IDS: &str = "MSCRM.BypassBusinessLogicExecutionStepIds";

    /// Bypass Power Automate flows triggered by Dataverse events (no privilege required)
    pub const SUPPRESS_POWER_AUTOMATE: &str = "MSCRM.SuppressCallbackRegistrationExpanderJob";
}

/// HTTP methods for operations
pub mod methods {
    pub const GET: &str = "GET";
    pub const POST: &str = "POST";
    pub const PATCH: &str = "PATCH";
    pub const DELETE: &str = "DELETE";
}

/// Build full entity endpoint URL
pub fn entity_endpoint(base_url: &str, entity: &str) -> String {
    format!("{}{}/{}", base_url, api_path(), entity)
}

/// Build entity record endpoint URL
pub fn entity_record_endpoint(base_url: &str, entity: &str, id: &str) -> String {
    format!("{}{}/{}({})", base_url, api_path(), entity, id)
}

/// Build upsert endpoint URL with alternate key
pub fn upsert_endpoint(base_url: &str, entity: &str, key_field: &str, key_value: &str) -> String {
    format!("{}{}/{}({}='{}')", base_url, api_path(), entity, key_field, key_value)
}

/// Build batch endpoint URL
pub fn batch_endpoint(base_url: &str) -> String {
    format!("{}{}/{}", base_url, api_path(), BATCH_ENDPOINT)
}

// === Schema/Metadata Endpoints ===

/// Build entity definition endpoint for creating attributes
/// POST /EntityDefinitions(LogicalName='{entity}')/Attributes
pub fn entity_attributes_endpoint(base_url: &str, entity: &str) -> String {
    format!(
        "{}{}/EntityDefinitions(LogicalName='{}')/Attributes",
        base_url,
        api_path(),
        entity
    )
}

/// Build specific attribute endpoint for update/delete
/// PUT/DELETE /EntityDefinitions(LogicalName='{entity}')/Attributes(LogicalName='{attribute}')
pub fn entity_attribute_endpoint(base_url: &str, entity: &str, attribute: &str) -> String {
    format!(
        "{}{}/EntityDefinitions(LogicalName='{}')/Attributes(LogicalName='{}')",
        base_url,
        api_path(),
        entity,
        attribute
    )
}

/// Build PublishAllXml endpoint
/// POST /PublishAllXml
pub fn publish_all_xml_endpoint(base_url: &str) -> String {
    format!("{}{}/PublishAllXml", base_url, api_path())
}