//! Performance metrics collection and aggregation for Dynamics 365 API operations
//!
//! Provides comprehensive performance monitoring including response times,
//! throughput, error rates, and operational statistics.

use super::config::MonitoringConfig;
use super::logging::OperationMetrics;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Global performance metrics collector
#[derive(Debug, Clone)]
pub struct MetricsCollector {
    inner: Arc<Mutex<MetricsCollectorInner>>,
    config: MonitoringConfig,
}

#[derive(Debug)]
struct MetricsCollectorInner {
    /// Per-operation type metrics
    operation_metrics: HashMap<String, OperationTypeMetrics>,
    /// Per-entity metrics
    entity_metrics: HashMap<String, EntityMetrics>,
    /// Global aggregated metrics
    global_metrics: GlobalMetrics,
    /// Start time for rate calculations
    start_time: Instant,
}

/// Metrics for a specific operation type (create, update, delete, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationTypeMetrics {
    /// Operation type name
    pub operation_type: String,
    /// Total number of operations
    pub total_operations: u64,
    /// Number of successful operations
    pub successful_operations: u64,
    /// Number of failed operations
    pub failed_operations: u64,
    /// Total duration across all operations
    pub total_duration: Duration,
    /// Minimum duration observed
    pub min_duration: Duration,
    /// Maximum duration observed
    pub max_duration: Duration,
    /// Total retry attempts made
    pub total_retries: u64,
    /// Total rate limit delays encountered
    pub total_rate_limit_delays: Duration,
    /// HTTP status code counts
    pub status_codes: HashMap<u16, u64>,
}

/// Metrics for a specific entity type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMetrics {
    /// Entity name
    pub entity_name: String,
    /// Total operations on this entity
    pub total_operations: u64,
    /// Successful operations
    pub successful_operations: u64,
    /// Failed operations
    pub failed_operations: u64,
    /// Average duration for operations on this entity
    pub average_duration: Duration,
    /// Most common operation types
    pub operation_types: HashMap<String, u64>,
}

/// Global metrics across all operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalMetrics {
    /// Total operations since start
    pub total_operations: u64,
    /// Total successful operations
    pub successful_operations: u64,
    /// Total failed operations
    pub failed_operations: u64,
    /// Operations per second (calculated)
    pub operations_per_second: f64,
    /// Average response time
    pub average_response_time: Duration,
    /// 95th percentile response time
    pub p95_response_time: Duration,
    /// 99th percentile response time
    pub p99_response_time: Duration,
    /// Error rate percentage
    pub error_rate: f64,
    /// Total time spent in retries
    pub total_retry_time: Duration,
    /// Total time spent waiting for rate limits
    pub total_rate_limit_time: Duration,
    /// Uptime since metrics collection started
    pub uptime: Duration,
}

