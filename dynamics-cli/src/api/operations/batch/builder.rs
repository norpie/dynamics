//! Dynamics 365 $batch request builder
//!
//! Builds proper multipart/mixed format for Dynamics 365 Web API batch operations
//! following OData v4.0 specifications.

use crate::api::constants::{self, headers, methods};
use crate::api::operations::Operation;
use serde_json::Value;
use uuid::Uuid;

const CRLF: &str = "\r\n";

/// Builder for creating Dynamics 365 $batch requests
pub struct BatchRequestBuilder {
    batch_id: String,
    changeset_id: String,
    base_url: String,
    requests: Vec<BatchItem>,
    /// Bypass headers to apply to each operation in the batch
    bypass_headers: Vec<(String, String)>,
}

/// Individual item in a batch request
#[derive(Debug, Clone)]
pub enum BatchItem {
    /// A direct batch request (like GET operations)
    DirectRequest {
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: Option<String>,
    },
    /// Operations grouped in a changeset (transactional)
    ChangeSet {
        operations: Vec<ChangeSetOperation>,
    },
}

/// Operation within a changeset
#[derive(Debug, Clone)]
pub struct ChangeSetOperation {
    pub content_id: u32,
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

impl BatchRequestBuilder {
    /// Create a new batch request builder
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            batch_id: format!("batch_{}", Uuid::new_v4().to_string().replace('-', "_")),
            changeset_id: format!("changeset_{}", Uuid::new_v4().to_string().replace('-', "_")),
            base_url: base_url.into(),
            requests: Vec::new(),
            bypass_headers: Vec::new(),
        }
    }

    /// Set bypass headers to apply to each operation in the batch
    ///
    /// Dynamics 365 requires bypass headers on each individual request within
    /// a batch, not just on the outer HTTP request.
    pub fn with_bypass_headers(mut self, headers: Vec<(&str, String)>) -> Self {
        self.bypass_headers = headers.into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        self
    }

    /// Add operations as a changeset (transactional)
    pub fn add_changeset(mut self, operations: &[Operation]) -> Self {
        if operations.is_empty() {
            return self;
        }

        let changeset_operations: Vec<ChangeSetOperation> = operations
            .iter()
            .enumerate()
            .map(|(index, operation)| self.operation_to_changeset_operation(operation, (index + 1) as u32))
            .collect();

        self.requests.push(BatchItem::ChangeSet {
            operations: changeset_operations,
        });

        self
    }

    /// Add a single operation as a changeset
    pub fn add_operation(self, operation: &Operation) -> Self {
        self.add_changeset(&[operation.clone()])
    }

    /// Build operation headers, including bypass headers if configured
    fn build_op_headers(&self, base_headers: Vec<(String, String)>) -> Vec<(String, String)> {
        let mut headers = base_headers;
        // Add bypass headers to each operation
        for (name, value) in &self.bypass_headers {
            headers.push((name.clone(), value.clone()));
        }
        headers
    }

    /// Convert an Operation to a ChangeSetOperation
    fn operation_to_changeset_operation(&self, operation: &Operation, content_id: u32) -> ChangeSetOperation {
        match operation {
            Operation::Create { entity, data } => {
                let path = format!("{}/{}", constants::api_path(), entity);
                let body = serde_json::to_string(data).unwrap_or_default();

                ChangeSetOperation {
                    content_id,
                    method: methods::POST.to_string(),
                    path,
                    headers: self.build_op_headers(vec![
                        ("Content-Type".to_string(), headers::CONTENT_TYPE_JSON.to_string()),
                        ("Prefer".to_string(), headers::PREFER_RETURN_REPRESENTATION.to_string()),
                    ]),
                    body: Some(body),
                }
            }
            Operation::CreateWithRefs { entity, data, content_id_refs } => {
                let path = format!("{}/{}", constants::api_path(), entity);

                // Merge data with content-ID references
                let mut payload = data.clone();
                if let Value::Object(ref mut map) = payload {
                    for (field, ref_value) in content_id_refs {
                        map.insert(field.clone(), Value::String(ref_value.clone()));
                    }
                }

                let body = serde_json::to_string(&payload).unwrap_or_default();

                ChangeSetOperation {
                    content_id,
                    method: methods::POST.to_string(),
                    path,
                    headers: self.build_op_headers(vec![
                        ("Content-Type".to_string(), headers::CONTENT_TYPE_JSON.to_string()),
                        ("Prefer".to_string(), headers::PREFER_RETURN_REPRESENTATION.to_string()),
                    ]),
                    body: Some(body),
                }
            }
            Operation::Update { entity, id, data } => {
                let path = format!("{}/{}({})", constants::api_path(), entity, id);
                let body = serde_json::to_string(data).unwrap_or_default();

                ChangeSetOperation {
                    content_id,
                    method: methods::PATCH.to_string(),
                    path,
                    headers: self.build_op_headers(vec![
                        ("Content-Type".to_string(), headers::CONTENT_TYPE_JSON.to_string()),
                        ("If-Match".to_string(), headers::IF_MATCH_ANY.to_string()),
                        ("Prefer".to_string(), headers::PREFER_RETURN_REPRESENTATION.to_string()),
                    ]),
                    body: Some(body),
                }
            }
            Operation::Delete { entity, id } => {
                let path = format!("{}/{}({})", constants::api_path(), entity, id);

                ChangeSetOperation {
                    content_id,
                    method: methods::DELETE.to_string(),
                    path,
                    headers: self.build_op_headers(vec![]),
                    body: None,
                }
            }
            Operation::Upsert { entity, key_field, key_value, data } => {
                let path = format!("{}/{}({}='{}')", constants::api_path(), entity, key_field, key_value);
                let body = serde_json::to_string(data).unwrap_or_default();

                ChangeSetOperation {
                    content_id,
                    method: methods::PATCH.to_string(),
                    path,
                    headers: self.build_op_headers(vec![
                        ("Content-Type".to_string(), headers::CONTENT_TYPE_JSON.to_string()),
                        ("Prefer".to_string(), headers::PREFER_RETURN_REPRESENTATION.to_string()),
                    ]),
                    body: Some(body),
                }
            }
            Operation::AssociateRef { entity, entity_ref, navigation_property, target_ref } => {
                // POST /entities(id)/navigation_property/$ref with body {"@odata.id": "target"}
                let path = format!("{}/{}({})/{}/$ref",
                    constants::api_path(),
                    entity,
                    entity_ref,
                    navigation_property
                );

                // Convert relative target_ref to absolute URL for @odata.id
                let absolute_target_ref = if target_ref.starts_with("http://") || target_ref.starts_with("https://") {
                    target_ref.clone()
                } else {
                    // target_ref is like "/nrq_categories(guid)", need to prepend base_url + api path
                    let base = self.base_url.trim_end_matches('/');
                    let api_path = constants::api_path();
                    let api_path = api_path.trim_start_matches('/');
                    let path = target_ref.trim_start_matches('/');
                    format!("{}/{}/{}", base, api_path, path)
                };

                let body_json = serde_json::json!({
                    "@odata.id": absolute_target_ref
                });
                let body = serde_json::to_string(&body_json).unwrap_or_default();

                ChangeSetOperation {
                    content_id,
                    method: methods::POST.to_string(),
                    path,
                    headers: self.build_op_headers(vec![
                        ("Content-Type".to_string(), headers::CONTENT_TYPE_JSON.to_string()),
                    ]),
                    body: Some(body),
                }
            }
            Operation::DisassociateRef { entity, entity_ref, navigation_property, target_id } => {
                // DELETE /entities(id)/navigation_property(target_id)/$ref
                let path = format!("{}/{}({})/{}({})/$ref",
                    constants::api_path(),
                    entity,
                    entity_ref,
                    navigation_property,
                    target_id
                );

                ChangeSetOperation {
                    content_id,
                    method: methods::DELETE.to_string(),
                    path,
                    headers: self.build_op_headers(vec![]),
                    body: None,
                }
            }
            // Schema operations - these typically shouldn't be in batch changesets,
            // but we handle them for completeness
            Operation::CreateAttribute { entity, attribute_data, solution_name } => {
                let path = format!("{}/EntityDefinitions(LogicalName='{}')/Attributes", constants::api_path(), entity);
                let body = serde_json::to_string(attribute_data).unwrap_or_default();

                let mut op_headers = vec![
                    ("Content-Type".to_string(), headers::CONTENT_TYPE_JSON.to_string()),
                ];
                if let Some(solution) = solution_name {
                    op_headers.push(("MSCRM.SolutionUniqueName".to_string(), solution.clone()));
                }

                ChangeSetOperation {
                    content_id,
                    method: methods::POST.to_string(),
                    path,
                    headers: self.build_op_headers(op_headers),
                    body: Some(body),
                }
            }
            Operation::UpdateAttribute { entity, attribute, attribute_data } => {
                let path = format!("{}/EntityDefinitions(LogicalName='{}')/Attributes(LogicalName='{}')",
                    constants::api_path(), entity, attribute);
                let body = serde_json::to_string(attribute_data).unwrap_or_default();

                ChangeSetOperation {
                    content_id,
                    method: "PUT".to_string(), // Schema updates use PUT
                    path,
                    headers: self.build_op_headers(vec![
                        ("Content-Type".to_string(), headers::CONTENT_TYPE_JSON.to_string()),
                    ]),
                    body: Some(body),
                }
            }
            Operation::DeleteAttribute { entity, attribute } => {
                let path = format!("{}/EntityDefinitions(LogicalName='{}')/Attributes(LogicalName='{}')",
                    constants::api_path(), entity, attribute);

                ChangeSetOperation {
                    content_id,
                    method: methods::DELETE.to_string(),
                    path,
                    headers: self.build_op_headers(vec![]),
                    body: None,
                }
            }
            Operation::PublishAllXml => {
                let path = format!("{}/PublishAllXml", constants::api_path());

                ChangeSetOperation {
                    content_id,
                    method: methods::POST.to_string(),
                    path,
                    headers: self.build_op_headers(vec![
                        ("Content-Type".to_string(), headers::CONTENT_TYPE_JSON.to_string()),
                    ]),
                    body: None,
                }
            }
        }
    }

    /// Build the complete batch request body
    pub fn build(self) -> BatchRequest {
        let mut body = String::new();

        for request in &self.requests {
            match request {
                BatchItem::DirectRequest { method, path, headers, body: req_body } => {
                    // Add direct request to batch
                    body.push_str(&format!("--{}{}", self.batch_id, CRLF));
                    body.push_str(&format!("Content-Type: application/http{}", CRLF));
                    body.push_str(&format!("Content-Transfer-Encoding: binary{}", CRLF));
                    body.push_str(CRLF);

                    // HTTP request line
                    body.push_str(&format!("{} {} HTTP/1.1{}", method, path, CRLF));

                    // Headers
                    for (name, value) in headers {
                        body.push_str(&format!("{}: {}{}", name, value, CRLF));
                    }

                    body.push_str(CRLF);

                    // Body (if present)
                    if let Some(req_body) = req_body {
                        body.push_str(req_body);
                    }

                    body.push_str(CRLF);
                }
                BatchItem::ChangeSet { operations } => {
                    // Start changeset
                    body.push_str(&format!("--{}{}", self.batch_id, CRLF));
                    body.push_str(&format!("Content-Type: multipart/mixed; boundary=\"{}\"{}", self.changeset_id, CRLF));
                    body.push_str(CRLF);

                    // Add each operation in changeset
                    for operation in operations {
                        body.push_str(&format!("--{}{}", self.changeset_id, CRLF));
                        body.push_str(&format!("Content-Type: application/http{}", CRLF));
                        body.push_str(&format!("Content-Transfer-Encoding: binary{}", CRLF));
                        body.push_str(&format!("Content-ID: {}{}", operation.content_id, CRLF));
                        body.push_str(CRLF);

                        // HTTP request line
                        body.push_str(&format!("{} {} HTTP/1.1{}", operation.method, operation.path, CRLF));

                        // Headers
                        for (name, value) in &operation.headers {
                            body.push_str(&format!("{}: {}{}", name, value, CRLF));
                        }

                        body.push_str(CRLF);

                        // Body (if present)
                        if let Some(op_body) = &operation.body {
                            body.push_str(op_body);
                        }

                        body.push_str(CRLF);
                    }

                    // End changeset
                    body.push_str(&format!("--{}--{}", self.changeset_id, CRLF));
                    body.push_str(CRLF);
                }
            }
        }

        // End batch
        body.push_str(&format!("--{}--{}", self.batch_id, CRLF));

        BatchRequest {
            content_type: format!("multipart/mixed; boundary=\"{}\"", self.batch_id),
            body,
        }
    }

    /// Get the batch boundary ID (for testing)
    pub fn batch_id(&self) -> &str {
        &self.batch_id
    }

    /// Get the changeset boundary ID (for testing)
    pub fn changeset_id(&self) -> &str {
        &self.changeset_id
    }
}

