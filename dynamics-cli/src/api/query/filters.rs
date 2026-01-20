//! OData Filter building
//!
//! Provides type-safe filter construction for OData queries

use serde_json::Value;

#[derive(Debug, Clone)]
pub enum Filter {
    // Comparison operators
    Eq(String, FilterValue),
    Ne(String, FilterValue),
    Gt(String, FilterValue),
    Ge(String, FilterValue),
    Lt(String, FilterValue),
    Le(String, FilterValue),

    // String functions
    Contains(String, String),
    StartsWith(String, String),
    EndsWith(String, String),

    // Logical operators
    And(Vec<Filter>),
    Or(Vec<Filter>),
    Not(Box<Filter>),

    // Raw OData filter for advanced cases
    Raw(String),
}

#[derive(Debug, Clone)]
pub enum FilterValue {
    String(String),
    Guid(String), // GUIDs don't use quotes in OData
    Number(f64),
    Integer(i64),
    Boolean(bool),
    Null,
}

impl Filter {
    // Comparison operators
    pub fn eq(field: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        Self::Eq(field.into(), value.into())
    }

    pub fn ne(field: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        Self::Ne(field.into(), value.into())
    }

    pub fn gt(field: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        Self::Gt(field.into(), value.into())
    }

    pub fn ge(field: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        Self::Ge(field.into(), value.into())
    }

    pub fn lt(field: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        Self::Lt(field.into(), value.into())
    }

    pub fn le(field: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        Self::Le(field.into(), value.into())
    }

    // String functions
    pub fn contains(field: impl Into<String>, value: impl Into<String>) -> Self {
        Self::Contains(field.into(), value.into())
    }

    pub fn starts_with(field: impl Into<String>, value: impl Into<String>) -> Self {
        Self::StartsWith(field.into(), value.into())
    }

    pub fn ends_with(field: impl Into<String>, value: impl Into<String>) -> Self {
        Self::EndsWith(field.into(), value.into())
    }

    // Logical operators
    pub fn and(filters: Vec<Filter>) -> Self {
        Self::And(filters)
    }

    pub fn or(filters: Vec<Filter>) -> Self {
        Self::Or(filters)
    }

    pub fn not(filter: Filter) -> Self {
        Self::Not(Box::new(filter))
    }

    // Raw filter for advanced cases
    pub fn raw(filter: impl Into<String>) -> Self {
        Self::Raw(filter.into())
    }

    /// Build an OR filter for matching multiple GUID values on a field
    /// Example: field eq id1 or field eq id2 or field eq id3
    ///
    /// This is a common pattern when querying by multiple IDs.
    pub fn any_of(field: impl Into<String>, ids: &[String]) -> Self {
        let field_name = field.into();
        let filters: Vec<Filter> = ids
            .iter()
            .map(|id| Filter::eq(field_name.clone(), FilterValue::Guid(id.clone())))
            .collect();
        Self::Or(filters)
    }

    /// Convert filter to OData query string
    pub fn to_odata_string(&self) -> String {
        match self {
            Filter::Eq(field, value) => format!("{} eq {}", field, value.to_odata_string()),
            Filter::Ne(field, value) => format!("{} ne {}", field, value.to_odata_string()),
            Filter::Gt(field, value) => format!("{} gt {}", field, value.to_odata_string()),
            Filter::Ge(field, value) => format!("{} ge {}", field, value.to_odata_string()),
            Filter::Lt(field, value) => format!("{} lt {}", field, value.to_odata_string()),
            Filter::Le(field, value) => format!("{} le {}", field, value.to_odata_string()),

            Filter::Contains(field, value) => {
                format!("contains({}, '{}')", field, value.replace('\'', "''"))
            }
            Filter::StartsWith(field, value) => {
                format!("startswith({}, '{}')", field, value.replace('\'', "''"))
            }
            Filter::EndsWith(field, value) => {
                format!("endswith({}, '{}')", field, value.replace('\'', "''"))
            }

            Filter::And(filters) => {
                let filter_strings: Vec<String> =
                    filters.iter().map(|f| f.to_odata_string()).collect();
                format!("({})", filter_strings.join(" and "))
            }
            Filter::Or(filters) => {
                let filter_strings: Vec<String> =
                    filters.iter().map(|f| f.to_odata_string()).collect();
                format!("({})", filter_strings.join(" or "))
            }
            Filter::Not(filter) => format!("not ({})", filter.to_odata_string()),

            Filter::Raw(raw) => raw.clone(),
        }
    }
}

