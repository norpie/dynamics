//! Comprehensive Dynamics 365 Web API Module
//!
//! This module provides a complete, modern interface to the Microsoft Dynamics 365 Web API.
//! It builds upon the existing temporary implementations in src/dynamics/ to create a
//! production-ready API client with full CRUD operations, OData query building,
//! batch processing, and enterprise-grade features.

pub mod auth;
pub mod client;
pub mod constants;
pub mod manager;
pub mod metadata;
pub mod models;
pub mod operations;
pub mod pluralization;
pub mod query;
pub mod resilience;

pub use auth::AuthManager;
pub use client::{DynamicsClient, EntityMetadataInfo};
pub use manager::ClientManager;
pub use models::{Environment, CredentialSet, TokenInfo};
pub use operations::{Operation, OperationResult, Operations};
pub use query::{Query, QueryBuilder, QueryResult, Filter, FilterValue, OrderBy};
pub use resilience::{RetryPolicy, RetryConfig, ResilienceConfig, RateLimitConfig, MonitoringConfig, LogLevel, RateLimiterStats, RateLimiter, RetryableError, ApiLogger, OperationContext, OperationMetrics, MetricsCollector, MetricsSnapshot, OperationTypeMetrics, EntityMetrics, GlobalMetrics};
pub use metadata::{
    parse_entity_list, parse_entity_metadata,
    EntityMetadata, FieldMetadata, FieldType, FormMetadata, RelationshipMetadata,
    RelationshipType, ViewMetadata,
};