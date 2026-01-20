//! Dynamics 365 $batch response parser
//!
//! Parses multipart/mixed batch responses from Dynamics 365 Web API

use crate::api::operations::{Operation, OperationResult};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
enum ParsingState {
    MultipartHeaders,
    HttpStatus,
    HttpHeaders,
    Body,
}

/// Parsed batch response
#[derive(Debug, Clone)]
pub struct BatchResponse {
    pub results: Vec<BatchResponseItem>,
}

/// Individual response item from a batch
#[derive(Debug, Clone)]
pub struct BatchResponseItem {
    pub content_id: Option<u32>,
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub is_success: bool,
}

/// Parser for batch responses
pub struct BatchResponseParser;

impl BatchResponseParser {
    /// Parse a batch response into individual results
    pub fn parse(
        response_text: &str,
        operations: &[Operation],
    ) -> anyhow::Result<Vec<OperationResult>> {
        let batch_response = Self::parse_multipart(response_text)?;
        Self::map_to_operation_results(batch_response, operations)
    }

    /// Parse the multipart response format
    fn parse_multipart(response_text: &str) -> anyhow::Result<BatchResponse> {
        let mut results = Vec::new();

        // Find the main batch boundary - look for the actual boundary in the response
        let batch_boundary = Self::extract_batch_boundary(response_text)?;

        // Split by batch boundary
        let parts: Vec<&str> = response_text
            .split(&format!("--{}", batch_boundary))
            .collect();

        for part in parts {
            let part = part.trim();
            if part.is_empty() || part == "--" {
                continue;
            }

            // Check if this part contains a changeset
            if part.contains("Content-Type: multipart/mixed") && part.contains("changesetresponse")
            {
                let changeset_results = Self::parse_changeset(part)?;
                results.extend(changeset_results);
            } else if part.contains("Content-Type: application/http") {
                // Direct response (not in changeset)
                if let Ok(item) = Self::parse_http_response(part) {
                    results.push(item);
                }
            }
        }

        Ok(BatchResponse { results })
    }

    /// Parse a changeset section
    fn parse_changeset(changeset_text: &str) -> anyhow::Result<Vec<BatchResponseItem>> {
        let mut results = Vec::new();

        // Find changeset boundary
        let changeset_boundary = Self::extract_changeset_boundary(changeset_text)?;

        // Split by changeset boundary
        let parts: Vec<&str> = changeset_text
            .split(&format!("--{}", changeset_boundary))
            .collect();

        for part in parts {
            let part = part.trim();
            if part.is_empty() || part == "--" {
                continue;
            }

            if part.contains("Content-Type: application/http") {
                if let Ok(item) = Self::parse_http_response(part) {
                    results.push(item);
                }
            }
        }

        Ok(results)
    }

    /// Parse an individual HTTP response
    fn parse_http_response(response_text: &str) -> anyhow::Result<BatchResponseItem> {
        let lines: Vec<&str> = response_text.lines().collect();
        let mut content_id = None;
        let mut status_code = 500;
        let mut headers = HashMap::new();
        let mut body = None;
        let mut state = ParsingState::MultipartHeaders;
        let mut body_lines = Vec::new();

        for line in lines {
            let line = line.trim();

            match state {
                ParsingState::MultipartHeaders => {
                    // Extract Content-ID from multipart headers
                    if line.starts_with("Content-ID:") {
                        if let Some(id_str) = line.split(':').nth(1) {
                            content_id = id_str.trim().parse().ok();
                        }
                        continue;
                    }

                    // Skip other multipart headers
                    if line.starts_with("Content-Type:")
                        || line.starts_with("Content-Transfer-Encoding:")
                    {
                        continue;
                    }

                    // Empty line transitions to HTTP response
                    if line.is_empty() {
                        state = ParsingState::HttpStatus;
                        continue;
                    }
                }
                ParsingState::HttpStatus => {
                    // Parse HTTP status line
                    if line.starts_with("HTTP/1.1") {
                        if let Some(status_str) = line.split_whitespace().nth(1) {
                            status_code = status_str.parse().unwrap_or(500);
                        }
                        state = ParsingState::HttpHeaders;
                        continue;
                    }
                }
                ParsingState::HttpHeaders => {
                    // Empty line transitions to body
                    if line.is_empty() {
                        state = ParsingState::Body;
                        continue;
                    }

                    // Parse HTTP response headers
                    if line.contains(':') {
                        if let Some(colon_pos) = line.find(':') {
                            let header_name = line[..colon_pos].trim().to_string();
                            let header_value = line[colon_pos + 1..].trim().to_string();
                            headers.insert(header_name, header_value);
                        }
                    }
                }
                ParsingState::Body => {
                    // Collect all remaining lines as body
                    body_lines.push(line);
                }
            }
        }

        // Join body lines
        if !body_lines.is_empty() {
            let body_text = body_lines.join("\n").trim().to_string();
            if !body_text.is_empty() {
                body = Some(body_text);
            }
        }

        let is_success = status_code >= 200 && status_code < 300;

        Ok(BatchResponseItem {
            content_id,
            status_code,
            headers,
            body,
            is_success,
        })
    }

