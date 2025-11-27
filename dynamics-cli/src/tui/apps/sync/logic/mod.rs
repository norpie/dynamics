//! Business logic for the Entity Sync App
//!
//! This module contains pure functions for:
//! - Schema comparison and diff generation
//! - Dependency graph building and topological sorting
//! - Junction entity detection

pub mod schema_diff;
pub mod dependency_graph;
pub mod junction_detect;

pub use schema_diff::*;
pub use dependency_graph::*;
pub use junction_detect::*;