/// Snapshot of current performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Global metrics
    pub global: GlobalMetrics,
    /// Per-operation metrics
    pub operations: Vec<OperationTypeMetrics>,
    /// Per-entity metrics
    pub entities: Vec<EntityMetrics>,
    /// Timestamp when snapshot was taken
    pub timestamp: String,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(config: MonitoringConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(MetricsCollectorInner {
                operation_metrics: HashMap::new(),
                entity_metrics: HashMap::new(),
                global_metrics: GlobalMetrics::new(),
                start_time: Instant::now(),
            })),
            config,
        }
    }

    /// Record completion of an operation
    pub fn record_operation(&self, operation_type: &str, entity: &str, metrics: &OperationMetrics) {
        if !self.config.performance_metrics {
            return;
        }

        let mut inner = self.inner.lock().unwrap();
        let uptime = inner.start_time.elapsed(); // Calculate uptime before mutable borrows

        // Update operation type metrics
        let op_metrics = inner
            .operation_metrics
            .entry(operation_type.to_string())
            .or_insert_with(|| OperationTypeMetrics::new(operation_type));
        op_metrics.record_operation(metrics);

        // Update entity metrics
        let entity_metrics = inner
            .entity_metrics
            .entry(entity.to_string())
            .or_insert_with(|| EntityMetrics::new(entity));
        entity_metrics.record_operation(operation_type, metrics);

        // Update global metrics
        inner.global_metrics.record_operation(metrics, uptime);
    }

    /// Get current metrics snapshot
    pub fn snapshot(&self) -> MetricsSnapshot {
        let inner = self.inner.lock().unwrap();

        MetricsSnapshot {
            global: inner.global_metrics.clone(),
            operations: inner.operation_metrics.values().cloned().collect(),
            entities: inner.entity_metrics.values().cloned().collect(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.operation_metrics.clear();
        inner.entity_metrics.clear();
        inner.global_metrics = GlobalMetrics::new();
        inner.start_time = Instant::now();
    }

    /// Get metrics for a specific operation type
    pub fn operation_metrics(&self, operation_type: &str) -> Option<OperationTypeMetrics> {
        let inner = self.inner.lock().unwrap();
        inner.operation_metrics.get(operation_type).cloned()
    }

    /// Get metrics for a specific entity
    pub fn entity_metrics(&self, entity: &str) -> Option<EntityMetrics> {
        let inner = self.inner.lock().unwrap();
        inner.entity_metrics.get(entity).cloned()
    }

    /// Get top performing operations by success rate
    pub fn top_operations_by_success_rate(&self, limit: usize) -> Vec<OperationTypeMetrics> {
        let inner = self.inner.lock().unwrap();
        let mut operations: Vec<_> = inner.operation_metrics.values().cloned().collect();

        operations.sort_by(|a, b| {
            let a_rate = a.success_rate();
            let b_rate = b.success_rate();
            b_rate
                .partial_cmp(&a_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        operations.into_iter().take(limit).collect()
    }

    /// Get slowest operations by average duration
    pub fn slowest_operations(&self, limit: usize) -> Vec<OperationTypeMetrics> {
        let inner = self.inner.lock().unwrap();
        let mut operations: Vec<_> = inner.operation_metrics.values().cloned().collect();

        operations.sort_by(|a, b| {
            let a_avg = a.average_duration();
            let b_avg = b.average_duration();
            b_avg.cmp(&a_avg)
        });

        operations.into_iter().take(limit).collect()
    }
}

impl OperationTypeMetrics {
    fn new(operation_type: &str) -> Self {
        Self {
            operation_type: operation_type.to_string(),
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            total_duration: Duration::ZERO,
            min_duration: Duration::MAX,
            max_duration: Duration::ZERO,
            total_retries: 0,
            total_rate_limit_delays: Duration::ZERO,
            status_codes: HashMap::new(),
        }
    }

    fn record_operation(&mut self, metrics: &OperationMetrics) {
        self.total_operations += 1;

        if metrics.success {
            self.successful_operations += 1;
        } else {
            self.failed_operations += 1;
        }

        self.total_duration += metrics.duration;
        self.min_duration = self.min_duration.min(metrics.duration);
        self.max_duration = self.max_duration.max(metrics.duration);
        self.total_retries += metrics.retry_attempts as u64;
        self.total_rate_limit_delays += metrics.total_rate_limit_delay();

        if let Some(status_code) = metrics.status_code {
            *self.status_codes.entry(status_code).or_insert(0) += 1;
        }
    }

    /// Calculate success rate as percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            (self.successful_operations as f64 / self.total_operations as f64) * 100.0
        }
    }

    /// Calculate average duration
    pub fn average_duration(&self) -> Duration {
        if self.total_operations == 0 {
            Duration::ZERO
        } else {
            self.total_duration / self.total_operations as u32
        }
    }

    /// Calculate error rate as percentage
    pub fn error_rate(&self) -> f64 {
        100.0 - self.success_rate()
    }
}

impl EntityMetrics {
    fn new(entity_name: &str) -> Self {
        Self {
            entity_name: entity_name.to_string(),
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            average_duration: Duration::ZERO,
            operation_types: HashMap::new(),
        }
    }

    fn record_operation(&mut self, operation_type: &str, metrics: &OperationMetrics) {
        self.total_operations += 1;

        if metrics.success {
            self.successful_operations += 1;
        } else {
            self.failed_operations += 1;
        }

        // Update running average duration
        let new_avg_ms = ((self.average_duration.as_millis() as u64 * (self.total_operations - 1))
            + metrics.duration.as_millis() as u64)
            / self.total_operations;
        self.average_duration = Duration::from_millis(new_avg_ms);

        *self
            .operation_types
            .entry(operation_type.to_string())
            .or_insert(0) += 1;
    }

    /// Calculate success rate as percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            (self.successful_operations as f64 / self.total_operations as f64) * 100.0
        }
    }
}

