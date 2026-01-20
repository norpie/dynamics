//! Structured logging with correlation tracking for Dynamics 365 API operations
//!
//! Provides structured logging capabilities that include correlation IDs,
//! performance metrics, and request/response tracking for debugging and monitoring.

use super::config::{LogLevel, MonitoringConfig};
use log::{debug, error, info, trace, warn};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Structured logger for API operations with correlation tracking
#[derive(Debug, Clone)]
pub struct ApiLogger {
    config: MonitoringConfig,
}

/// Context for a single API operation with correlation tracking
#[derive(Debug, Clone)]
pub struct OperationContext {
    /// Unique correlation ID for this operation
    pub correlation_id: String,
    /// Operation type (create, update, delete, etc.)
    pub operation_type: String,
    /// Entity being operated on
    pub entity: String,
    /// Additional metadata for the operation
    pub metadata: HashMap<String, Value>,
    /// Start time for performance tracking
    pub start_time: Instant,
}

/// Performance metrics for an API operation
#[derive(Debug, Clone)]
pub struct OperationMetrics {
    /// Total duration of the operation
    pub duration: Duration,
    /// Number of retry attempts made
    pub retry_attempts: u32,
    /// Whether the operation succeeded
    pub success: bool,
    /// HTTP status code returned
    pub status_code: Option<u16>,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Rate limiting delays encountered
    pub rate_limit_delays: Vec<Duration>,
}

impl ApiLogger {
    /// Create a new API logger with the given configuration
    pub fn new(config: MonitoringConfig) -> Self {
        Self { config }
    }

    /// Start tracking a new operation
    pub fn start_operation(
        &self,
        operation_type: &str,
        entity: &str,
        correlation_id: &str,
    ) -> OperationContext {
        let context = OperationContext {
            correlation_id: correlation_id.to_string(),
            operation_type: operation_type.to_string(),
            entity: entity.to_string(),
            metadata: HashMap::new(),
            start_time: Instant::now(),
        };

        if self.config.request_logging && self.should_log(&LogLevel::Info) {
            let log_data = json!({
                "event": "operation_started",
                "correlation_id": context.correlation_id,
                "operation_type": context.operation_type,
                "entity": context.entity,
                "timestamp": chrono::Utc::now().to_rfc3339()
            });

            info!("API Operation Started: {}", log_data);
        }

        context
    }