    /// Extract batch boundary from the response
    fn extract_batch_boundary(text: &str) -> anyhow::Result<String> {
        // Look for the first boundary line that starts with --batchresponse
        for line in text.lines() {
            if line.starts_with("--batchresponse") {
                let boundary = line.trim_start_matches("--").to_string();
                return Ok(boundary);
            }
        }

        // Fallback: look in Content-Type header
        for line in text.lines() {
            if line.contains("boundary=") && line.contains("batchresponse") {
                if let Some(boundary_pos) = line.find("boundary=") {
                    let boundary_part = &line[boundary_pos + 9..]; // Skip "boundary="
                    let boundary = boundary_part
                        .split_whitespace()
                        .next()
                        .unwrap_or(boundary_part)
                        .trim_start_matches('"')
                        .trim_end_matches('"')
                        .to_string();
                    return Ok(boundary);
                }
            }
        }

        anyhow::bail!("Could not find batch boundary in response")
    }

    /// Extract changeset boundary from changeset content
    fn extract_changeset_boundary(text: &str) -> anyhow::Result<String> {
        // Look for the first boundary line that starts with --changesetresponse
        for line in text.lines() {
            if line.starts_with("--changesetresponse") {
                let boundary = line.trim_start_matches("--").to_string();
                return Ok(boundary);
            }
        }

        // Fallback: look in Content-Type header
        for line in text.lines() {
            if line.contains("boundary=") && line.contains("changesetresponse") {
                if let Some(boundary_pos) = line.find("boundary=") {
                    let boundary_part = &line[boundary_pos + 9..]; // Skip "boundary="
                    let boundary = boundary_part
                        .split_whitespace()
                        .next()
                        .unwrap_or(boundary_part)
                        .trim_start_matches('"')
                        .trim_end_matches('"')
                        .to_string();
                    return Ok(boundary);
                }
            }
        }

        anyhow::bail!("Could not find changeset boundary in response")
    }

    /// Map batch response items to operation results
    fn map_to_operation_results(
        batch_response: BatchResponse,
        operations: &[Operation],
    ) -> anyhow::Result<Vec<OperationResult>> {
        let mut results = Vec::new();

        for (index, operation) in operations.iter().enumerate() {
            // Find matching response by Content-ID or index
            let response_item = batch_response
                .results
                .iter()
                .find(|item| item.content_id.map(|id| id as usize) == Some(index + 1))
                .or_else(|| batch_response.results.get(index));

            if let Some(item) = response_item {
                let data = if item.is_success {
                    // Try to parse JSON response
                    item.body.as_ref().and_then(|body| {
                        let trimmed = body.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            serde_json::from_str::<Value>(trimmed).ok()
                        }
                    })
                } else {
                    None
                };

                let error = if !item.is_success {
                    // Try to extract detailed error from response body
                    Self::extract_error_message(item.body.as_ref())
                        .or_else(|| item.body.clone())
                        .or_else(|| Some(format!("HTTP {}", item.status_code)))
                } else {
                    None
                };

                results.push(OperationResult {
                    operation: operation.clone(),
                    success: item.is_success,
                    data,
                    error,
                    status_code: Some(item.status_code),
                    headers: item.headers.clone(),
                });
            } else {
                // No response found for this operation
                results.push(OperationResult {
                    operation: operation.clone(),
                    success: false,
                    data: None,
                    error: Some("No response found in batch".to_string()),
                    status_code: None,
                    headers: HashMap::new(),
                });
            }
        }

