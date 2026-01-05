use super::constants::{self, headers, methods};
use super::operations::{Operation, OperationResult, BatchRequestBuilder, BatchResponseParser};
use super::query::{Query, QueryResult, QueryResponse};
use super::resilience::{RetryPolicy, RetryConfig, ResilienceConfig, RateLimiter, ConcurrencyLimiter, ApiLogger, OperationContext, OperationMetrics, MetricsCollector, BypassConfig};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// Build HTTP headers for bypassing Dynamics 365 custom business logic
fn build_bypass_headers(config: &BypassConfig) -> Vec<(&'static str, String)> {
    let mut bypass_headers = Vec::new();

    // Build BypassBusinessLogicExecution value (CustomSync, CustomAsync, or both)
    let mut bypass_values = Vec::new();
    if config.custom_sync {
        bypass_values.push("CustomSync");
    }
    if config.custom_async {
        bypass_values.push("CustomAsync");
    }
    if !bypass_values.is_empty() {
        bypass_headers.push((headers::BYPASS_BUSINESS_LOGIC, bypass_values.join(",")));
    }

    // Specific plugin step IDs to bypass
    if !config.step_ids.is_empty() {
        bypass_headers.push((headers::BYPASS_STEP_IDS, config.step_ids.join(",")));
    }

    // Power Automate flows (no privilege required)
    if config.power_automate_flows {
        bypass_headers.push((headers::SUPPRESS_POWER_AUTOMATE, "true".to_string()));
    }

    // Log bypass headers if any are configured
    if !bypass_headers.is_empty() {
        log::debug!("Bypass headers configured: {:?}", bypass_headers);
    } else {
        log::debug!("No bypass headers configured");
    }

    bypass_headers
}

/// Modern Dynamics 365 Web API client with connection pooling
#[derive(Clone)]
pub struct DynamicsClient {
    base_url: String,
    http_client: reqwest::Client,
    access_token: String,
    retry_policy: RetryPolicy, // Default retry policy for backwards compatibility
    rate_limiter: RateLimiter, // Global rate limiter for this client instance
    concurrency_limiter: ConcurrencyLimiter, // Global concurrency limiter for this client instance
    api_logger: ApiLogger, // Structured logger for operations
    metrics_collector: MetricsCollector, // Performance metrics collector
}

/// Entity metadata from EntityDefinitions (EntitySetName, IsIntersect, PrimaryNameAttribute, etc.)
#[derive(Debug, Clone)]
pub struct EntityMetadataInfo {
    pub entity_set_name: String,
    pub is_intersect: bool,
    pub primary_name_attribute: Option<String>,
}

/// Incoming reference from another entity (OneToMany relationship)
#[derive(Debug, Clone)]
pub struct IncomingReference {
    pub referencing_entity: String,
    pub referencing_attribute: String,
}

/// ManyToMany relationship metadata from EntityDefinitions
#[derive(Debug, Clone)]
pub struct ManyToManyRelationship {
    /// Junction/intersect entity name (e.g., "accountleads")
    pub intersect_entity_name: String,
    /// First entity in the relationship
    pub entity1_logical_name: String,
    /// Second entity in the relationship
    pub entity2_logical_name: String,
    /// FK field in junction for entity1 (e.g., "accountid")
    pub entity1_intersect_attribute: String,
    /// FK field in junction for entity2 (e.g., "leadid")
    pub entity2_intersect_attribute: String,
    /// Navigation property name on entity1
    pub entity1_navigation_property: String,
    /// Navigation property name on entity2
    pub entity2_navigation_property: String,
}

impl DynamicsClient {
    /// Apply rate limiting and concurrency limiting using the client's global limiters
    /// Returns an owned semaphore permit that must be held for the duration of the request
    async fn apply_rate_limiting(&self) -> anyhow::Result<tokio::sync::OwnedSemaphorePermit> {
        // First acquire concurrency permit (limits concurrent requests)
        let permit = self.concurrency_limiter.acquire().await;
        // Then apply rate limiting (limits request rate)
        self.rate_limiter.acquire().await;
        Ok(permit)
    }

    /// Get rate limiter statistics for monitoring
    pub fn rate_limiter_stats(&self) -> crate::api::resilience::RateLimiterStats {
        self.rate_limiter.stats()
    }

    /// Get concurrency limiter statistics for monitoring
    pub fn concurrency_limiter_stats(&self) -> crate::api::resilience::ConcurrencyStats {
        self.concurrency_limiter.stats()
    }

    /// Get the maximum number of queue items that can run concurrently
    pub fn max_queue_items(&self) -> usize {
        self.concurrency_limiter.max_queue_items()
    }

    /// Get performance metrics snapshot
    pub fn metrics_snapshot(&self) -> crate::api::resilience::MetricsSnapshot {
        self.metrics_collector.snapshot()
    }
    pub fn new(base_url: String, access_token: String) -> Self {
        let http_client = reqwest::Client::builder()
            .pool_max_idle_per_host(10)           // Max idle connections per host
            .pool_idle_timeout(Duration::from_secs(90))  // Keep connections alive for 90s
            .timeout(Duration::from_secs(600))    // Request timeout (10 minutes for batch operations)
            .connect_timeout(Duration::from_secs(10))    // Connection timeout
            .user_agent("dynamics-cli/1.0")       // Custom user agent
            .build()
            .expect("Failed to build HTTP client");

        let default_config = ResilienceConfig::default();
        Self {
            base_url,
            http_client,
            access_token,
            retry_policy: RetryPolicy::default(),
            rate_limiter: RateLimiter::new(default_config.rate_limit.clone()),
            concurrency_limiter: ConcurrencyLimiter::new(default_config.concurrency.clone()),
            api_logger: ApiLogger::new(default_config.monitoring.clone()),
            metrics_collector: MetricsCollector::new(default_config.monitoring),
        }
    }

    /// Create a new client with custom HTTP client configuration
    pub fn with_custom_client(base_url: String, access_token: String, http_client: reqwest::Client) -> Self {
        let default_config = ResilienceConfig::default();
        Self {
            base_url,
            http_client,
            access_token,
            retry_policy: RetryPolicy::default(),
            rate_limiter: RateLimiter::new(default_config.rate_limit.clone()),
            concurrency_limiter: ConcurrencyLimiter::new(default_config.concurrency.clone()),
            api_logger: ApiLogger::new(default_config.monitoring.clone()),
            metrics_collector: MetricsCollector::new(default_config.monitoring),
        }
    }

    /// Create a new client with custom retry policy
    pub fn with_retry_policy(base_url: String, access_token: String, retry_config: RetryConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            .timeout(Duration::from_secs(600))
            .connect_timeout(Duration::from_secs(10))
            .user_agent("dynamics-cli/1.0")
            .build()
            .expect("Failed to build HTTP client");

        let default_config = ResilienceConfig::default();
        Self {
            base_url,
            http_client,
            access_token,
            retry_policy: RetryPolicy::new(retry_config),
            rate_limiter: RateLimiter::new(default_config.rate_limit.clone()),
            concurrency_limiter: ConcurrencyLimiter::new(default_config.concurrency.clone()),
            api_logger: ApiLogger::new(default_config.monitoring.clone()),
            metrics_collector: MetricsCollector::new(default_config.monitoring),
        }
    }

    /// Execute a single operation
    pub async fn execute(&self, operation: &Operation, resilience: &ResilienceConfig) -> anyhow::Result<OperationResult> {
        match operation {
            Operation::Create { entity, data } => self.create_record(entity, data, resilience).await,
            Operation::CreateWithRefs { .. } => {
                // CreateWithRefs should only be used in batch operations
                Err(anyhow::anyhow!(
                    "CreateWithRefs operation can only be executed within a batch changeset. Use execute_batch() instead."
                ))
            }
            Operation::Update { entity, id, data } => self.update_record(entity, id, data, resilience).await,
            Operation::Delete { entity, id } => self.delete_record(entity, id, resilience).await,
            Operation::Upsert { entity, key_field, key_value, data } => {
                self.upsert_record(entity, key_field, key_value, data, resilience).await
            }
            Operation::AssociateRef { entity, entity_ref, navigation_property, target_ref } => {
                self.associate_ref(entity, entity_ref, navigation_property, target_ref, resilience).await
            }
            Operation::DisassociateRef { entity, entity_ref, navigation_property, target_id } => {
                self.disassociate_ref(entity, entity_ref, navigation_property, target_id, resilience).await
            }
            // Schema/Metadata operations
            Operation::CreateAttribute { entity, attribute_data, solution_name } => {
                self.create_attribute(entity, attribute_data, solution_name.as_deref(), resilience).await
            }
            Operation::UpdateAttribute { entity, attribute, attribute_data } => {
                self.update_attribute(entity, attribute, attribute_data, resilience).await
            }
            Operation::DeleteAttribute { entity, attribute } => {
                self.delete_attribute(entity, attribute, resilience).await
            }
            Operation::PublishAllXml => {
                self.publish_all_xml(resilience).await
            }
        }
    }

