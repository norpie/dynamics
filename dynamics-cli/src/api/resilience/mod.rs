//! Production resilience and hardening features
//!
//! Provides retry policies, rate limiting, concurrency limiting, and monitoring
//! capabilities for production-grade Dynamics 365 API interactions.

pub mod concurrency;
pub mod config;
pub mod logging;
pub mod metrics;
pub mod rate_limiter;
pub mod retry;

pub use concurrency::{ConcurrencyLimiter, ConcurrencyStats};
pub use config::{
    BypassConfig, ConcurrencyConfig, LogLevel, MonitoringConfig, RateLimitConfig, ResilienceConfig,
};
pub use logging::{ApiLogger, OperationContext, OperationMetrics};
pub use metrics::{
    EntityMetrics, GlobalMetrics, MetricsCollector, MetricsSnapshot, OperationTypeMetrics,
};
pub use rate_limiter::{RateLimiter, RateLimiterStats};
pub use retry::{RetryConfig, RetryPolicy, RetryableError};