        Ok(results)
    }

    /// Extract error message from Dynamics 365 error response
    fn extract_error_message(body: Option<&String>) -> Option<String> {
        let body = body?;

        // Try to parse as JSON error response
        if let Ok(json_value) = serde_json::from_str::<Value>(body) {
            // Standard Dynamics 365 error format: {"error":{"code":"...","message":"..."}}
            if let Some(error_obj) = json_value.get("error") {
                if let Some(message) = error_obj.get("message").and_then(|m| m.as_str()) {
                    let code = error_obj
                        .get("code")
                        .and_then(|c| c.as_str())
                        .unwrap_or("Unknown");
                    return Some(format!("Dynamics 365 Error [{}]: {}", code, message));
                }
            }

            // Alternative error format: {"Message":"..."}
            if let Some(message) = json_value.get("Message").and_then(|m| m.as_str()) {
                return Some(format!("Dynamics 365 Error: {}", message));
            }

            // Fallback to full JSON if it's an error structure
            if json_value.is_object() {
                return Some(format!("Dynamics 365 Error: {}", body));
            }
        }

        // If not JSON or no recognizable error structure, return the raw body
        if !body.trim().is_empty() {
            Some(body.clone())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::operations::Operation;
    use serde_json::json;

    #[test]
    fn test_parse_simple_batch_response() {
        let response = r#"--batchresponse_f44bd09d-573f-4a30-bca0-2e500ee7e139
Content-Type: multipart/mixed; boundary=changesetresponse_ee30dcdb-1094-4c24-8170-262eae9336a4

--changesetresponse_ee30dcdb-1094-4c24-8170-262eae9336a4
Content-Type: application/http
Content-Transfer-Encoding: binary
Content-ID: 1

HTTP/1.1 201 Created
Content-Type: application/json; odata.metadata=minimal
OData-Version: 4.0
Location: https://test.crm.dynamics.com/api/data/v9.2/contacts(abc-123)

{"contactid":"abc-123","firstname":"John"}
--changesetresponse_ee30dcdb-1094-4c24-8170-262eae9336a4--
--batchresponse_f44bd09d-573f-4a30-bca0-2e500ee7e139--"#;

        let operations = vec![Operation::create("contacts", json!({"firstname": "John"}))];

        let results = BatchResponseParser::parse(response, &operations).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_success());
        assert!(results[0].data.is_some());
    }

    #[test]
    fn test_parse_error_response() {
        let response = r#"--batchresponse_f44bd09d-573f-4a30-bca0-2e500ee7e139
Content-Type: multipart/mixed; boundary=changesetresponse_ee30dcdb-1094-4c24-8170-262eae9336a4

--changesetresponse_ee30dcdb-1094-4c24-8170-262eae9336a4
Content-Type: application/http
Content-Transfer-Encoding: binary
Content-ID: 1

HTTP/1.1 400 Bad Request
Content-Type: application/json; odata.metadata=minimal
OData-Version: 4.0

{"error":{"code":"0x80060888","message":"Bad Request - Error in query syntax."}}
--changesetresponse_ee30dcdb-1094-4c24-8170-262eae9336a4--
--batchresponse_f44bd09d-573f-4a30-bca0-2e500ee7e139--"#;

        let operations = vec![Operation::create("contacts", json!({"firstname": "John"}))];

        let results = BatchResponseParser::parse(response, &operations).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_error());
        assert!(results[0].error.is_some());
        assert_eq!(results[0].status_code, Some(400));
    }

    #[test]
    fn test_extract_batch_boundary() {
        let text = "--batchresponse_12345\nContent-Type: application/http";
        let boundary = BatchResponseParser::extract_batch_boundary(text).unwrap();
        assert_eq!(boundary, "batchresponse_12345");

        let text2 = "Content-Type: multipart/mixed; boundary=batchresponse_67890";
        let boundary2 = BatchResponseParser::extract_batch_boundary(text2).unwrap();
        assert_eq!(boundary2, "batchresponse_67890");
    }

    #[test]
    fn test_extract_error_message() {
        // Test standard Dynamics 365 error format
        let error_json =
            r#"{"error":{"code":"0x80060888","message":"Bad Request - Error in query syntax."}}"#;
        let error_msg = BatchResponseParser::extract_error_message(Some(&error_json.to_string()));
        assert_eq!(
            error_msg,
            Some(
                "Dynamics 365 Error [0x80060888]: Bad Request - Error in query syntax.".to_string()
            )
        );

        // Test alternative Message format
        let message_json = r#"{"Message":"Invalid entity name"}"#;
        let message_msg =
            BatchResponseParser::extract_error_message(Some(&message_json.to_string()));
        assert_eq!(
            message_msg,
            Some("Dynamics 365 Error: Invalid entity name".to_string())
        );

        // Test plain text error
        let plain_error = "Not Found";
        let plain_msg = BatchResponseParser::extract_error_message(Some(&plain_error.to_string()));
        assert_eq!(plain_msg, Some("Not Found".to_string()));

        // Test empty/None
        let empty_msg = BatchResponseParser::extract_error_message(None);
        assert_eq!(empty_msg, None);
    }
}
