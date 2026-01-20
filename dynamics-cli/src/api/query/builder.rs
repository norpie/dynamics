//! QueryBuilder for fluent query construction
//!
//! Provides a fluent API that builds Query objects for execution

use super::filters::Filter;
use super::orderby::{OrderBy, OrderByClause};
use super::query::Query;
use super::result::QueryResult;
use crate::api::client::DynamicsClient;

#[derive(Debug, Clone)]
pub struct QueryBuilder {
    query: Query,
}

impl QueryBuilder {
    pub fn new(entity: impl Into<String>) -> Self {
        Self {
            query: Query::new(entity),
        }
    }

    /// Select specific fields
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.query.select = Some(fields.iter().map(|f| f.to_string()).collect());
        self
    }

    /// Add filter condition
    pub fn filter(mut self, filter: Filter) -> Self {
        self.query.filter = Some(filter);
        self
    }

    /// Add ordering
    pub fn orderby(mut self, order: OrderBy) -> Self {
        self.query.orderby = self.query.orderby.add(order);
        self
    }

    /// Add multiple ordering clauses
    pub fn orderby_multiple(mut self, orders: Vec<OrderBy>) -> Self {
        for order in orders {
            self.query.orderby = self.query.orderby.add(order);
        }
        self
    }

    /// Expand related entities
    pub fn expand(mut self, expansions: &[&str]) -> Self {
        self.query.expand = Some(expansions.iter().map(|e| e.to_string()).collect());
        self
    }

    /// Limit number of results
    pub fn top(mut self, top: u32) -> Self {
        self.query.top = Some(top);
        self
    }

    /// Skip a number of results (for pagination)
    pub fn skip(mut self, skip: u32) -> Self {
        self.query.skip = Some(skip);
        self
    }

    /// Include count in response
    pub fn count(mut self) -> Self {
        self.query.count = true;
        self
    }

    /// Build the final Query object (reusable)
    pub fn build(self) -> Query {
        self.query
    }

    /// Build and execute immediately
    pub async fn execute(self, client: &DynamicsClient) -> anyhow::Result<QueryResult> {
        let query = self.build();
        client.execute_query(&query).await
    }
}

// Convenience methods for common patterns
impl QueryBuilder {
    /// Select all active records (statecode = 0)
    pub fn active_only(self) -> Self {
        self.filter(Filter::eq("statecode", 0))
    }

    /// Select records created after a date
    pub fn created_after(self, date: impl Into<String>) -> Self {
        self.filter(Filter::gt("createdon", date.into()))
    }

    /// Select records created before a date
    pub fn created_before(self, date: impl Into<String>) -> Self {
        self.filter(Filter::lt("createdon", date.into()))
    }

    /// Order by creation date (newest first)
    pub fn newest_first(self) -> Self {
        self.orderby(OrderBy::desc("createdon"))
    }

    /// Order by creation date (oldest first)
    pub fn oldest_first(self) -> Self {
        self.orderby(OrderBy::asc("createdon"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::query::filters::Filter;

    #[test]
    fn test_basic_query_builder() {
        let query = QueryBuilder::new("contacts")
            .select(&["firstname", "lastname"])
            .filter(Filter::eq("statecode", 0))
            .orderby(OrderBy::desc("createdon"))
            .top(10)
            .build();

        assert_eq!(query.entity, "contacts");
        assert_eq!(
            query.select,
            Some(vec!["firstname".to_string(), "lastname".to_string()])
        );
        assert!(query.filter.is_some());
        assert_eq!(query.top, Some(10));
    }

    #[test]
    fn test_multiple_orderby() {
        let query = QueryBuilder::new("contacts")
            .orderby(OrderBy::asc("lastname"))
            .orderby(OrderBy::desc("createdon"))
            .build();

        let orderby_string = query.orderby.to_odata_string().unwrap();
        assert_eq!(orderby_string, "lastname asc, createdon desc");
    }

    #[test]
    fn test_convenience_methods() {
        let query = QueryBuilder::new("contacts")
            .active_only()
            .newest_first()
            .top(25)
            .build();

        // Should filter for active records
        assert!(query.filter.is_some());

        // Should have top=25
        assert_eq!(query.top, Some(25));

        // Should have ordering
        assert!(query.orderby.to_odata_string().is_some());
    }

    #[test]
    fn test_complex_filter_building() {
        let query = QueryBuilder::new("contacts")
            .filter(Filter::and(vec![
                Filter::eq("statecode", 0),
                Filter::or(vec![
                    Filter::contains("firstname", "John"),
                    Filter::contains("lastname", "Smith"),
                ]),
            ]))
            .build();

        if let Some(filter) = &query.filter {
            let filter_string = filter.to_odata_string();
            assert!(filter_string.contains("statecode eq 0"));
            assert!(filter_string.contains("contains(firstname, 'John')"));
            assert!(filter_string.contains("contains(lastname, 'Smith')"));
        } else {
            panic!("Filter should be set");
        }
    }

    #[test]
    fn test_expand_and_select() {
        let query = QueryBuilder::new("contacts")
            .select(&["firstname", "lastname"])
            .expand(&["parentcontactid($select=fullname)", "account($select=name)"])
            .build();

        assert_eq!(
            query.expand,
            Some(vec![
                "parentcontactid($select=fullname)".to_string(),
                "account($select=name)".to_string()
            ])
        );
    }
}
