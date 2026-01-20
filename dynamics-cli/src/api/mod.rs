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
pub use client::{DynamicsClient, EntityMetadataInfo, IncomingReference, ManyToManyRelationship};
pub use manager::ClientManager;
pub use metadata::{
    EntityMetadata, FieldMetadata, FieldType, FormMetadata, RelationshipMetadata, RelationshipType,
    ViewMetadata, parse_entity_list, parse_entity_metadata,
};
pub use models::{CredentialSet, Environment, TokenInfo};
pub use operations::{Operation, OperationResult, Operations};
pub use query::{Filter, FilterValue, OrderBy, Query, QueryBuilder, QueryResult};
pub use resilience::{
    ApiLogger, EntityMetrics, GlobalMetrics, LogLevel, MetricsCollector, MetricsSnapshot,
    MonitoringConfig, OperationContext, OperationMetrics, OperationTypeMetrics, RateLimitConfig,
    RateLimiter, RateLimiterStats, ResilienceConfig, RetryConfig, RetryPolicy, RetryableError,
};
