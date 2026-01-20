//! OData OrderBy building
//!
//! Provides type-safe ordering construction for OData queries

#[derive(Debug, Clone)]
pub enum OrderBy {
    Asc(String),
    Desc(String),
}

impl OrderBy {
    pub fn asc(field: impl Into<String>) -> Self {
        Self::Asc(field.into())
    }

    pub fn desc(field: impl Into<String>) -> Self {
        Self::Desc(field.into())
    }

    /// Convert to OData orderby string
    pub fn to_odata_string(&self) -> String {
        match self {
            OrderBy::Asc(field) => format!("{} asc", field),
            OrderBy::Desc(field) => format!("{} desc", field),
        }
    }
}

/// Helper to combine multiple OrderBy clauses
#[derive(Debug, Clone)]
pub struct OrderByClause {
    clauses: Vec<OrderBy>,
}

impl OrderByClause {
    pub fn new() -> Self {
        Self {
            clauses: Vec::new(),
        }
    }

    pub fn add(mut self, order: OrderBy) -> Self {
        self.clauses.push(order);
        self
    }

    pub fn to_odata_string(&self) -> Option<String> {
        if self.clauses.is_empty() {
            None
        } else {
            let order_strings: Vec<String> =
                self.clauses.iter().map(|o| o.to_odata_string()).collect();
            Some(order_strings.join(", "))
        }
    }
}

impl Default for OrderByClause {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_orderby() {
        assert_eq!(OrderBy::asc("firstname").to_odata_string(), "firstname asc");
        assert_eq!(
            OrderBy::desc("createdon").to_odata_string(),
            "createdon desc"
        );
    }

    #[test]
    fn test_multiple_orderby() {
        let clause = OrderByClause::new()
            .add(OrderBy::asc("lastname"))
            .add(OrderBy::desc("createdon"));

        assert_eq!(
            clause.to_odata_string(),
            Some("lastname asc, createdon desc".to_string())
        );
    }

    #[test]
    fn test_empty_orderby() {
        let clause = OrderByClause::new();
        assert_eq!(clause.to_odata_string(), None);
    }
}
