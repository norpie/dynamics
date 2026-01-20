//! Batch operations module for Dynamics 365 Web API
//!
//! Provides proper $batch request building and response parsing

pub mod builder;
pub mod parser;

pub use builder::{BatchRequest, BatchRequestBuilder};
pub use parser::{BatchResponse, BatchResponseItem, BatchResponseParser};