    /// Log HTTP request details
    pub fn log_request(
        &self,
        context: &OperationContext,
        method: &str,
        url: &str,
        headers: &HashMap<String, String>,
    ) {
        if !self.config.request_logging || !self.should_log(&LogLevel::Debug) {
            return;
        }

        let log_data = json!({
            "event": "http_request",
            "correlation_id": context.correlation_id,
            "operation_type": context.operation_type,
            "entity": context.entity,
            "method": method,
            "url": url,
            "headers": self.sanitize_headers(headers),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        debug!("HTTP Request: {}", log_data);
    }

    /// Log HTTP response details
    pub fn log_response(
        &self,
        context: &OperationContext,
        status_code: u16,
        headers: &HashMap<String, String>,
        duration: Duration,
    ) {
        if !self.config.request_logging || !self.should_log(&LogLevel::Debug) {
            return;
        }

        let log_data = json!({
            "event": "http_response",
            "correlation_id": context.correlation_id,
            "operation_type": context.operation_type,
            "entity": context.entity,
            "status_code": status_code,
            "duration_ms": duration.as_millis(),
            "headers": self.sanitize_headers(headers),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        if status_code >= 400 {
            warn!("HTTP Response (Error): {}", log_data);
        } else {
            debug!("HTTP Response: {}", log_data);
        }
    }

    /// Log retry attempt
    pub fn log_retry(
        &self,
        context: &OperationContext,
        attempt: u32,
        error: &str,
        delay: Duration,
    ) {
        if !self.should_log(&LogLevel::Warn) {
            return;
        }

        let log_data = json!({
            "event": "retry_attempt",
            "correlation_id": context.correlation_id,
            "operation_type": context.operation_type,
            "entity": context.entity,
            "attempt": attempt,
            "error": error,
            "delay_ms": delay.as_millis(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        warn!("Retry Attempt: {}", log_data);
    }

    /// Log rate limiting event
    pub fn log_rate_limit(
        &self,
        context: &OperationContext,
        delay: Duration,
        tokens_available: f64,
    ) {
        if !self.should_log(&LogLevel::Debug) {
            return;
        }

        let log_data = json!({
            "event": "rate_limited",
            "correlation_id": context.correlation_id,
            "operation_type": context.operation_type,
            "entity": context.entity,
            "delay_ms": delay.as_millis(),
            "tokens_available": tokens_available,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        debug!("Rate Limited: {}", log_data);
    }

    /// Complete an operation and log metrics
    pub fn complete_operation(&self, context: &OperationContext, metrics: &OperationMetrics) {
        if self.config.performance_metrics && self.should_log(&LogLevel::Info) {
            let log_data = json!({
                "event": "operation_completed",
                "correlation_id": context.correlation_id,
                "operation_type": context.operation_type,
                "entity": context.entity,
                "duration_ms": metrics.duration.as_millis(),
                "retry_attempts": metrics.retry_attempts,
                "success": metrics.success,
                "status_code": metrics.status_code,
                "error_message": metrics.error_message,
                "rate_limit_delays_ms": metrics.rate_limit_delays.iter().map(|d| d.as_millis()).collect::<Vec<_>>(),
                "timestamp": chrono::Utc::now().to_rfc3339()
            });

            if metrics.success {
                info!("API Operation Completed: {}", log_data);
            } else {
                error!("API Operation Failed: {}", log_data);
            }
        }
    }

    /// Log batch operation details
    pub fn log_batch_operation(
        &self,
        correlation_id: &str,
        operation_count: usize,
        duration: Duration,
        success_count: usize,
    ) {
        if !self.config.performance_metrics || !self.should_log(&LogLevel::Info) {
            return;
        }

        let log_data = json!({
            "event": "batch_operation_completed",
            "correlation_id": correlation_id,
            "operation_type": "batch",
            "operation_count": operation_count,
            "success_count": success_count,
            "failure_count": operation_count - success_count,
            "duration_ms": duration.as_millis(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        info!("Batch Operation Completed: {}", log_data);
    }

    /// Add metadata to an operation context
    pub fn add_metadata(&self, context: &mut OperationContext, key: &str, value: Value) {
        if self.should_log(&LogLevel::Trace) {
            trace!(
                "Added metadata to operation {}: {} = {}",
                context.correlation_id, key, value
            );
        }

        context.metadata.insert(key.to_string(), value);
    }

    /// Log performance warning for slow operations
    pub fn log_performance_warning(
        &self,
        context: &OperationContext,
        duration: Duration,
        threshold: Duration,
    ) {
        if !self.config.performance_metrics || !self.should_log(&LogLevel::Warn) {
            return;
        }

        let log_data = json!({
            "event": "performance_warning",
            "correlation_id": context.correlation_id,
            "operation_type": context.operation_type,
            "entity": context.entity,
            "duration_ms": duration.as_millis(),
            "threshold_ms": threshold.as_millis(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        warn!("Slow Operation Detected: {}", log_data);
    }

    /// Check if we should log at the given level
    fn should_log(&self, level: &LogLevel) -> bool {
        match (&self.config.log_level, level) {
            (LogLevel::Error, LogLevel::Error) => true,
            (LogLevel::Warn, LogLevel::Error | LogLevel::Warn) => true,
            (LogLevel::Info, LogLevel::Error | LogLevel::Warn | LogLevel::Info) => true,
            (
                LogLevel::Debug,
                LogLevel::Error | LogLevel::Warn | LogLevel::Info | LogLevel::Debug,
            ) => true,
            (LogLevel::Trace, _) => true,
            _ => false,
        }
    }

    /// Sanitize headers to remove sensitive information
    fn sanitize_headers(&self, headers: &HashMap<String, String>) -> HashMap<String, String> {
        let mut sanitized = HashMap::new();

        for (key, value) in headers {
            let key_lower = key.to_lowercase();
            if key_lower.contains("authorization")
                || key_lower.contains("token")
                || key_lower.contains("key")
            {
                sanitized.insert(key.clone(), "[REDACTED]".to_string());
            } else {
                sanitized.insert(key.clone(), value.clone());
            }
        }

        sanitized
    }
}

impl OperationContext {
    /// Calculate elapsed time since operation started
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Create metrics from this context
    pub fn create_metrics(
        &self,
        success: bool,
        status_code: Option<u16>,
        error_message: Option<String>,
    ) -> OperationMetrics {
        OperationMetrics {
            duration: self.elapsed(),
            retry_attempts: 0,
            success,
            status_code,
            error_message,
            rate_limit_delays: Vec::new(),
        }
    }
}

impl OperationMetrics {
    /// Add a retry attempt to the metrics
    pub fn add_retry(&mut self) {
        self.retry_attempts += 1;
    }

    /// Add a rate limit delay to the metrics
    pub fn add_rate_limit_delay(&mut self, delay: Duration) {
        self.rate_limit_delays.push(delay);
    }

    /// Calculate total time spent waiting for rate limits
    pub fn total_rate_limit_delay(&self) -> Duration {
        self.rate_limit_delays.iter().sum()
    }

    /// Check if this operation was slow (exceeded typical thresholds)
    pub fn is_slow(&self, threshold: Duration) -> bool {
        self.duration > threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_operation_context_creation() {
        let config = MonitoringConfig {
            correlation_ids: true,
            request_logging: true,
            performance_metrics: true,
            log_level: LogLevel::Debug,
        };

        let logger = ApiLogger::new(config);
        let context = logger.start_operation("create", "contacts", "test-123");

        assert_eq!(context.correlation_id, "test-123");
        assert_eq!(context.operation_type, "create");
        assert_eq!(context.entity, "contacts");
        assert!(context.metadata.is_empty());
    }

    #[test]
    fn test_operation_metrics() {
        let context = OperationContext {
            correlation_id: "test-123".to_string(),
            operation_type: "create".to_string(),
            entity: "contacts".to_string(),
            metadata: HashMap::new(),
            start_time: Instant::now(),
        };

        let mut metrics = context.create_metrics(true, Some(201), None);
        assert_eq!(metrics.retry_attempts, 0);
        assert!(metrics.success);
        assert_eq!(metrics.status_code, Some(201));

        metrics.add_retry();
        metrics.add_rate_limit_delay(Duration::from_millis(100));

        assert_eq!(metrics.retry_attempts, 1);
        assert_eq!(metrics.rate_limit_delays.len(), 1);
        assert_eq!(metrics.total_rate_limit_delay(), Duration::from_millis(100));
    }

    #[test]
    fn test_header_sanitization() {
        let config = MonitoringConfig {
            correlation_ids: true,
            request_logging: true,
            performance_metrics: true,
            log_level: LogLevel::Debug,
        };

        let logger = ApiLogger::new(config);
        let mut headers = HashMap::new();
        headers.insert(
            "Authorization".to_string(),
            "Bearer secret-token".to_string(),
        );
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("X-API-Key".to_string(), "secret-key".to_string());

        let sanitized = logger.sanitize_headers(&headers);

        assert_eq!(
            sanitized.get("Authorization"),
            Some(&"[REDACTED]".to_string())
        );
        assert_eq!(
            sanitized.get("Content-Type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(sanitized.get("X-API-Key"), Some(&"[REDACTED]".to_string()));
    }

    #[test]
    fn test_log_level_filtering() {
        let config = MonitoringConfig {
            correlation_ids: true,
            request_logging: true,
            performance_metrics: true,
            log_level: LogLevel::Warn,
        };

        let logger = ApiLogger::new(config);

        assert!(logger.should_log(&LogLevel::Error));
        assert!(logger.should_log(&LogLevel::Warn));
        assert!(!logger.should_log(&LogLevel::Info));
        assert!(!logger.should_log(&LogLevel::Debug));
        assert!(!logger.should_log(&LogLevel::Trace));
    }

    #[test]
    fn test_performance_threshold() {
        let metrics = OperationMetrics {
            duration: Duration::from_millis(5000),
            retry_attempts: 1,
            success: true,
            status_code: Some(200),
            error_message: None,
            rate_limit_delays: vec![Duration::from_millis(100)],
        };

        assert!(metrics.is_slow(Duration::from_millis(3000)));
        assert!(!metrics.is_slow(Duration::from_millis(10000)));
    }
}
