use super::constants::{self, headers, methods};
use super::operations::{Operation, OperationResult, BatchRequestBuilder, BatchResponseParser};
use super::query::{Query, QueryResult, QueryResponse};
use super::resilience::{RetryPolicy, RetryConfig, ResilienceConfig, RateLimiter, ApiLogger, OperationContext, OperationMetrics, MetricsCollector};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// Modern Dynamics 365 Web API client with connection pooling
#[derive(Clone)]
pub struct DynamicsClient {
    base_url: String,
    http_client: reqwest::Client,
    access_token: String,
    retry_policy: RetryPolicy, // Default retry policy for backwards compatibility
    rate_limiter: RateLimiter, // Global rate limiter for this client instance
    api_logger: ApiLogger, // Structured logger for operations
    metrics_collector: MetricsCollector, // Performance metrics collector
}

impl DynamicsClient {
    /// Apply rate limiting using the client's global rate limiter
    async fn apply_rate_limiting(&self) -> anyhow::Result<()> {
        self.rate_limiter.acquire().await;
        Ok(())
    }

    /// Get rate limiter statistics for monitoring
    pub fn rate_limiter_stats(&self) -> crate::api::resilience::RateLimiterStats {
        self.rate_limiter.stats()
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

        Self {
            base_url,
            http_client,
            access_token,
            retry_policy: RetryPolicy::default(),
            rate_limiter: RateLimiter::new(ResilienceConfig::default().rate_limit),
            api_logger: ApiLogger::new(ResilienceConfig::default().monitoring),
            metrics_collector: MetricsCollector::new(ResilienceConfig::default().monitoring),
        }
    }

    /// Create a new client with custom HTTP client configuration
    pub fn with_custom_client(base_url: String, access_token: String, http_client: reqwest::Client) -> Self {
        Self {
            base_url,
            http_client,
            access_token,
            retry_policy: RetryPolicy::default(),
            rate_limiter: RateLimiter::new(ResilienceConfig::default().rate_limit),
            api_logger: ApiLogger::new(ResilienceConfig::default().monitoring),
            metrics_collector: MetricsCollector::new(ResilienceConfig::default().monitoring),
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

        Self {
            base_url,
            http_client,
            access_token,
            retry_policy: RetryPolicy::new(retry_config),
            rate_limiter: RateLimiter::new(ResilienceConfig::default().rate_limit),
            api_logger: ApiLogger::new(ResilienceConfig::default().monitoring),
            metrics_collector: MetricsCollector::new(ResilienceConfig::default().monitoring),
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
        let params = query.to_query_params();

        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(&url)
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .query(&params)
                .send()
                .await
        }).await?;

        self.parse_query_response(response).await
    }