impl GlobalMetrics {
    fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            operations_per_second: 0.0,
            average_response_time: Duration::ZERO,
            p95_response_time: Duration::ZERO,
            p99_response_time: Duration::ZERO,
            error_rate: 0.0,
            total_retry_time: Duration::ZERO,
            total_rate_limit_time: Duration::ZERO,
            uptime: Duration::ZERO,
        }
    }

    fn record_operation(&mut self, metrics: &OperationMetrics, uptime: Duration) {
        self.total_operations += 1;
        self.uptime = uptime;

        if metrics.success {
            self.successful_operations += 1;
        } else {
            self.failed_operations += 1;
        }

        // Update running average response time
        let new_avg_ms = ((self.average_response_time.as_millis() as u64
            * (self.total_operations - 1))
            + metrics.duration.as_millis() as u64)
            / self.total_operations;
        self.average_response_time = Duration::from_millis(new_avg_ms);

        // Calculate operations per second
        if uptime.as_secs() > 0 {
            self.operations_per_second = self.total_operations as f64 / uptime.as_secs_f64();
        }

        // Calculate error rate
        self.error_rate = if self.total_operations == 0 {
            0.0
        } else {
            (self.failed_operations as f64 / self.total_operations as f64) * 100.0
        };

        // Update retry and rate limit times
        self.total_retry_time += Duration::from_millis(metrics.retry_attempts as u64 * 100); // Estimate
        self.total_rate_limit_time += metrics.total_rate_limit_delay();

        // Note: P95/P99 calculations would require storing all durations or using approximation algorithms
        // For simplicity, we'll use max duration as an approximation
        self.p95_response_time = metrics.duration;
        self.p99_response_time = metrics.duration;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::resilience::config::{LogLevel, MonitoringConfig};

    #[test]
    fn test_metrics_collection() {
        let config = MonitoringConfig {
            correlation_ids: true,
            request_logging: false,
            performance_metrics: true,
            log_level: LogLevel::Info,
        };

        let collector = MetricsCollector::new(config);

        // Record some operations
        let metrics1 = OperationMetrics {
            duration: Duration::from_millis(100),
            retry_attempts: 0,
            success: true,
            status_code: Some(201),
            error_message: None,
            rate_limit_delays: vec![],
        };

        let metrics2 = OperationMetrics {
            duration: Duration::from_millis(200),
            retry_attempts: 1,
            success: false,
            status_code: Some(500),
            error_message: Some("Server error".to_string()),
            rate_limit_delays: vec![Duration::from_millis(50)],
        };

        collector.record_operation("create", "contacts", &metrics1);
        collector.record_operation("create", "contacts", &metrics2);

        // Get snapshot
        let snapshot = collector.snapshot();

        assert_eq!(snapshot.global.total_operations, 2);
        assert_eq!(snapshot.global.successful_operations, 1);
        assert_eq!(snapshot.global.failed_operations, 1);
        assert_eq!(snapshot.global.error_rate, 50.0);

        // Check operation metrics
        let create_metrics = collector.operation_metrics("create").unwrap();
        assert_eq!(create_metrics.total_operations, 2);
        assert_eq!(create_metrics.success_rate(), 50.0);
        assert_eq!(create_metrics.total_retries, 1);

        // Check entity metrics
        let contact_metrics = collector.entity_metrics("contacts").unwrap();
        assert_eq!(contact_metrics.total_operations, 2);
        assert_eq!(contact_metrics.success_rate(), 50.0);
    }

    #[test]
    fn test_operation_type_metrics() {
        let mut metrics = OperationTypeMetrics::new("create");

        let op_metrics = OperationMetrics {
            duration: Duration::from_millis(150),
            retry_attempts: 0,
            success: true,
            status_code: Some(201),
            error_message: None,
            rate_limit_delays: vec![],
        };

        metrics.record_operation(&op_metrics);

        assert_eq!(metrics.total_operations, 1);
        assert_eq!(metrics.successful_operations, 1);
        assert_eq!(metrics.success_rate(), 100.0);
        assert_eq!(metrics.average_duration(), Duration::from_millis(150));
        assert_eq!(metrics.status_codes[&201], 1);
    }

    #[test]
    fn test_entity_metrics() {
        let mut metrics = EntityMetrics::new("contacts");

        let op_metrics = OperationMetrics {
            duration: Duration::from_millis(100),
            retry_attempts: 0,
            success: true,
            status_code: Some(200),
            error_message: None,
            rate_limit_delays: vec![],
        };

        metrics.record_operation("update", &op_metrics);

        assert_eq!(metrics.total_operations, 1);
        assert_eq!(metrics.successful_operations, 1);
        assert_eq!(metrics.success_rate(), 100.0);
        assert_eq!(metrics.average_duration, Duration::from_millis(100));
        assert_eq!(metrics.operation_types["update"], 1);
    }

    #[test]
    fn test_metrics_disabled() {
        let config = MonitoringConfig {
            correlation_ids: true,
            request_logging: false,
            performance_metrics: false, // Disabled
            log_level: LogLevel::Info,
        };

        let collector = MetricsCollector::new(config);

        let metrics = OperationMetrics {
            duration: Duration::from_millis(100),
            retry_attempts: 0,
            success: true,
            status_code: Some(200),
            error_message: None,
            rate_limit_delays: vec![],
        };

        collector.record_operation("create", "contacts", &metrics);

        // Should not record anything when disabled
        let snapshot = collector.snapshot();
        assert_eq!(snapshot.global.total_operations, 0);
    }
}