/// Complete batch request ready to send
#[derive(Debug, Clone)]
pub struct BatchRequest {
    pub content_type: String,
    pub body: String,
}

impl BatchRequest {
    /// Get the Content-Type header value
    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    /// Get the request body
    pub fn body(&self) -> &str {
        &self.body
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_single_create_operation() {
        let operation = Operation::create("contacts", json!({
            "firstname": "John",
            "lastname": "Doe"
        }));

        let batch = BatchRequestBuilder::new("https://test.crm.dynamics.com")
            .add_operation(&operation)
            .build();

        assert!(batch.content_type.starts_with("multipart/mixed; boundary=\"batch_"));
        assert!(batch.body.contains("POST /api/data/v9.2/contacts HTTP/1.1"));
        assert!(batch.body.contains("Content-Type: application/json"));
        assert!(batch.body.contains("\"firstname\":\"John\""));
        assert!(batch.body.contains("Content-ID: 1"));
    }

    #[test]
    fn test_multiple_operations_changeset() {
        let operations = vec![
            Operation::create("contacts", json!({"firstname": "John"})),
            Operation::update("contacts", "123-456", json!({"lastname": "Updated"})),
            Operation::delete("contacts", "789-012"),
        ];

        let batch = BatchRequestBuilder::new("https://test.crm.dynamics.com")
            .add_changeset(&operations)
            .build();

        // Check structure
        assert!(batch.body.contains("Content-Type: multipart/mixed; boundary=\"changeset_"));
        assert!(batch.body.contains("POST /api/data/v9.2/contacts HTTP/1.1"));
        assert!(batch.body.contains("PATCH /api/data/v9.2/contacts(123-456) HTTP/1.1"));
        assert!(batch.body.contains("DELETE /api/data/v9.2/contacts(789-012) HTTP/1.1"));

        // Check Content-IDs
        assert!(batch.body.contains("Content-ID: 1"));
        assert!(batch.body.contains("Content-ID: 2"));
        assert!(batch.body.contains("Content-ID: 3"));

        // Check proper termination
        assert!(batch.body.ends_with("--\r\n"));
    }

    #[test]
    fn test_upsert_operation() {
        let operation = Operation::upsert(
            "contacts",
            "emailaddress1",
            "test@example.com",
            json!({"firstname": "Jane"})
        );

        let batch = BatchRequestBuilder::new("https://test.crm.dynamics.com")
            .add_operation(&operation)
            .build();

        assert!(batch.body.contains("PATCH /api/data/v9.2/contacts(emailaddress1='test@example.com') HTTP/1.1"));
        assert!(batch.body.contains("\"firstname\":\"Jane\""));
    }
}