    /// Execute multiple operations as a batch
    pub async fn execute_batch(&self, operations: &[Operation], resilience: &ResilienceConfig) -> anyhow::Result<Vec<OperationResult>> {
        if operations.is_empty() {
            return Ok(Vec::new());
        }

        if operations.len() == 1 {
            let result = self.execute(&operations[0], resilience).await?;
            return Ok(vec![result]);
        }

        self.execute_batch_request(operations, resilience).await
    }

    /// Execute an OData query
    pub async fn execute_query(&self, query: &Query) -> anyhow::Result<QueryResult> {
        let url = constants::entity_endpoint(&self.base_url, &query.entity);

        // Build params, excluding $top since we'll use Prefer: odata.maxpagesize instead
        let mut params = query.to_query_params();
        params.remove("$top");

        // Build Prefer header with annotations and optional maxpagesize
        let prefer = match query.top {
            Some(page_size) => format!(
                "odata.include-annotations=\"OData.Community.Display.V1.FormattedValue\",odata.maxpagesize={}",
                page_size
            ),
            None => "odata.include-annotations=\"OData.Community.Display.V1.FormattedValue\"".to_string(),
        };

        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(&url)
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header("Prefer", &prefer)
                .query(&params)
                .send()
                .await
        }).await?;

        self.parse_query_response(response).await
    }

    /// Execute FetchXML query directly (for FQL compatibility)
    pub async fn execute_fetchxml(&self, entity_name: &str, fetchxml: &str) -> anyhow::Result<Value> {
        let _permit = self.apply_rate_limiting().await?;

        let encoded_fetchxml = urlencoding::encode(fetchxml);

        // Pluralize entity name for the endpoint
        let plural_entity = super::pluralization::pluralize_entity_name(entity_name);

        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(&format!("{}{}/{}?fetchXml={}", self.base_url, constants::api_path(), plural_entity, encoded_fetchxml))
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header("OData-MaxVersion", headers::ODATA_VERSION)
                .header("Prefer", headers::PREFER_INCLUDE_ANNOTATIONS)
                .send()
                .await
        }).await?;

        let query_result = self.parse_query_response(response).await?;
        match query_result.data {
            Some(query_response) => {
                // Return the structured OData response
                let mut result = serde_json::json!({
                    "value": query_response.value
                });
                if let Some(count) = query_response.count {
                    result["@odata.count"] = serde_json::Value::from(count);
                }
                if let Some(next_link) = query_response.next_link {
                    result["@odata.nextLink"] = serde_json::Value::from(next_link);
                }
                Ok(result)
            },
            None => Ok(serde_json::json!({"value": []}))
        }
    }

    /// Execute a request to a navigation property (for N:N relationships)
    /// Example: nrq_questionnaires(<id>)/nrq_questionnaire_nrq_Category_nrq_Category
    pub async fn execute_navigation_property(
        &self,
        entity_collection: &str,
        entity_id: &str,
        navigation_property: &str,
        select_fields: Option<Vec<String>>,
    ) -> anyhow::Result<Value> {
        let _permit = self.apply_rate_limiting().await?;

        let mut url = format!("{}{}/{}({})/{}",
            self.base_url,
            constants::api_path(),
            entity_collection,
            entity_id,
            navigation_property
        );

        if let Some(fields) = select_fields {
            url.push_str(&format!("?$select={}", fields.join(",")));
        }

        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(&url)
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header("OData-MaxVersion", headers::ODATA_VERSION)
                .header("Prefer", headers::PREFER_INCLUDE_ANNOTATIONS)
                .send()
                .await
        }).await?;

        let query_result = self.parse_query_response(response).await?;
        match query_result.data {
            Some(query_response) => {
                Ok(serde_json::json!({
                    "value": query_response.value
                }))
            },
            None => Ok(serde_json::json!({"value": []}))
        }
    }

    /// Execute the next page of results using @odata.nextLink
    pub async fn execute_next_page(&self, next_link: &str, page_size: Option<u32>) -> anyhow::Result<QueryResult> {
        // Build Prefer header with annotations and optional maxpagesize
        let prefer = match page_size {
            Some(size) => format!(
                "odata.include-annotations=\"OData.Community.Display.V1.FormattedValue\",odata.maxpagesize={}",
                size
            ),
            None => "odata.include-annotations=\"OData.Community.Display.V1.FormattedValue\"".to_string(),
        };

        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(next_link)
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header("Prefer", &prefer)
                .send()
                .await
        }).await?;

        self.parse_query_response(response).await
    }

    /// Execute a raw HTTP request to the Dynamics API
    ///
    /// This method provides direct access to the Dynamics 365 Web API for any endpoint.
    /// The endpoint parameter is automatically URL-encoded and prefixed with the API base path.
    ///
    /// # Arguments
    /// * `method` - HTTP method (GET, POST, PATCH, DELETE)
    /// * `endpoint` - API endpoint path (e.g., "accounts?$select=name&$top=5")
    /// * `data` - Optional JSON data string for POST/PATCH requests
    ///
    /// # Returns
    /// JSON response as `serde_json::Value`
    pub async fn execute_raw(&self, method: &str, endpoint: &str, data: Option<&str>) -> anyhow::Result<Value> {
        let _permit = self.apply_rate_limiting().await?;

        // Build full URL
        let url = if endpoint.starts_with("http") {
            // Absolute URL provided
            endpoint.to_string()
        } else {
            // Relative path - construct full URL
            let base = format!("{}{}", self.base_url, constants::api_path());
            let trimmed_endpoint = endpoint.trim_start_matches('/');
            format!("{}/{}", base, trimmed_endpoint)
        };

        // Parse JSON data if provided
        let json_data: Option<Value> = if let Some(d) = data {
            Some(serde_json::from_str(d)
                .map_err(|e| anyhow::anyhow!("Failed to parse JSON data: {}", e))?)
        } else {
            None
        };

        // Validate HTTP method first
        let method_upper = method.to_uppercase();
        if !["GET", "POST", "PATCH", "DELETE"].contains(&method_upper.as_str()) {
            return Err(anyhow::anyhow!("Unsupported HTTP method: {}", method));
        }

        // Execute request with retry policy
        let response = self.retry_policy.execute(|| async {
            let mut request = match method_upper.as_str() {
                "GET" => self.http_client.get(&url),
                "POST" => self.http_client.post(&url),
                "PATCH" => self.http_client.patch(&url),
                "DELETE" => self.http_client.delete(&url),
                _ => unreachable!(), // Already validated above
            };

            request = request
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header("OData-MaxVersion", headers::ODATA_VERSION);

            // Add body for POST/PATCH
            if let Some(ref json) = json_data {
                request = request
                    .header("Content-Type", headers::CONTENT_TYPE_JSON)
                    .json(json);
            }

            request.send().await
        }).await?;

        let status = response.status();

        // Handle different status codes
        if status.is_success() {
            // Try to parse as JSON, return empty object if no content
            if status == reqwest::StatusCode::NO_CONTENT {
                Ok(serde_json::json!({}))
            } else {
                let text = response.text().await?;
                if text.is_empty() {
                    Ok(serde_json::json!({}))
                } else {
                    serde_json::from_str(&text)
                        .map_err(|e| anyhow::anyhow!("Failed to parse response as JSON: {}", e))
                }
            }
        } else {
            // Error response - try to extract error details
            let error_text = response.text().await?;
            Err(anyhow::anyhow!(
                "API request failed with status {}: {}",
                status,
                error_text
            ))
        }
    }

    /// Create a new record
    async fn create_record(&self, entity: &str, data: &Value, resilience: &ResilienceConfig) -> anyhow::Result<OperationResult> {
        let url = constants::entity_endpoint(&self.base_url, entity);
        let correlation_id = uuid::Uuid::new_v4().to_string();

        // Start operation tracking
        let logger = ApiLogger::new(resilience.monitoring.clone());
        let mut context = logger.start_operation("create", entity, &correlation_id);

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        // Log request details
        let mut request_headers = HashMap::new();
        request_headers.insert("Content-Type".to_string(), headers::CONTENT_TYPE_JSON.to_string());
        request_headers.insert("OData-Version".to_string(), headers::ODATA_VERSION.to_string());
        request_headers.insert(headers::X_CORRELATION_ID.to_string(), correlation_id.clone());
        logger.log_request(&context, "POST", &url, &request_headers);

        // Log request body for debugging
        let body_str = serde_json::to_string_pretty(data).unwrap_or_default();
        log::debug!("Create request body:\n{}", body_str);

        let retry_policy = crate::api::resilience::RetryPolicy::new(resilience.retry.clone());
        let request_start = std::time::Instant::now();
        let bypass_headers = build_bypass_headers(&resilience.bypass);
        let response = retry_policy.execute(|| async {
            let mut request = self.http_client
                .post(&url)
                .bearer_auth(&self.access_token)
                .header("Content-Type", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header("Prefer", headers::PREFER_RETURN_REPRESENTATION)
                .header(headers::X_CORRELATION_ID, &correlation_id);

            // Apply bypass headers if configured
            for (name, value) in &bypass_headers {
                request = request.header(*name, value);
            }

            request.json(data).send().await
        }).await?;

        // Log response details
        let request_duration = request_start.elapsed();
        let status_code = response.status().as_u16();
        let mut response_headers = HashMap::new();
        for (name, value) in response.headers() {
            if let Ok(value_str) = value.to_str() {
                response_headers.insert(name.to_string(), value_str.to_string());
            }
        }
        logger.log_response(&context, status_code, &response_headers, request_duration);

        // Parse response and complete operation logging
        let result = self.parse_response(Operation::Create {
            entity: entity.to_string(),
            data: data.clone(),
        }, response).await;

        // Log operation completion and collect metrics
        let metrics = context.create_metrics(
            result.is_ok(),
            Some(status_code),
            result.as_ref().err().map(|e| e.to_string())
        );
        logger.complete_operation(&context, &metrics);

        // Collect performance metrics using the client's global collector
        self.metrics_collector.record_operation("create", entity, &metrics);

        result
    }

    /// Update an existing record
    async fn update_record(&self, entity: &str, id: &str, data: &Value, resilience: &ResilienceConfig) -> anyhow::Result<OperationResult> {
        let url = constants::entity_record_endpoint(&self.base_url, entity, id);
        let correlation_id = uuid::Uuid::new_v4().to_string();

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let retry_policy = crate::api::resilience::RetryPolicy::new(resilience.retry.clone());
        let bypass_headers = build_bypass_headers(&resilience.bypass);
        let response = retry_policy.execute(|| async {
            let mut request = self.http_client
                .patch(&url)
                .bearer_auth(&self.access_token)
                .header("Content-Type", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header("If-Match", headers::IF_MATCH_ANY)
                .header("Prefer", headers::PREFER_RETURN_REPRESENTATION)
                .header(headers::X_CORRELATION_ID, &correlation_id);

            // Apply bypass headers if configured
            for (name, value) in &bypass_headers {
                request = request.header(*name, value);
            }

            request.json(data).send().await
        }).await?;

        self.parse_response(Operation::Update {
            entity: entity.to_string(),
            id: id.to_string(),
            data: data.clone(),
        }, response).await
    }

    /// Delete a record
    async fn delete_record(&self, entity: &str, id: &str, resilience: &ResilienceConfig) -> anyhow::Result<OperationResult> {
        let url = constants::entity_record_endpoint(&self.base_url, entity, id);
        let correlation_id = uuid::Uuid::new_v4().to_string();

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let retry_policy = crate::api::resilience::RetryPolicy::new(resilience.retry.clone());
        let bypass_headers = build_bypass_headers(&resilience.bypass);
        let response = retry_policy.execute(|| async {
            let mut request = self.http_client
                .delete(&url)
                .bearer_auth(&self.access_token)
                .header("OData-Version", headers::ODATA_VERSION)
                .header(headers::X_CORRELATION_ID, &correlation_id);

            // Apply bypass headers if configured
            for (name, value) in &bypass_headers {
                request = request.header(*name, value);
            }

            request.send().await
        }).await?;

        self.parse_response(Operation::Delete {
            entity: entity.to_string(),
            id: id.to_string(),
        }, response).await
    }

    /// Upsert a record using alternate key
    async fn upsert_record(&self, entity: &str, key_field: &str, key_value: &str, data: &Value, resilience: &ResilienceConfig) -> anyhow::Result<OperationResult> {
        let url = constants::upsert_endpoint(&self.base_url, entity, key_field, key_value);
        let correlation_id = uuid::Uuid::new_v4().to_string();

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let retry_policy = crate::api::resilience::RetryPolicy::new(resilience.retry.clone());
        let bypass_headers = build_bypass_headers(&resilience.bypass);
        let response = retry_policy.execute(|| async {
            let mut request = self.http_client
                .patch(&url)
                .bearer_auth(&self.access_token)
                .header("Content-Type", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header("Prefer", headers::PREFER_RETURN_REPRESENTATION)
                .header(headers::X_CORRELATION_ID, &correlation_id);

            // Apply bypass headers if configured
            for (name, value) in &bypass_headers {
                request = request.header(*name, value);
            }

            request.json(data).send().await
        }).await?;

        self.parse_response(Operation::Upsert {
            entity: entity.to_string(),
            key_field: key_field.to_string(),
            key_value: key_value.to_string(),
            data: data.clone(),
        }, response).await
    }

    /// Associate records via navigation property ($ref)
    async fn associate_ref(&self, entity: &str, entity_ref: &str, navigation_property: &str, target_ref: &str, resilience: &ResilienceConfig) -> anyhow::Result<OperationResult> {
        // POST /entities(id)/navigation_property/$ref with body {"@odata.id": "target"}
        let url = format!("{}/{}({})/{}/$ref", self.base_url, entity, entity_ref, navigation_property);
        let correlation_id = uuid::Uuid::new_v4().to_string();

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let body = serde_json::json!({
            "@odata.id": target_ref
        });

        let retry_policy = crate::api::resilience::RetryPolicy::new(resilience.retry.clone());
        let bypass_headers = build_bypass_headers(&resilience.bypass);
        let response = retry_policy.execute(|| async {
            let mut request = self.http_client
                .post(&url)
                .bearer_auth(&self.access_token)
                .header("Content-Type", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header(headers::X_CORRELATION_ID, &correlation_id);

            // Apply bypass headers if configured
            for (name, value) in &bypass_headers {
                request = request.header(*name, value);
            }

            request.json(&body).send().await
        }).await?;

        self.parse_response(Operation::AssociateRef {
            entity: entity.to_string(),
            entity_ref: entity_ref.to_string(),
            navigation_property: navigation_property.to_string(),
            target_ref: target_ref.to_string(),
        }, response).await
    }

    /// Disassociate records via navigation property (remove N:N relationship)
    async fn disassociate_ref(&self, entity: &str, entity_ref: &str, navigation_property: &str, target_id: &str, resilience: &ResilienceConfig) -> anyhow::Result<OperationResult> {
        // DELETE /entities(id)/navigation_property(target_id)/$ref
        let url = format!("{}/{}({})/{}({})/$ref", self.base_url, entity, entity_ref, navigation_property, target_id);
        let correlation_id = uuid::Uuid::new_v4().to_string();

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let retry_policy = crate::api::resilience::RetryPolicy::new(resilience.retry.clone());
        let bypass_headers = build_bypass_headers(&resilience.bypass);
        let response = retry_policy.execute(|| async {
            let mut request = self.http_client
                .delete(&url)
                .bearer_auth(&self.access_token)
                .header("OData-Version", headers::ODATA_VERSION)
                .header(headers::X_CORRELATION_ID, &correlation_id);

            // Apply bypass headers if configured
            for (name, value) in &bypass_headers {
                request = request.header(*name, value);
            }

            request.send().await
        }).await?;

        self.parse_response(Operation::DisassociateRef {
            entity: entity.to_string(),
            entity_ref: entity_ref.to_string(),
            navigation_property: navigation_property.to_string(),
            target_id: target_id.to_string(),
        }, response).await
    }

    // === Schema/Metadata Operations ===

    /// Create a new attribute/column on an entity
    async fn create_attribute(&self, entity: &str, attribute_data: &Value, solution_name: Option<&str>, resilience: &ResilienceConfig) -> anyhow::Result<OperationResult> {
        let url = constants::entity_attributes_endpoint(&self.base_url, entity);
        let correlation_id = uuid::Uuid::new_v4().to_string();

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let retry_policy = crate::api::resilience::RetryPolicy::new(resilience.retry.clone());
        let response = retry_policy.execute(|| async {
            let mut request = self.http_client
                .post(&url)
                .bearer_auth(&self.access_token)
                .header("Content-Type", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header(headers::X_CORRELATION_ID, &correlation_id);

            // Add solution header if provided
            if let Some(solution) = solution_name {
                request = request.header("MSCRM.SolutionUniqueName", solution);
            }

            request.json(attribute_data).send().await
        }).await?;

        self.parse_response(Operation::CreateAttribute {
            entity: entity.to_string(),
            attribute_data: attribute_data.clone(),
            solution_name: solution_name.map(|s| s.to_string()),
        }, response).await
    }

    /// Update an existing attribute/column
    async fn update_attribute(&self, entity: &str, attribute: &str, attribute_data: &Value, resilience: &ResilienceConfig) -> anyhow::Result<OperationResult> {
        let url = constants::entity_attribute_endpoint(&self.base_url, entity, attribute);
        let correlation_id = uuid::Uuid::new_v4().to_string();

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let retry_policy = crate::api::resilience::RetryPolicy::new(resilience.retry.clone());
        let response = retry_policy.execute(|| async {
            self.http_client
                .put(&url)  // Schema updates use PUT, not PATCH
                .bearer_auth(&self.access_token)
                .header("Content-Type", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header(headers::X_CORRELATION_ID, &correlation_id)
                .json(attribute_data)
                .send()
                .await
        }).await?;

        self.parse_response(Operation::UpdateAttribute {
            entity: entity.to_string(),
            attribute: attribute.to_string(),
            attribute_data: attribute_data.clone(),
        }, response).await
    }

    /// Delete an attribute/column from an entity
    async fn delete_attribute(&self, entity: &str, attribute: &str, resilience: &ResilienceConfig) -> anyhow::Result<OperationResult> {
        let url = constants::entity_attribute_endpoint(&self.base_url, entity, attribute);
        let correlation_id = uuid::Uuid::new_v4().to_string();

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let retry_policy = crate::api::resilience::RetryPolicy::new(resilience.retry.clone());
        let response = retry_policy.execute(|| async {
            self.http_client
                .delete(&url)
                .bearer_auth(&self.access_token)
                .header("OData-Version", headers::ODATA_VERSION)
                .header(headers::X_CORRELATION_ID, &correlation_id)
                .send()
                .await
        }).await?;

        self.parse_response(Operation::DeleteAttribute {
            entity: entity.to_string(),
            attribute: attribute.to_string(),
        }, response).await
    }

    /// Publish all customizations to make schema changes active
    async fn publish_all_xml(&self, resilience: &ResilienceConfig) -> anyhow::Result<OperationResult> {
        let url = constants::publish_all_xml_endpoint(&self.base_url);
        let correlation_id = uuid::Uuid::new_v4().to_string();

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let retry_policy = crate::api::resilience::RetryPolicy::new(resilience.retry.clone());
        let response = retry_policy.execute(|| async {
            self.http_client
                .post(&url)
                .bearer_auth(&self.access_token)
                .header("Content-Type", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header(headers::X_CORRELATION_ID, &correlation_id)
                .send()
                .await
        }).await?;

        self.parse_response(Operation::PublishAllXml, response).await
    }

    /// Execute operations using the $batch endpoint
    async fn execute_batch_request(&self, operations: &[Operation], resilience: &ResilienceConfig) -> anyhow::Result<Vec<OperationResult>> {
        let url = constants::batch_endpoint(&self.base_url);
        let correlation_id = uuid::Uuid::new_v4().to_string();

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        // Build bypass headers to include on each operation within the batch
        let bypass_headers = build_bypass_headers(&resilience.bypass);

        // Build the batch request using the proper builder
        let batch_request = BatchRequestBuilder::new(&self.base_url)
            .with_bypass_headers(bypass_headers.clone())
            .add_changeset(operations)
            .build();

        let content_type = batch_request.content_type().to_string();
        let body = batch_request.body().to_string();

        log::debug!("Batch request: {} operations (correlation_id: {})", operations.len(), correlation_id);
        if !bypass_headers.is_empty() {
            log::debug!("Batch request includes bypass headers on each operation: {:?}", bypass_headers);
        }

        let retry_policy = crate::api::resilience::RetryPolicy::new(resilience.retry.clone());
        let request_start = std::time::Instant::now();
        let response = retry_policy.execute(|| async {
            let mut request = self.http_client
                .post(&url)
                .bearer_auth(&self.access_token)
                .header("Content-Type", content_type.clone())
                .header("OData-Version", headers::ODATA_VERSION)
                .header(headers::X_CORRELATION_ID, &correlation_id);

            // Apply bypass headers if configured
            for (name, value) in &bypass_headers {
                request = request.header(*name, value);
            }

            request.body(body.clone()).send().await
        }).await?;

        let request_duration = request_start.elapsed();
        let status_code = response.status().as_u16();

        // Warn if request took longer than expected
        if request_duration.as_secs() > 60 {
            log::warn!("⚠️ Batch request took {:.1}s - unusually slow! ({} ops, status={})",
                request_duration.as_secs_f32(), operations.len(), status_code);
        } else {
            log::info!("Batch completed: {} ops in {:.1}s (status={})",
                operations.len(), request_duration.as_secs_f32(), status_code);
        }

        let response_text = response.text().await?;

        // Log full response for debugging (first 2000 chars to avoid log spam)
        let truncated_response = if response_text.len() > 2000 {
            format!("{}... (truncated, total {} chars)", &response_text[..2000], response_text.len())
        } else {
            response_text.clone()
        };
        log::debug!("Batch response body:\n{}", truncated_response);

        if status_code >= 200 && status_code < 300 {
            // Use the proper parser
            let results = BatchResponseParser::parse(&response_text, operations)?;

            // Log any individual operation failures
            for (idx, result) in results.iter().enumerate() {
                if !result.success {
                    log::error!(
                        "Batch operation {} FAILED: {} on entity '{}' (status: {})",
                        idx + 1,
                        result.operation.operation_type(),
                        result.operation.entity(),
                        result.status_code.map(|s| s.to_string()).unwrap_or_else(|| "unknown".to_string())
                    );
                    if let Some(ref err) = result.error {
                        log::error!("  Error details: {}", err);
                    }
                }
            }

            Ok(results)
        } else {
            log::error!("Batch request FAILED (status {}): {}", status_code, response_text);
            anyhow::bail!("Batch request failed (status {}): {}", status_code, response_text)
        }
    }


    /// Parse HTTP response into OperationResult
    async fn parse_response(&self, operation: Operation, response: reqwest::Response) -> anyhow::Result<OperationResult> {
        let status_code = response.status().as_u16();
        let mut headers = HashMap::new();

        // Extract useful headers
        for (name, value) in response.headers() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(name.to_string(), value_str.to_string());
            }
        }

        if response.status().is_success() {
            let data = if response.status() == 204 {
                // No content (delete operations)
                None
            } else {
                // Get response text first, then try to parse as JSON
                let text = response.text().await.unwrap_or_default();
                if text.is_empty() {
                    None
                } else {
                    match serde_json::from_str::<Value>(&text) {
                        Ok(json) => Some(json),
                        Err(_) => Some(Value::String(text)),
                    }
                }
            };

            Ok(OperationResult {
                operation,
                success: true,
                data,
                error: None,
                status_code: Some(status_code),
                headers,
            })
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());

            // Log the error
            log::error!(
                "Operation FAILED: {} on entity '{}' (status: {})",
                operation.operation_type(),
                operation.entity(),
                status_code
            );
            log::error!("  Error details: {}", error_text);

            Ok(OperationResult {
                operation,
                success: false,
                data: None,
                error: Some(error_text),
                status_code: Some(status_code),
                headers,
            })
        }
    }

    /// Parse HTTP response into QueryResult
    async fn parse_query_response(&self, response: reqwest::Response) -> anyhow::Result<QueryResult> {
        let status_code = response.status().as_u16();
        let mut headers = HashMap::new();

        // Extract useful headers
        for (name, value) in response.headers() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(name.to_string(), value_str.to_string());
            }
        }

        if response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            if text.is_empty() {
                return Ok(QueryResult::error(
                    "Empty response from server".to_string(),
                    Some(status_code),
                    headers,
                ));
            }

            match serde_json::from_str::<Value>(&text) {
                Ok(json) => {
                    match QueryResponse::from_json(json) {
                        Ok(query_response) => Ok(QueryResult::success(query_response, status_code, headers)),
                        Err(e) => Ok(QueryResult::error(
                            format!("Failed to parse OData response: {}", e),
                            Some(status_code),
                            headers,
                        )),
                    }
                },
                Err(e) => Ok(QueryResult::error(
                    format!("Invalid JSON response: {}", e),
                    Some(status_code),
                    headers,
                )),
            }
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            Ok(QueryResult::error(error_text, Some(status_code), headers))
        }
    }

    /// Fetch entity metadata from Dynamics 365 $metadata endpoint
    pub async fn fetch_metadata(&self) -> anyhow::Result<String> {
        let metadata_url = format!("{}/{}/$metadata", self.base_url, constants::api_path());

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(&metadata_url)
                .bearer_auth(&self.access_token)
                .header("Accept", "application/xml")
                .header("OData-Version", headers::ODATA_VERSION)
                .send()
                .await
        }).await?;

        let status = response.status();
        if status.is_success() {
            let metadata_xml = response.text().await?;
            Ok(metadata_xml)
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Metadata fetch failed with status {}: {}", status, error_text)
        }
    }

    /// Fetch entity field definitions from $metadata endpoint (includes navigation properties like _value fields)
    pub async fn fetch_entity_fields(&self, entity_name: &str) -> anyhow::Result<Vec<super::metadata::FieldMetadata>> {
        use roxmltree::Document;

        let metadata_xml = self.fetch_metadata().await?;
        let doc = Document::parse(&metadata_xml)
            .map_err(|e| anyhow::anyhow!("Failed to parse metadata XML: {}", e))?;

        // Find the EntityType element for our entity
        let entity_type = doc
            .descendants()
            .find(|node| {
                node.has_tag_name("EntityType")
                    && node.attribute("Name")
                        .is_some_and(|name| name.eq_ignore_ascii_case(entity_name))
            })
            .ok_or_else(|| anyhow::anyhow!("Entity '{}' not found in metadata", entity_name))?;

        let mut fields = Vec::new();

        // Parse Property elements (actual attributes)
        for property in entity_type.children().filter(|n| n.has_tag_name("Property")) {
            if let Some(field_name) = property.attribute("Name") {
                let field_type_str = property.attribute("Type").unwrap_or("unknown");
                let nullable = property.attribute("Nullable").map(|v| v == "true").unwrap_or(true);
                let is_required = !nullable;

                let field_type = Self::parse_field_type(field_type_str, None);

                fields.push(super::metadata::FieldMetadata {
                    logical_name: field_name.to_string(),
                    schema_name: None, // Not available in XML metadata
                    display_name: None,
                    field_type,
                    is_required,
                    is_primary_key: false, // TODO: detect from Key element
                    max_length: None,
                    related_entity: None,
                    option_values: vec![], // Not available in XML metadata
                });
            }
        }

        // Parse NavigationProperty elements (relationships)
        for nav_prop in entity_type.children().filter(|n| n.has_tag_name("NavigationProperty")) {
            if let Some(field_name) = nav_prop.attribute("Name") {
                let field_type_str = nav_prop.attribute("Type").unwrap_or("unknown");

                // Extract target entity from navigation property type
                // Format: "Collection(Microsoft.Dynamics.CRM.account)" or "Microsoft.Dynamics.CRM.account"
                let related_entity = field_type_str
                    .strip_prefix("Collection(Microsoft.Dynamics.CRM.")
                    .and_then(|s| s.strip_suffix(")"))
                    .or_else(|| field_type_str.strip_prefix("Microsoft.Dynamics.CRM."))
                    .map(|s| s.to_string());

                // Determine relationship type from Type attribute format
                // Collection(...) = OneToMany or ManyToMany
                // Non-collection = ManyToOne (lookup)
                let is_collection = field_type_str.starts_with("Collection(");

                // Store relationship cardinality in a pseudo-field type
                // We'll extract this later when building relationships
                let relationship_cardinality = if is_collection {
                    "OneToMany"  // Collection relationship
                } else {
                    "ManyToOne"  // Lookup relationship
                };

                // Use Other type to store cardinality for collection relationships
                let field_type = if is_collection {
                    super::metadata::FieldType::Other(format!("Relationship:{}", relationship_cardinality))
                } else {
                    super::metadata::FieldType::Lookup  // ManyToOne stays as Lookup
                };

                fields.push(super::metadata::FieldMetadata {
                    logical_name: field_name.to_string(),
                    schema_name: None, // Not available in XML metadata
                    display_name: None,
                    field_type,
                    is_required: false,
                    is_primary_key: false,
                    max_length: None,
                    related_entity,
                    option_values: vec![], // Not available in XML metadata
                });
            }
        }

        Ok(fields)
    }

    fn parse_field_type(type_str: &str, targets: Option<&Vec<serde_json::Value>>) -> super::metadata::FieldType {
        match type_str {
            "Edm.String" => super::metadata::FieldType::String,
            "Edm.Int32" => super::metadata::FieldType::Integer,
            "Edm.Decimal" | "Edm.Double" => super::metadata::FieldType::Decimal,
            "Edm.Boolean" => super::metadata::FieldType::Boolean,
            "Edm.DateTime" | "Edm.DateTimeOffset" => super::metadata::FieldType::DateTime,
            "Edm.Guid" => super::metadata::FieldType::UniqueIdentifier,
            other => super::metadata::FieldType::Other(other.to_string()),
        }
    }

    /// Fetch option set values for all picklist-type attributes of an entity
    /// Returns a map of logical_name -> Vec<OptionSetValue>
    async fn fetch_picklist_optionsets(&self, entity_name: &str) -> anyhow::Result<HashMap<String, Vec<super::metadata::OptionSetValue>>> {
        let mut result = HashMap::new();

        // Fetch from PicklistAttributeMetadata (regular picklists)
        let picklist_url = format!(
            "{}/{}/EntityDefinitions(LogicalName='{}')/Attributes/Microsoft.Dynamics.CRM.PicklistAttributeMetadata?$select=LogicalName&$expand=OptionSet($select=Options)",
            self.base_url,
            constants::api_path(),
            entity_name
        );

        // Fetch from StateAttributeMetadata (state fields)
        let state_url = format!(
            "{}/{}/EntityDefinitions(LogicalName='{}')/Attributes/Microsoft.Dynamics.CRM.StateAttributeMetadata?$select=LogicalName&$expand=OptionSet($select=Options)",
            self.base_url,
            constants::api_path(),
            entity_name
        );

        // Fetch from StatusAttributeMetadata (status fields)
        let status_url = format!(
            "{}/{}/EntityDefinitions(LogicalName='{}')/Attributes/Microsoft.Dynamics.CRM.StatusAttributeMetadata?$select=LogicalName&$expand=OptionSet($select=Options)",
            self.base_url,
            constants::api_path(),
            entity_name
        );

        // Fetch all three in parallel
        let _permit = self.apply_rate_limiting().await?;

        let (picklist_result, state_result, status_result) = tokio::join!(
            self.fetch_optionset_from_url(&picklist_url),
            self.fetch_optionset_from_url(&state_url),
            self.fetch_optionset_from_url(&status_url)
        );

        // Merge results
        if let Ok(map) = picklist_result {
            result.extend(map);
        }
        if let Ok(map) = state_result {
            result.extend(map);
        }
        if let Ok(map) = status_result {
            result.extend(map);
        }

        Ok(result)
    }

    /// Helper to fetch option sets from a specific attribute type URL
    async fn fetch_optionset_from_url(&self, url: &str) -> anyhow::Result<HashMap<String, Vec<super::metadata::OptionSetValue>>> {
        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(url)
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .send()
                .await
        }).await?;

        let mut result = HashMap::new();

        if response.status().is_success() {
            let json: Value = response.json().await?;
            if let Some(attributes) = json["value"].as_array() {
                for attr in attributes {
                    if let Some(logical_name) = attr["LogicalName"].as_str() {
                        if let Some(options) = attr["OptionSet"]["Options"].as_array() {
                            let option_values: Vec<super::metadata::OptionSetValue> = options
                                .iter()
                                .filter_map(|opt| {
                                    let value = opt["Value"].as_i64()?;
                                    let label = opt["Label"]["UserLocalizedLabel"]["Label"]
                                        .as_str()
                                        .map(|s| s.to_string());
                                    Some(super::metadata::OptionSetValue { value, label })
                                })
                                .collect();
                            result.insert(logical_name.to_string(), option_values);
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    /// Fetch entity field definitions from EntityDefinitions endpoint (attributes only, no navigation properties)
    pub async fn fetch_entity_fields_alt(&self, entity_name: &str) -> anyhow::Result<Vec<super::metadata::FieldMetadata>> {
        let url = format!(
            "{}/{}/EntityDefinitions(LogicalName='{}')/Attributes",
            self.base_url,
            constants::api_path(),
            entity_name
        );

        // Fetch picklist option sets in parallel (requires type-specific endpoint)
        let optionset_map = self.fetch_picklist_optionsets(entity_name).await.unwrap_or_default();

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(&url)
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .send()
                .await
        }).await?;

        let status = response.status();
        if status.is_success() {
            let json: Value = response.json().await?;
            let attributes = json["value"].as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected 'value' array in response"))?;

            let fields = attributes.iter()
                .filter_map(|attr| {
                    let logical_name = attr["LogicalName"].as_str()?.to_string();
                    let schema_name = attr["SchemaName"].as_str().map(|s| s.to_string());
                    let display_name = attr["DisplayName"]["UserLocalizedLabel"]["Label"].as_str()
                        .map(|s| s.to_string());
                    let is_required = attr["RequiredLevel"]["Value"].as_str() == Some("ApplicationRequired")
                        || attr["RequiredLevel"]["Value"].as_str() == Some("SystemRequired");
                    let is_primary_key = attr["IsPrimaryId"].as_bool().unwrap_or(false);
                    let max_length = attr["MaxLength"].as_i64().map(|l| l as i32);

                    let field_type = match attr["AttributeType"].as_str()? {
                        "String" => super::metadata::FieldType::String,
                        "Integer" => super::metadata::FieldType::Integer,
                        "Decimal" | "Double" => super::metadata::FieldType::Decimal,
                        "Boolean" => super::metadata::FieldType::Boolean,
                        "DateTime" => super::metadata::FieldType::DateTime,
                        "Lookup" | "Customer" | "Owner" => {
                            let related_entity = attr["Targets"].as_array()
                                .and_then(|targets| targets.first())
                                .and_then(|t| t.as_str())
                                .map(|s| s.to_string());
                            super::metadata::FieldType::Lookup
                        },
                        "Picklist" | "State" | "Status" => super::metadata::FieldType::OptionSet,
                        "Money" => super::metadata::FieldType::Money,
                        "Memo" => super::metadata::FieldType::Memo,
                        "Uniqueidentifier" => super::metadata::FieldType::UniqueIdentifier,
                        "Virtual" => {
                            // Check AttributeTypeName.Value to distinguish MultiSelectPicklist
                            // from computed virtual fields (like *name, *yominame)
                            let type_name = attr["AttributeTypeName"]["Value"].as_str().unwrap_or("Virtual");
                            if type_name == "MultiSelectPicklistType" {
                                super::metadata::FieldType::MultiSelectOptionSet
                            } else {
                                super::metadata::FieldType::Other("Virtual".to_string())
                            }
                        },
                        other => super::metadata::FieldType::Other(other.to_string()),
                    };

                    let related_entity = if matches!(field_type, super::metadata::FieldType::Lookup) {
                        attr["Targets"].as_array()
                            .and_then(|targets| targets.first())
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    };

                    // Get option values from pre-fetched optionset_map for OptionSet/MultiSelectOptionSet fields
                    let option_values = if matches!(field_type,
                        super::metadata::FieldType::OptionSet |
                        super::metadata::FieldType::MultiSelectOptionSet)
                    {
                        optionset_map.get(&logical_name).cloned().unwrap_or_default()
                    } else {
                        vec![]
                    };

                    Some(super::metadata::FieldMetadata {
                        logical_name,
                        schema_name,
                        display_name,
                        field_type,
                        is_required,
                        is_primary_key,
                        max_length,
                        related_entity,
                        option_values,
                    })
                })
                .collect();

            Ok(fields)
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Field metadata fetch failed with status {}: {}", status, error_text)
        }
    }

    /// Fetch raw attribute metadata for an entity
    /// Returns a HashMap keyed by logical_name containing the full attribute JSON
    /// Used for CreateAttribute operations during schema sync
    pub async fn fetch_entity_attributes_raw(&self, entity_name: &str) -> anyhow::Result<HashMap<String, Value>> {
        let url = format!(
            "{}/{}/EntityDefinitions(LogicalName='{}')/Attributes",
            self.base_url,
            constants::api_path(),
            entity_name
        );

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(&url)
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .send()
                .await
        }).await?;

        let status = response.status();
        if status.is_success() {
            let json: Value = response.json().await?;
            let attributes = json["value"].as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected 'value' array in response"))?;

            let mut raw_map = HashMap::new();
            for attr in attributes {
                if let Some(logical_name) = attr["LogicalName"].as_str() {
                    raw_map.insert(logical_name.to_string(), attr.clone());
                }
            }

            Ok(raw_map)
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Raw attribute metadata fetch failed with status {}: {}", status, error_text)
        }
    }

    /// Fetch entity fields by combining both metadata sources
    /// - XML metadata: NavigationProperties (relationships)
    /// - EntityDefinitions API: Attributes with proper lookup targets
    pub async fn fetch_entity_fields_combined(&self, entity_name: &str) -> anyhow::Result<Vec<super::metadata::FieldMetadata>> {
        use std::collections::HashMap;

        // Fetch from both sources in parallel
        let (xml_fields, api_fields) = tokio::try_join!(
            self.fetch_entity_fields(entity_name),
            self.fetch_entity_fields_alt(entity_name)
        )?;

        log::debug!("XML returned {} fields for {}", xml_fields.len(), entity_name);
        log::debug!("API returned {} fields for {}", api_fields.len(), entity_name);

        // Build lookup by lowercase logical_name from API fields (better metadata)
        // Use lowercase for case-insensitive deduplication (Dynamics sometimes returns different casing)
        let api_lookup: HashMap<String, super::metadata::FieldMetadata> = api_fields
            .into_iter()
            .map(|f| (f.logical_name.to_lowercase(), f))
            .collect();

        // Start with XML fields, but upgrade any that have better data from API
        // Use lowercase keys for case-insensitive deduplication
        let mut combined: HashMap<String, super::metadata::FieldMetadata> = HashMap::new();

        for xml_field in xml_fields {
            // If API has this field with better data, prefer it (case-insensitive lookup)
            let lookup_key = xml_field.logical_name.to_lowercase();
            if let Some(api_field) = api_lookup.get(&lookup_key) {
                // Check if both are NavigationProperties/relationships
                let xml_is_nav = matches!(&xml_field.field_type, super::metadata::FieldType::Other(t) if t.starts_with("Relationship:"))
                    || matches!(&xml_field.field_type, super::metadata::FieldType::Lookup);
                let api_is_lookup = matches!(&api_field.field_type, super::metadata::FieldType::Lookup);

                if xml_is_nav && api_is_lookup {
                    // Both represent the same relationship - prefer API version (has better metadata)
                    log::trace!("Deduplicating relationship {}: XML({:?}) + API({:?}) -> API",
                        xml_field.logical_name, xml_field.field_type, api_field.field_type);
                    combined.insert(lookup_key, api_field.clone());
                } else if api_field.related_entity.is_some() || api_field.display_name.is_some() {
                    // Prefer API version if it has related_entity (lookup fields) or display name
                    log::trace!("Upgrading field {}: XML({:?}) -> API({:?})",
                        xml_field.logical_name, xml_field.field_type, api_field.field_type);
                    combined.insert(lookup_key, api_field.clone());
                } else {
                    log::trace!("Keeping XML field {}: {:?}", xml_field.logical_name, xml_field.field_type);
                    combined.insert(lookup_key, xml_field);
                }
            } else {
                // Only in XML (NavigationProperty)
                log::trace!("XML-only field {}: {:?}", xml_field.logical_name, xml_field.field_type);
                combined.insert(lookup_key, xml_field);
            }
        }

        // Add any API-only fields that weren't in XML
        // Note: api_lookup keys are already lowercase
        for (lowercase_name, api_field) in api_lookup {
            combined.entry(lowercase_name).or_insert(api_field);
        }

        let mut result: Vec<_> = combined.into_values().collect();
        result.sort_by(|a, b| a.logical_name.cmp(&b.logical_name));

        log::debug!("Combined result has {} fields for {}", result.len(), entity_name);

        // Log relationship fields specifically
        let lookups: Vec<_> = result.iter()
            .filter(|f| matches!(&f.field_type, super::metadata::FieldType::Lookup))
            .map(|f| &f.logical_name)
            .collect();
        let nav_props: Vec<_> = result.iter()
            .filter(|f| matches!(&f.field_type, super::metadata::FieldType::Other(t) if t.starts_with("Relationship:")))
            .map(|f| &f.logical_name)
            .collect();
        log::debug!("Lookups: {}, NavigationProperties: {}", lookups.len(), nav_props.len());

        Ok(result)
    }

    /// Fetch entity metadata (EntitySetName, IsIntersect, PrimaryNameAttribute) from EntityDefinitions
    /// - EntitySetName: plural form used in OData URLs (e.g., "accounts", "nrq_fund_nrq_flemishshareset")
    /// - IsIntersect: true for junction/many-to-many relationship entities
    /// - PrimaryNameAttribute: field used for display name (e.g., "name", "fullname")
    pub async fn fetch_entity_metadata_info(&self, entity_name: &str) -> anyhow::Result<EntityMetadataInfo> {
        let url = format!(
            "{}/{}/EntityDefinitions(LogicalName='{}')?$select=EntitySetName,IsIntersect,PrimaryNameAttribute",
            self.base_url,
            constants::api_path(),
            entity_name
        );

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(&url)
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .send()
                .await
        }).await?;

        let status = response.status();
        if status.is_success() {
            let json: Value = response.json().await?;
            let entity_set_name = json["EntitySetName"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("EntitySetName not found in response"))?
                .to_string();
            let is_intersect = json["IsIntersect"]
                .as_bool()
                .unwrap_or(false);
            let primary_name_attribute = json["PrimaryNameAttribute"]
                .as_str()
                .map(|s| s.to_string());
            Ok(EntityMetadataInfo {
                entity_set_name,
                is_intersect,
                primary_name_attribute,
            })
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Entity metadata fetch failed with status {}: {}", status, error_text)
        }
    }

    /// Fetch incoming references (entities that have lookups pointing TO this entity)
    /// Uses OneToManyRelationships metadata
    pub async fn fetch_incoming_references(&self, entity_name: &str) -> anyhow::Result<Vec<IncomingReference>> {
        let url = format!(
            "{}/{}/EntityDefinitions(LogicalName='{}')/OneToManyRelationships?$select=ReferencingEntity,ReferencingAttribute",
            self.base_url,
            constants::api_path(),
            entity_name
        );

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(&url)
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .send()
                .await
        }).await?;

        let status = response.status();
        if status.is_success() {
            let json: Value = response.json().await?;
            let relationships = json["value"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected 'value' array in response"))?;

            let refs: Vec<IncomingReference> = relationships
                .iter()
                .filter_map(|rel| {
                    let referencing_entity = rel["ReferencingEntity"].as_str()?;
                    let referencing_attribute = rel["ReferencingAttribute"].as_str()?;
                    Some(IncomingReference {
                        referencing_entity: referencing_entity.to_string(),
                        referencing_attribute: referencing_attribute.to_string(),
                    })
                })
                .collect();

            Ok(refs)
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Incoming references fetch failed with status {}: {}", status, error_text)
        }
    }

    /// Fetch ManyToMany relationships for an entity
    /// Used to get navigation property names for AssociateRef operations on junction entities
    pub async fn fetch_many_to_many_relationships(&self, entity_name: &str) -> anyhow::Result<Vec<ManyToManyRelationship>> {
        let url = format!(
            "{}/{}/EntityDefinitions(LogicalName='{}')/ManyToManyRelationships",
            self.base_url,
            constants::api_path(),
            entity_name
        );

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(&url)
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .send()
                .await
        }).await?;

        let status = response.status();
        if status.is_success() {
            let json: Value = response.json().await?;
            let relationships = json["value"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected 'value' array in response"))?;

            let rels: Vec<ManyToManyRelationship> = relationships
                .iter()
                .filter_map(|rel| {
                    let intersect_entity_name = rel["IntersectEntityName"].as_str()?;
                    let entity1_logical_name = rel["Entity1LogicalName"].as_str()?;
                    let entity2_logical_name = rel["Entity2LogicalName"].as_str()?;
                    let entity1_intersect_attribute = rel["Entity1IntersectAttribute"].as_str()?;
                    let entity2_intersect_attribute = rel["Entity2IntersectAttribute"].as_str()?;
                    let entity1_navigation_property = rel["Entity1NavigationPropertyName"].as_str()?;
                    let entity2_navigation_property = rel["Entity2NavigationPropertyName"].as_str()?;

                    Some(ManyToManyRelationship {
                        intersect_entity_name: intersect_entity_name.to_string(),
                        entity1_logical_name: entity1_logical_name.to_string(),
                        entity2_logical_name: entity2_logical_name.to_string(),
                        entity1_intersect_attribute: entity1_intersect_attribute.to_string(),
                        entity2_intersect_attribute: entity2_intersect_attribute.to_string(),
                        entity1_navigation_property: entity1_navigation_property.to_string(),
                        entity2_navigation_property: entity2_navigation_property.to_string(),
                    })
                })
                .collect();

            Ok(rels)
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("ManyToMany relationships fetch failed with status {}: {}", status, error_text)
        }
    }

    /// Fetch entity forms from systemforms endpoint
    pub async fn fetch_entity_forms(&self, entity_name: &str) -> anyhow::Result<Vec<super::metadata::FormMetadata>> {
        let url = format!(
            "{}/{}/systemforms?$filter=objecttypecode eq '{}'&$select=formid,name,type,formxml",
            self.base_url,
            constants::api_path(),
            entity_name
        );

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(&url)
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .send()
                .await
        }).await?;

        let status = response.status();
        if status.is_success() {
            let json: Value = response.json().await?;
            let forms_array = json["value"].as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected 'value' array in response"))?;

            let forms = forms_array.iter()
                .filter_map(|form| {
                    let id = form["formid"].as_str()?.to_string();
                    let name = form["name"].as_str()?.to_string();
                    let form_type = form["type"].as_i64()
                        .map(|t| match t {
                            2 => "Main".to_string(),
                            5 => "Mobile".to_string(),
                            6 => "Quick View".to_string(),
                            7 => "Quick Create".to_string(),
                            8 => "Dialog".to_string(),
                            9 => "Task Flow".to_string(),
                            11 => "Card".to_string(),
                            12 => "Main - Interactive experience".to_string(),
                            _ => format!("Type {}", t),
                        })
                        .unwrap_or_else(|| "Unknown".to_string());

                    // Parse formxml if present
                    let form_structure = if let Some(formxml_str) = form["formxml"].as_str() {
                        Self::parse_form_xml(formxml_str, entity_name).ok()
                    } else {
                        None
                    };

                    Some(super::metadata::FormMetadata {
                        id,
                        name,
                        form_type,
                        form_structure,
                    })
                })
                .collect();

            Ok(forms)
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Form metadata fetch failed with status {}: {}", status, error_text)
        }
    }

    /// Fetch entity views from savedqueries endpoint
    pub async fn fetch_entity_views(&self, entity_name: &str) -> anyhow::Result<Vec<super::metadata::ViewMetadata>> {
        let url = format!(
            "{}/{}/savedqueries?$filter=returnedtypecode eq '{}'&$select=savedqueryid,name,querytype,layoutxml",
            self.base_url,
            constants::api_path(),
            entity_name
        );

        // Apply rate limiting before making the request
        let _permit = self.apply_rate_limiting().await?;

        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(&url)
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .send()
                .await
        }).await?;

        let status = response.status();
        if status.is_success() {
            let json: Value = response.json().await?;
            let views_array = json["value"].as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected 'value' array in response"))?;

            let views = views_array.iter()
                .filter_map(|view| {
                    let id = view["savedqueryid"].as_str()?.to_string();
                    let name = view["name"].as_str()?.to_string();
                    let view_type = view["querytype"].as_i64()
                        .map(|t| match t {
                            0 => "Public".to_string(),
                            1 => "Advanced Find".to_string(),
                            2 => "Associated".to_string(),
                            4 => "Quick Find".to_string(),
                            8 => "Lookup".to_string(),
                            16 => "Main Application".to_string(),
                            64 => "Offline".to_string(),
                            128 => "Outlook".to_string(),
                            256 => "Wizard".to_string(),
                            _ => format!("Type {}", t),
                        })
                        .unwrap_or_else(|| "Unknown".to_string());

                    // Parse layoutxml to extract columns with metadata
                    let columns = if let Some(layout_xml) = view["layoutxml"].as_str() {
                        Self::parse_view_layout_xml(layout_xml)
                    } else {
                        Vec::new()
                    };

                    Some(super::metadata::ViewMetadata {
                        id,
                        name,
                        view_type,
                        columns,
                    })
                })
                .collect();

            Ok(views)
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("View metadata fetch failed with status {}: {}", status, error_text)
        }
    }

    /// Parse form XML into structured hierarchy
    fn parse_form_xml(formxml: &str, entity_name: &str) -> anyhow::Result<super::metadata::FormStructure> {
        use roxmltree::Document;

        let doc = Document::parse(formxml)
            .map_err(|e| anyhow::anyhow!("Failed to parse form XML: {}", e))?;

        let mut tabs = Vec::new();

        // Find all tab elements
        for tab_node in doc.descendants().filter(|n| n.has_tag_name("tab")) {
            let tab_name = tab_node.attribute("name").unwrap_or("").to_string();
            let tab_label = tab_node.descendants()
                .find(|n| n.has_tag_name("label"))
                .and_then(|n| n.attribute("description"))
                .unwrap_or(&tab_name)
                .to_string();
            let visible = tab_node.attribute("visible").map(|v| v == "true").unwrap_or(true);
            let expanded = tab_node.attribute("expanded").map(|v| v == "true").unwrap_or(true);
            let order = tab_node.attribute("verticallayout")
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(0);

            let mut sections = Vec::new();

            // Find all sections within this tab
            for section_node in tab_node.descendants().filter(|n| n.has_tag_name("section")) {
                let section_name = section_node.attribute("name").unwrap_or("").to_string();
                let section_label = section_node.descendants()
                    .find(|n| n.has_tag_name("label"))
                    .and_then(|n| n.attribute("description"))
                    .unwrap_or(&section_name)
                    .to_string();
                let section_visible = section_node.attribute("visible").map(|v| v == "true").unwrap_or(true);
                let section_columns = section_node.attribute("columns")
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(1);

                let mut fields = Vec::new();

                // Find all fields (control elements with datafieldname) within this section
                for (idx, control_node) in section_node.descendants()
                    .filter(|n| n.has_tag_name("control") && n.attribute("datafieldname").is_some())
                    .enumerate()
                {
                    let logical_name = control_node.attribute("datafieldname").unwrap_or("").to_string();
                    let field_label = control_node.descendants()
                        .find(|n| n.has_tag_name("label"))
                        .and_then(|n| n.attribute("description"))
                        .unwrap_or(&logical_name)
                        .to_string();
                    let field_visible = control_node.attribute("visible").map(|v| v == "true").unwrap_or(true);
                    let required_level = control_node.attribute("classid")
                        .and_then(|_| control_node.attribute("requirementlevel"))
                        .unwrap_or("None")
                        .to_string();
                    let readonly = control_node.attribute("disabled").map(|v| v == "true").unwrap_or(false);

                    fields.push(super::metadata::FormField {
                        logical_name,
                        label: field_label,
                        visible: field_visible,
                        required_level,
                        readonly,
                        row: idx as i32,  // Approximation
                        column: 0,  // Would need more complex layout parsing
                    });
                }

                sections.push(super::metadata::FormSection {
                    name: section_name,
                    label: section_label,
                    visible: section_visible,
                    columns: section_columns,
                    order: 0,  // Would need to extract from XML
                    fields,
                });
            }

            tabs.push(super::metadata::FormTab {
                name: tab_name,
                label: tab_label,
                visible,
                expanded,
                order,
                sections,
            });
        }

        Ok(super::metadata::FormStructure {
            name: "Form Structure".to_string(),
            entity_name: entity_name.to_string(),
            tabs,
        })
    }

    /// Parse view layout XML to extract column metadata
    fn parse_view_layout_xml(layout_xml: &str) -> Vec<super::metadata::ViewColumn> {
        use roxmltree::Document;

        let doc = match Document::parse(layout_xml) {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };

        let mut columns = Vec::new();

        // Find all cell elements (columns in the view)
        for (idx, cell_node) in doc.descendants()
            .filter(|n| n.has_tag_name("cell"))
            .enumerate()
        {
            if let Some(name) = cell_node.attribute("name") {
                let width = cell_node.attribute("width")
                    .and_then(|w| w.parse::<u32>().ok());
                let is_primary = cell_node.attribute("isprimary")
                    .map(|v| v == "true" || v == "1")
                    .unwrap_or(idx == 0);  // First column is typically primary

                columns.push(super::metadata::ViewColumn {
                    name: name.to_string(),
                    width,
                    is_primary,
                });
            }
        }

        columns
    }

    /// Fetch a single record by ID
    /// Returns the full record as JSON with all fields and formatted values
    pub async fn fetch_record_by_id(
        &self,
        entity_name: &str,
        record_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let _permit = self.apply_rate_limiting().await?;

        // Pluralize entity name for the endpoint
        let plural_entity = super::pluralization::pluralize_entity_name(entity_name);

        // Build URL with $select=* to get all fields
        // Also add Prefer header to include formatted values and lookup properties
        let url = format!("{}{}/{}({})?$select=*",
            self.base_url,
            constants::api_path(),
            plural_entity,
            record_id
        );

        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(&url)
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header("Prefer", "odata.include-annotations=\"OData.Community.Display.V1.FormattedValue\"")
                .send()
                .await
        }).await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to fetch record: {} - {}", response.status(), response.text().await?);
        }

        let record: serde_json::Value = response.json().await?;

        log::debug!("Fetched record for entity '{}' with ID '{}'", entity_name, record_id);
        log::debug!("Record has {} top-level fields", record.as_object().map(|o| o.len()).unwrap_or(0));
        if let Some(obj) = record.as_object() {
            log::debug!("Record field names: {:?}", obj.keys().collect::<Vec<_>>());
        }

        Ok(record)
    }
}