    /// Execute FetchXML query directly (for FQL compatibility)
    pub async fn execute_fetchxml(&self, entity_name: &str, fetchxml: &str) -> anyhow::Result<Value> {
        self.apply_rate_limiting().await?;

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
        self.apply_rate_limiting().await?;

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
    pub async fn execute_next_page(&self, next_link: &str) -> anyhow::Result<QueryResult> {
        let response = self.retry_policy.execute(|| async {
            self.http_client
                .get(next_link)
                .bearer_auth(&self.access_token)
                .header("Accept", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .send()
                .await
        }).await?;

        self.parse_query_response(response).await
    }

    /// Create a new record
    async fn create_record(&self, entity: &str, data: &Value, resilience: &ResilienceConfig) -> anyhow::Result<OperationResult> {
        let url = constants::entity_endpoint(&self.base_url, entity);
        let correlation_id = uuid::Uuid::new_v4().to_string();

        // Start operation tracking
        let logger = ApiLogger::new(resilience.monitoring.clone());
        let mut context = logger.start_operation("create", entity, &correlation_id);

        // Apply rate limiting before making the request
        self.apply_rate_limiting().await?;

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
        let response = retry_policy.execute(|| async {
            self.http_client
                .post(&url)
                .bearer_auth(&self.access_token)
                .header("Content-Type", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header("Prefer", headers::PREFER_RETURN_REPRESENTATION)
                .header(headers::X_CORRELATION_ID, &correlation_id)
                .json(data)
                .send()
                .await
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
        self.apply_rate_limiting().await?;

        let retry_policy = crate::api::resilience::RetryPolicy::new(resilience.retry.clone());
        let response = retry_policy.execute(|| async {
            self.http_client
                .patch(&url)
                .bearer_auth(&self.access_token)
                .header("Content-Type", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header("If-Match", headers::IF_MATCH_ANY)
                .header("Prefer", headers::PREFER_RETURN_REPRESENTATION)
                .header(headers::X_CORRELATION_ID, &correlation_id)
                .json(data)
                .send()
                .await
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
        self.apply_rate_limiting().await?;

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
        self.apply_rate_limiting().await?;

        let retry_policy = crate::api::resilience::RetryPolicy::new(resilience.retry.clone());
        let response = retry_policy.execute(|| async {
            self.http_client
                .patch(&url)
                .bearer_auth(&self.access_token)
                .header("Content-Type", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header("Prefer", headers::PREFER_RETURN_REPRESENTATION)
                .header(headers::X_CORRELATION_ID, &correlation_id)
                .json(data)
                .send()
                .await
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
        self.apply_rate_limiting().await?;

        let body = serde_json::json!({
            "@odata.id": target_ref
        });

        let retry_policy = crate::api::resilience::RetryPolicy::new(resilience.retry.clone());
        let response = retry_policy.execute(|| async {
            self.http_client
                .post(&url)
                .bearer_auth(&self.access_token)
                .header("Content-Type", headers::CONTENT_TYPE_JSON)
                .header("OData-Version", headers::ODATA_VERSION)
                .header(headers::X_CORRELATION_ID, &correlation_id)
                .json(&body)
                .send()
                .await
        }).await?;

        self.parse_response(Operation::AssociateRef {
            entity: entity.to_string(),
            entity_ref: entity_ref.to_string(),
            navigation_property: navigation_property.to_string(),
            target_ref: target_ref.to_string(),
        }, response).await
    }

    /// Execute operations using the $batch endpoint
    async fn execute_batch_request(&self, operations: &[Operation], resilience: &ResilienceConfig) -> anyhow::Result<Vec<OperationResult>> {
        let url = constants::batch_endpoint(&self.base_url);
        let correlation_id = uuid::Uuid::new_v4().to_string();

        // Apply rate limiting before making the request
        self.apply_rate_limiting().await?;

        // Build the batch request using the proper builder
        let batch_request = BatchRequestBuilder::new(&self.base_url)
            .add_changeset(operations)
            .build();

        let content_type = batch_request.content_type().to_string();
        let body = batch_request.body().to_string();

        log::debug!("Executing batch request with {} operations (correlation_id: {})", operations.len(), correlation_id);

        // Log first 2000 chars of request body for debugging
        let truncated_body = if body.len() > 2000 {
            format!("{}... (truncated, total {} chars)", &body[..2000], body.len())
        } else {
            body.clone()
        };
        log::debug!("Batch request body:\n{}", truncated_body);

        let retry_policy = crate::api::resilience::RetryPolicy::new(resilience.retry.clone());
        let request_start = std::time::Instant::now();
        let response = retry_policy.execute(|| async {
            self.http_client
                .post(&url)
                .bearer_auth(&self.access_token)
                .header("Content-Type", content_type.clone())
                .header("OData-Version", headers::ODATA_VERSION)
                .header(headers::X_CORRELATION_ID, &correlation_id)
                .body(body.clone())
                .send()
                .await
        }).await?;

        let request_duration = request_start.elapsed();
        let status_code = response.status().as_u16();

        log::debug!("Batch request completed: status={}, duration={:?}", status_code, request_duration);

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
        self.apply_rate_limiting().await?;

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
                    display_name: None,
                    field_type,
                    is_required,
                    is_primary_key: false, // TODO: detect from Key element
                    max_length: None,
                    related_entity: None,
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
                    display_name: None,
                    field_type,
                    is_required: false,
                    is_primary_key: false,
                    max_length: None,
                    related_entity,
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

    /// Fetch entity field definitions from EntityDefinitions endpoint (attributes only, no navigation properties)
    pub async fn fetch_entity_fields_alt(&self, entity_name: &str) -> anyhow::Result<Vec<super::metadata::FieldMetadata>> {
        let url = format!(
            "{}/{}/EntityDefinitions(LogicalName='{}')/Attributes",
            self.base_url,
            constants::api_path(),
            entity_name
        );

        // Apply rate limiting before making the request
        self.apply_rate_limiting().await?;

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

                    Some(super::metadata::FieldMetadata {
                        logical_name,
                        display_name,
                        field_type,
                        is_required,
                        is_primary_key,
                        max_length,
                        related_entity,
                    })
                })
                .collect();

            Ok(fields)
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Field metadata fetch failed with status {}: {}", status, error_text)
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

        // Build lookup by logical_name from API fields (better metadata)
        let api_lookup: HashMap<String, super::metadata::FieldMetadata> = api_fields
            .into_iter()
            .map(|f| (f.logical_name.clone(), f))
            .collect();

        // Start with XML fields, but upgrade any that have better data from API
        let mut combined: HashMap<String, super::metadata::FieldMetadata> = HashMap::new();

        for xml_field in xml_fields {
            // If API has this field with better data, prefer it
            if let Some(api_field) = api_lookup.get(&xml_field.logical_name) {
                // Check if both are NavigationProperties/relationships
                let xml_is_nav = matches!(&xml_field.field_type, super::metadata::FieldType::Other(t) if t.starts_with("Relationship:"))
                    || matches!(&xml_field.field_type, super::metadata::FieldType::Lookup);
                let api_is_lookup = matches!(&api_field.field_type, super::metadata::FieldType::Lookup);

                if xml_is_nav && api_is_lookup {
                    // Both represent the same relationship - prefer API version (has better metadata)
                    log::trace!("Deduplicating relationship {}: XML({:?}) + API({:?}) -> API",
                        xml_field.logical_name, xml_field.field_type, api_field.field_type);
                    combined.insert(xml_field.logical_name.clone(), api_field.clone());
                } else if api_field.related_entity.is_some() || api_field.display_name.is_some() {
                    // Prefer API version if it has related_entity (lookup fields) or display name
                    log::trace!("Upgrading field {}: XML({:?}) -> API({:?})",
                        xml_field.logical_name, xml_field.field_type, api_field.field_type);
                    combined.insert(xml_field.logical_name.clone(), api_field.clone());
                } else {
                    log::trace!("Keeping XML field {}: {:?}", xml_field.logical_name, xml_field.field_type);
                    combined.insert(xml_field.logical_name.clone(), xml_field);
                }
            } else {
                // Only in XML (NavigationProperty)
                log::trace!("XML-only field {}: {:?}", xml_field.logical_name, xml_field.field_type);
                combined.insert(xml_field.logical_name.clone(), xml_field);
            }
        }

        // Add any API-only fields that weren't in XML
        for (name, api_field) in api_lookup {
            combined.entry(name).or_insert(api_field);
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

    /// Fetch entity forms from systemforms endpoint
    pub async fn fetch_entity_forms(&self, entity_name: &str) -> anyhow::Result<Vec<super::metadata::FormMetadata>> {
        let url = format!(
            "{}/{}/systemforms?$filter=objecttypecode eq '{}'&$select=formid,name,type,formxml",
            self.base_url,
            constants::api_path(),
            entity_name
        );

        // Apply rate limiting before making the request
        self.apply_rate_limiting().await?;

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
        self.apply_rate_limiting().await?;

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
        self.apply_rate_limiting().await?;

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