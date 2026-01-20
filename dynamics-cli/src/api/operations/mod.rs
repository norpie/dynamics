//! Dynamics 365 Operations Module
//!
//! This module provides a unified interface for Dynamics 365 CRUD operations
//! that can be executed individually or in batches.

pub mod batch;
pub mod operation;
pub mod operations;

pub use batch::{BatchRequest, BatchRequestBuilder, BatchResponseParser};
pub use operation::{Operation, OperationResult};
pub use operations::Operations;
