//! OData Query Builder Module
//!
//! Provides fluent API for building and executing OData queries against Dynamics 365.
//! Follows the same pattern as operations with Query (reusable) and QueryBuilder (fluent).

pub mod builder;
pub mod filters;
pub mod orderby;
pub mod query;
pub mod result;

pub use builder::QueryBuilder;
pub use filters::{Filter, FilterValue};
pub use orderby::OrderBy;
pub use query::Query;
pub use result::{QueryResponse, QueryResult};