impl FilterValue {
    pub fn to_odata_string(&self) -> String {
        match self {
            FilterValue::String(s) => format!("'{}'", s.replace('\'', "''")),
            FilterValue::Guid(g) => g.clone(), // GUIDs don't use quotes
            FilterValue::Number(n) => n.to_string(),
            FilterValue::Integer(i) => i.to_string(),
            FilterValue::Boolean(b) => b.to_string(),
            FilterValue::Null => "null".to_string(),
        }
    }
}

// Convenient From implementations for FilterValue
impl From<String> for FilterValue {
    fn from(value: String) -> Self {
        FilterValue::String(value)
    }
}

impl From<&str> for FilterValue {
    fn from(value: &str) -> Self {
        FilterValue::String(value.to_string())
    }
}

impl From<f64> for FilterValue {
    fn from(value: f64) -> Self {
        FilterValue::Number(value)
    }
}

impl From<i64> for FilterValue {
    fn from(value: i64) -> Self {
        FilterValue::Integer(value)
    }
}

impl From<i32> for FilterValue {
    fn from(value: i32) -> Self {
        FilterValue::Integer(value as i64)
    }
}

impl From<bool> for FilterValue {
    fn from(value: bool) -> Self {
        FilterValue::Boolean(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comparison_filters() {
        assert_eq!(
            Filter::eq("statecode", 0).to_odata_string(),
            "statecode eq 0"
        );
        assert_eq!(
            Filter::ne("firstname", "John").to_odata_string(),
            "firstname ne 'John'"
        );
        assert_eq!(
            Filter::gt("createdon", "2023-01-01").to_odata_string(),
            "createdon gt '2023-01-01'"
        );
    }

    #[test]
    fn test_string_functions() {
        assert_eq!(
            Filter::contains("firstname", "John").to_odata_string(),
            "contains(firstname, 'John')"
        );
        assert_eq!(
            Filter::starts_with("lastname", "Sm").to_odata_string(),
            "startswith(lastname, 'Sm')"
        );
    }

    #[test]
    fn test_logical_operators() {
        let and_filter = Filter::and(vec![
            Filter::eq("statecode", 0),
            Filter::contains("firstname", "John"),
        ]);
        assert_eq!(
            and_filter.to_odata_string(),
            "(statecode eq 0 and contains(firstname, 'John'))"
        );

        let or_filter = Filter::or(vec![Filter::eq("statecode", 0), Filter::eq("statecode", 1)]);
        assert_eq!(
            or_filter.to_odata_string(),
            "(statecode eq 0 or statecode eq 1)"
        );
    }

    #[test]
    fn test_nested_filters() {
        let complex_filter = Filter::and(vec![
            Filter::eq("statecode", 0),
            Filter::or(vec![
                Filter::contains("firstname", "John"),
                Filter::contains("lastname", "Smith"),
            ]),
        ]);
        let expected =
            "(statecode eq 0 and (contains(firstname, 'John') or contains(lastname, 'Smith')))";
        assert_eq!(complex_filter.to_odata_string(), expected);
    }

    #[test]
    fn test_quote_escaping() {
        let filter = Filter::contains("firstname", "O'Connor");
        assert_eq!(filter.to_odata_string(), "contains(firstname, 'O''Connor')");
    }
}
