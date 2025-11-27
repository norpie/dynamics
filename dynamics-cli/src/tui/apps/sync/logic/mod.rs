//! Business logic for the Entity Sync App
//!
//! This module contains pure functions for:
//! - Schema comparison and diff generation
//! - Dependency graph building and topological sorting
//! - Junction entity detection
//! - Operation building and ordering
//! - Report generation

pub mod schema_diff;
pub mod dependency_graph;
pub mod junction_detect;
pub mod operation_builder;
pub mod report_builder;

pub use schema_diff::*;
pub use dependency_graph::*;
pub use junction_detect::*;
pub use operation_builder::*;
pub use report_builder::*;
