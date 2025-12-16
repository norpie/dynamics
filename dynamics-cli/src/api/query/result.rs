//! Query result handling
//!
//! Handles OData query responses from Dynamics 365

use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub success: bool,
    pub data: Option<QueryResponse>,
    pub error: Option<String>,
    pub status_code: Option<u16>,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct QueryResponse {
    pub value: Vec<Value>,
    pub count: Option<u64>,
    pub next_link: Option<String>,
}

impl QueryResult {
    pub fn success(data: QueryResponse, status_code: u16, headers: HashMap<String, String>) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            status_code: Some(status_code),
            headers,
        }
    }

    pub fn error(error: String, status_code: Option<u16>, headers: HashMap<String, String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            status_code,
            headers,
        }
    }

    pub fn is_success(&self) -> bool {
        self.success
    }

    pub fn is_error(&self) -> bool {
        !self.success
    }

    /// Get the records from the response
    pub fn records(&self) -> Option<&Vec<Value>> {
        self.data.as_ref().map(|d| &d.value)
    }

    /// Get the count if requested in query
    pub fn count(&self) -> Option<u64> {
        self.data.as_ref().and_then(|d| d.count)
    }

    /// Get the next link for pagination
    pub fn next_link(&self) -> Option<&String> {
        self.data.as_ref().and_then(|d| d.next_link.as_ref())
    }

    /// Check if there are more results available
    pub fn has_more(&self) -> bool {
        self.next_link().is_some()
    }

    /// Get number of records returned
    pub fn len(&self) -> usize {
        self.records().map(|r| r.len()).unwrap_or(0)
    }

    /// Check if no records were returned
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Execute the next page if available
    pub async fn next_page(&self, client: &crate::api::DynamicsClient, page_size: Option<u32>) -> anyhow::Result<Option<QueryResult>> {
        match self.next_link() {
            Some(next_link) => Ok(Some(client.execute_next_page(next_link, page_size).await?)),
            None => Ok(None),
        }
    }
}

impl QueryResponse {
    /// Parse OData response JSON into QueryResponse
    pub fn from_json(json: Value) -> anyhow::Result<Self> {
        let value = json.get("value")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'value' array in response"))?
            .clone();

        let count = json.get("@odata.count")
            .and_then(|c| c.as_u64());

        let next_link = json.get("@odata.nextLink")
            .and_then(|n| n.as_str())
            .map(|s| s.to_string());

        Ok(Self {
            value,
            count,
            next_link,
        })
    }

    /// Get a specific field from all records
    pub fn get_field_values(&self, field_name: &str) -> Vec<Option<&Value>> {
        self.value.iter()
            .map(|record| record.get(field_name))
            .collect()
    }

    /// Find records where a field matches a value
    pub fn find_by_field(&self, field_name: &str, field_value: &Value) -> Vec<&Value> {
        self.value.iter()
            .filter(|record| {
                record.get(field_name)
                    .map(|v| v == field_value)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Get the first record (convenience method)
    pub fn first(&self) -> Option<&Value> {
        self.value.first()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_query_response_from_json() {
        let json = json!({
            "value": [
                {"contactid": "123", "firstname": "John"},
                {"contactid": "456", "firstname": "Jane"}
            ],
            "@odata.count": 2,
            "@odata.nextLink": "https://api.example.com/contacts?$skip=10"
        });

        let response = QueryResponse::from_json(json).unwrap();

        assert_eq!(response.value.len(), 2);
        assert_eq!(response.count, Some(2));
        assert_eq!(response.next_link, Some("https://api.example.com/contacts?$skip=10".to_string()));
    }

    #[test]
    fn test_query_response_minimal() {
        let json = json!({
            "value": [
                {"contactid": "123", "firstname": "John"}
            ]
        });

        let response = QueryResponse::from_json(json).unwrap();

        assert_eq!(response.value.len(), 1);
        assert_eq!(response.count, None);
        assert_eq!(response.next_link, None);
    }

    #[test]
    fn test_query_result_success() {
        let response = QueryResponse {
            value: vec![json!({"id": "123"})],
            count: Some(1),
            next_link: None,
        };

        let result = QueryResult::success(response, 200, HashMap::new());

        assert!(result.is_success());
        assert!(!result.is_error());
        assert_eq!(result.len(), 1);
        assert!(!result.has_more());
    }

    #[test]
    fn test_query_result_error() {
        let result = QueryResult::error(
            "Not found".to_string(),
            Some(404),
            HashMap::new(),
        );

        assert!(result.is_error());
        assert!(!result.is_success());
        assert_eq!(result.len(), 0);
        assert_eq!(result.error, Some("Not found".to_string()));
    }

    #[test]
    fn test_query_response_helpers() {
        let response = QueryResponse {
            value: vec![
                json!({"contactid": "123", "firstname": "John", "statecode": 0}),
                json!({"contactid": "456", "firstname": "Jane", "statecode": 1}),
            ],
            count: None,
            next_link: None,
        };

        // Test get_field_values
        let firstnames = response.get_field_values("firstname");
        assert_eq!(firstnames.len(), 2);
        assert_eq!(firstnames[0], Some(&json!("John")));

        // Test find_by_field
        let active_contacts = response.find_by_field("statecode", &json!(0));
        assert_eq!(active_contacts.len(), 1);

        // Test first
        let first_contact = response.first().unwrap();
        assert_eq!(first_contact.get("contactid"), Some(&json!("123")));
    }

    #[test]
    fn test_pagination_support() {
        // Test result with next link
        let response_with_next = QueryResponse {
            value: vec![json!({"id": "123"})],
            count: Some(10),
            next_link: Some("https://api.example.com/contacts?$skip=5&$top=5".to_string()),
        };

        let result_with_next = QueryResult::success(response_with_next, 200, HashMap::new());

        assert!(result_with_next.has_more());
        assert_eq!(result_with_next.count(), Some(10));
        assert!(result_with_next.next_link().is_some());

        // Test result without next link (final page)
        let response_final = QueryResponse {
            value: vec![json!({"id": "456"})],
            count: Some(6),
            next_link: None,
        };

        let result_final = QueryResult::success(response_final, 200, HashMap::new());

        assert!(!result_final.has_more());
        assert_eq!(result_final.count(), Some(6));
        assert!(result_final.next_link().is_none());
    }
}