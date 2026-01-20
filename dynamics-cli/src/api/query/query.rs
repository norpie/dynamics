//! Reusable Query object
//!
//! Represents a complete OData query that can be executed multiple times

use super::filters::Filter;
use super::orderby::OrderByClause;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Query {
    pub entity: String,
    pub select: Option<Vec<String>>,
    pub filter: Option<Filter>,
    pub orderby: OrderByClause,
    pub expand: Option<Vec<String>>,
    pub top: Option<u32>,
    pub skip: Option<u32>,
    pub count: bool,
}

impl Query {
    pub fn new(entity: impl Into<String>) -> Self {
        Self {
            entity: entity.into(),
            select: None,
            filter: None,
            orderby: OrderByClause::new(),
            expand: None,
            top: None,
            skip: None,
            count: false,
        }
    }

    /// Clone and modify - useful for creating variations of base queries
    pub fn with_top(mut self, top: u32) -> Self {
        self.top = Some(top);
        self
    }

    pub fn with_filter(mut self, filter: Filter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Generate the full OData query URL
    pub fn to_url(&self, base_url: &str) -> String {
        let mut url = format!("{}/api/data/v9.2/{}", base_url, self.entity);
        let mut params = Vec::new();

        // Add query parameters
        if let Some(select) = &self.select {
            params.push(format!("$select={}", select.join(",")));
        }

        if let Some(filter) = &self.filter {
            params.push(format!(
                "$filter={}",
                urlencoding::encode(&filter.to_odata_string())
            ));
        }

        if let Some(orderby) = self.orderby.to_odata_string() {
            params.push(format!("$orderby={}", urlencoding::encode(&orderby)));
        }

        if let Some(expand) = &self.expand {
            params.push(format!("$expand={}", expand.join(",")));
        }

        if let Some(top) = self.top {
            params.push(format!("$top={}", top));
        }

        if let Some(skip) = self.skip {
            params.push(format!("$skip={}", skip));
        }

        if self.count {
            params.push("$count=true".to_string());
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        url
    }

    /// Get query parameters as a HashMap for use with HTTP client
    pub fn to_query_params(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();

        if let Some(select) = &self.select {
            params.insert("$select".to_string(), select.join(","));
        }

        if let Some(filter) = &self.filter {
            params.insert("$filter".to_string(), filter.to_odata_string());
        }

        if let Some(orderby) = self.orderby.to_odata_string() {
            params.insert("$orderby".to_string(), orderby);
        }

        if let Some(expand) = &self.expand {
            params.insert("$expand".to_string(), expand.join(","));
        }

        if let Some(top) = self.top {
            params.insert("$top".to_string(), top.to_string());
        }

        if let Some(skip) = self.skip {
            params.insert("$skip".to_string(), skip.to_string());
        }

        if self.count {
            params.insert("$count".to_string(), "true".to_string());
        }

        params
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::query::filters::Filter;
    use crate::api::query::orderby::OrderBy;

    #[test]
    fn test_basic_query_url() {
        let query = Query::new("contacts");
        let url = query.to_url("https://test.crm.dynamics.com");
        assert_eq!(url, "https://test.crm.dynamics.com/api/data/v9.2/contacts");
    }

    #[test]
    fn test_complex_query_url() {
        let mut query = Query::new("contacts");
        query.select = Some(vec!["firstname".to_string(), "lastname".to_string()]);
        query.filter = Some(Filter::eq("statecode", 0));
        query.orderby = query.orderby.add(OrderBy::desc("createdon"));
        query.top = Some(10);
        query.count = true;

        let url = query.to_url("https://test.crm.dynamics.com");

        // URL should contain all parameters (order may vary)
        assert!(url.contains("$select=firstname,lastname"));
        assert!(url.contains("$filter=statecode%20eq%200"));
        assert!(url.contains("$orderby=createdon%20desc"));
        assert!(url.contains("$top=10"));
        assert!(url.contains("$count=true"));
    }

    #[test]
    fn test_query_params() {
        let mut query = Query::new("contacts");
        query.select = Some(vec!["firstname".to_string(), "lastname".to_string()]);
        query.filter = Some(Filter::eq("statecode", 0));
        query.top = Some(10);

        let params = query.to_query_params();

        assert_eq!(
            params.get("$select"),
            Some(&"firstname,lastname".to_string())
        );
        assert_eq!(params.get("$filter"), Some(&"statecode eq 0".to_string()));
        assert_eq!(params.get("$top"), Some(&"10".to_string()));
    }

    #[test]
    fn test_query_with_modifications() {
        let base_query = Query::new("contacts").with_filter(Filter::eq("statecode", 0));

        let query_with_top = base_query.clone().with_top(10);
        let query_with_filter = base_query.with_filter(Filter::contains("firstname", "Test"));

        assert_eq!(query_with_top.top, Some(10));
        assert!(query_with_filter.filter.is_some());
    }
}
