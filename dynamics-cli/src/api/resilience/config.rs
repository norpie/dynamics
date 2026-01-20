//! Resilience configuration with builder pattern
//!
//! Provides a unified configuration for retry policies, rate limiting,
//! and monitoring features with sane defaults.

use super::retry::RetryConfig;
use std::time::Duration;

/// Global resilience configuration for API operations
#[derive(Debug, Clone)]
pub struct ResilienceConfig {
    pub retry: RetryConfig,
    pub rate_limit: RateLimitConfig,
    pub concurrency: ConcurrencyConfig,
    pub monitoring: MonitoringConfig,
    pub bypass: BypassConfig,
}

/// Concurrency limiting configuration
#[derive(Debug, Clone)]
pub struct ConcurrencyConfig {
    /// Maximum concurrent HTTP requests to the API
    pub max_concurrent_requests: usize,
    /// Maximum queue items that can run concurrently
    pub max_queue_items: usize,
    /// Whether concurrency limiting is enabled
    pub enabled: bool,
}

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub burst_capacity: u32,
    pub enabled: bool,
}

/// Monitoring and logging configuration
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    pub correlation_ids: bool,
    pub request_logging: bool,
    pub performance_metrics: bool,
    pub log_level: LogLevel,
}

#[derive(Debug, Clone)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Configuration for bypassing Dynamics 365 custom business logic
///
/// These options allow bypassing plugins, workflows, and Power Automate flows
/// during API operations. Useful for data migrations and bulk operations.
///
/// Note: Most bypass options require the user to have the `prvBypassCustomBusinessLogic`
/// privilege in Dynamics 365. Only `power_automate_flows` bypass requires no special privilege.
#[derive(Debug, Clone)]
pub struct BypassConfig {
    /// Bypass synchronous custom plugins and real-time workflows
    pub custom_sync: bool,
    /// Bypass asynchronous custom plugins and workflows (not Power Automate flows)
    pub custom_async: bool,
    /// Bypass specific plugin steps by GUID (max 3 by default, configurable up to 10)
    pub step_ids: Vec<String>,
    /// Bypass Power Automate flows triggered by Dataverse events (no privilege required)
    pub power_automate_flows: bool,
}

impl Default for BypassConfig {
    fn default() -> Self {
        Self {
            custom_sync: false,
            custom_async: false,
            step_ids: Vec::new(),
            power_automate_flows: false,
        }
    }
}

impl BypassConfig {
    /// Check if any bypass options are enabled
    pub fn is_enabled(&self) -> bool {
        self.custom_sync
            || self.custom_async
            || !self.step_ids.is_empty()
            || self.power_automate_flows
    }

    /// Create a config that bypasses all custom logic (plugins, workflows, flows)
    pub fn all() -> Self {
        Self {
            custom_sync: true,
            custom_async: true,
            step_ids: Vec::new(),
            power_automate_flows: true,
        }
    }
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            retry: RetryConfig::default(),
            rate_limit: RateLimitConfig::default(),
            concurrency: ConcurrencyConfig::default(),
            monitoring: MonitoringConfig::default(),
            bypass: BypassConfig::default(),
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 600, // Conservative (Dataverse allows 1200/min)
            burst_capacity: 30,       // Allow moderate bursts
            enabled: true,
        }
    }
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 20, // Conservative (Dataverse allows 52)
            max_queue_items: 10,         // Queue items running concurrently
            enabled: true,
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            correlation_ids: true,
            request_logging: true,
            performance_metrics: true,
            log_level: LogLevel::Info,
        }
    }
}

impl ResilienceConfig {
    /// Create a new builder for ResilienceConfig
    pub fn builder() -> ResilienceConfigBuilder {
        ResilienceConfigBuilder::new()
    }

    /// Conservative config for production environments
    pub fn conservative() -> Self {
        Self {
            retry: RetryConfig::conservative(),
            rate_limit: RateLimitConfig {
                requests_per_minute: 300, // Very conservative
                burst_capacity: 15,
                enabled: true,
            },
            concurrency: ConcurrencyConfig {
                max_concurrent_requests: 10,
                max_queue_items: 5,
                enabled: true,
            },
            monitoring: MonitoringConfig {
                correlation_ids: true,
                request_logging: true,
                performance_metrics: true,
                log_level: LogLevel::Warn, // Less verbose in production
            },
            bypass: BypassConfig::default(),
        }
    }

    /// Aggressive config for development/testing
    pub fn development() -> Self {
        Self {
            retry: RetryConfig::aggressive(),
            rate_limit: RateLimitConfig {
                requests_per_minute: 1000, // Higher limits for dev
                burst_capacity: 50,
                enabled: false, // Often disabled in dev
            },
            concurrency: ConcurrencyConfig {
                max_concurrent_requests: 40,
                max_queue_items: 20,
                enabled: false, // Often disabled in dev
            },
            monitoring: MonitoringConfig {
                correlation_ids: true,
                request_logging: true,
                performance_metrics: true,
                log_level: LogLevel::Debug, // More verbose for debugging
            },
            bypass: BypassConfig::default(),
        }
    }

    /// Disable all resilience features (for testing)
    pub fn disabled() -> Self {
        Self {
            retry: RetryConfig {
                max_attempts: 1, // No retries
                base_delay: Duration::from_millis(0),
                max_delay: Duration::from_millis(0),
                backoff_multiplier: 1.0,
                jitter: false,
            },
            rate_limit: RateLimitConfig {
                requests_per_minute: u32::MAX,
                burst_capacity: u32::MAX,
                enabled: false,
            },
            concurrency: ConcurrencyConfig {
                max_concurrent_requests: usize::MAX,
                max_queue_items: usize::MAX,
                enabled: false,
            },
            monitoring: MonitoringConfig {
                correlation_ids: false,
                request_logging: false,
                performance_metrics: false,
                log_level: LogLevel::Error,
            },
            bypass: BypassConfig::default(),
        }
    }

    /// Config optimized for data migrations - bypasses all custom business logic
    ///
    /// Use this when performing bulk data operations where you want to skip
    /// plugins, workflows, and Power Automate flows.
    ///
    /// Note: Requires `prvBypassCustomBusinessLogic` privilege for plugin/workflow bypass.
    pub fn migration() -> Self {
        Self {
            retry: RetryConfig::conservative(),
            rate_limit: RateLimitConfig {
                requests_per_minute: 600,
                burst_capacity: 30,
                enabled: true,
            },
            concurrency: ConcurrencyConfig {
                max_concurrent_requests: 20,
                max_queue_items: 10,
                enabled: true,
            },
            monitoring: MonitoringConfig {
                correlation_ids: true,
                request_logging: true,
                performance_metrics: true,
                log_level: LogLevel::Info,
            },
            bypass: BypassConfig::all(),
        }
    }

    /// Load resilience config from the options system
    pub async fn load_from_options() -> anyhow::Result<Self> {
        let config = crate::global_config();

        // Load retry options
        let retry_enabled = config
            .options
            .get_bool("api.retry.enabled")
            .await
            .unwrap_or(true);
        let max_attempts = config
            .options
            .get_uint("api.retry.max_attempts")
            .await
            .unwrap_or(3) as u32;
        let base_delay_ms = config
            .options
            .get_uint("api.retry.base_delay_ms")
            .await
            .unwrap_or(500);
        let max_delay_ms = config
            .options
            .get_uint("api.retry.max_delay_ms")
            .await
            .unwrap_or(30000);
        let backoff_multiplier = config
            .options
            .get_float("api.retry.backoff_multiplier")
            .await
            .unwrap_or(2.0);
        let jitter = config
            .options
            .get_bool("api.retry.jitter")
            .await
            .unwrap_or(true);

        // Load rate limit options
        let rate_limit_enabled = config
            .options
            .get_bool("api.rate_limit.enabled")
            .await
            .unwrap_or(true);
        let requests_per_minute = config
            .options
            .get_uint("api.rate_limit.requests_per_minute")
            .await
            .unwrap_or(600) as u32;
        let burst_capacity = config
            .options
            .get_uint("api.rate_limit.burst_capacity")
            .await
            .unwrap_or(30) as u32;

        // Load concurrency options
        let concurrency_enabled = config
            .options
            .get_bool("api.concurrency.enabled")
            .await
            .unwrap_or(true);
        let max_concurrent_requests = config
            .options
            .get_uint("api.concurrency.max_concurrent_requests")
            .await
            .unwrap_or(20) as usize;
        let max_queue_items = config
            .options
            .get_uint("api.concurrency.max_queue_items")
            .await
            .unwrap_or(10) as usize;

        // Load monitoring options
        let correlation_ids = config
            .options
            .get_bool("api.monitoring.correlation_ids")
            .await
            .unwrap_or(true);
        let request_logging = config
            .options
            .get_bool("api.monitoring.request_logging")
            .await
            .unwrap_or(true);
        let performance_metrics = config
            .options
            .get_bool("api.monitoring.performance_metrics")
            .await
            .unwrap_or(true);
        let log_level_str = config
            .options
            .get_string("api.monitoring.log_level")
            .await
            .unwrap_or_else(|_| "info".to_string());

        let log_level = match log_level_str.as_str() {
            "error" => LogLevel::Error,
            "warn" => LogLevel::Warn,
            "info" => LogLevel::Info,
            "debug" => LogLevel::Debug,
            "trace" => LogLevel::Trace,
            _ => LogLevel::Info,
        };

        // Load bypass options
        let bypass_custom_sync = config
            .options
            .get_bool("api.bypass.custom_sync")
            .await
            .unwrap_or(false);
        let bypass_custom_async = config
            .options
            .get_bool("api.bypass.custom_async")
            .await
            .unwrap_or(false);
        let bypass_power_automate = config
            .options
            .get_bool("api.bypass.power_automate")
            .await
            .unwrap_or(false);
        let bypass_step_ids_str = config
            .options
            .get_string("api.bypass.step_ids")
            .await
            .unwrap_or_default();
        let bypass_step_ids: Vec<String> = if bypass_step_ids_str.is_empty() {
            Vec::new()
        } else {
            bypass_step_ids_str
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        };

        Ok(Self {
            retry: RetryConfig {
                max_attempts: if retry_enabled { max_attempts } else { 1 },
                base_delay: Duration::from_millis(base_delay_ms),
                max_delay: Duration::from_millis(max_delay_ms),
                backoff_multiplier,
                jitter,
            },
            rate_limit: RateLimitConfig {
                requests_per_minute,
                burst_capacity,
                enabled: rate_limit_enabled,
            },
            concurrency: ConcurrencyConfig {
                max_concurrent_requests,
                max_queue_items,
                enabled: concurrency_enabled,
            },
            monitoring: MonitoringConfig {
                correlation_ids,
                request_logging,
                performance_metrics,
                log_level,
            },
            bypass: BypassConfig {
                custom_sync: bypass_custom_sync,
                custom_async: bypass_custom_async,
                step_ids: bypass_step_ids,
                power_automate_flows: bypass_power_automate,
            },
        })
    }
}

/// Builder for ResilienceConfig
#[derive(Debug)]
pub struct ResilienceConfigBuilder {
    config: ResilienceConfig,
}

impl ResilienceConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: ResilienceConfig::default(),
        }
    }

    /// Configure retry behavior
    pub fn retry_config(mut self, retry: RetryConfig) -> Self {
        self.config.retry = retry;
        self
    }

    /// Set max retry attempts
    pub fn max_retries(mut self, attempts: u32) -> Self {
        self.config.retry.max_attempts = attempts;
        self
    }

    /// Configure rate limiting
    pub fn rate_limit_config(mut self, rate_limit: RateLimitConfig) -> Self {
        self.config.rate_limit = rate_limit;
        self
    }

    /// Set requests per minute limit
    pub fn requests_per_minute(mut self, rpm: u32) -> Self {
        self.config.rate_limit.requests_per_minute = rpm;
        self
    }

    /// Enable/disable rate limiting
    pub fn enable_rate_limiting(mut self, enabled: bool) -> Self {
        self.config.rate_limit.enabled = enabled;
        self
    }

    /// Configure concurrency limiting
    pub fn concurrency_config(mut self, concurrency: ConcurrencyConfig) -> Self {
        self.config.concurrency = concurrency;
        self
    }

    /// Set max concurrent requests
    pub fn max_concurrent_requests(mut self, max: usize) -> Self {
        self.config.concurrency.max_concurrent_requests = max;
        self
    }

    /// Set max queue items
    pub fn max_queue_items(mut self, max: usize) -> Self {
        self.config.concurrency.max_queue_items = max;
        self
    }

    /// Enable/disable concurrency limiting
    pub fn enable_concurrency_limiting(mut self, enabled: bool) -> Self {
        self.config.concurrency.enabled = enabled;
        self
    }

    /// Configure monitoring
    pub fn monitoring_config(mut self, monitoring: MonitoringConfig) -> Self {
        self.config.monitoring = monitoring;
        self
    }

    /// Enable/disable correlation IDs
    pub fn correlation_ids(mut self, enabled: bool) -> Self {
        self.config.monitoring.correlation_ids = enabled;
        self
    }

    /// Enable/disable request logging
    pub fn request_logging(mut self, enabled: bool) -> Self {
        self.config.monitoring.request_logging = enabled;
        self
    }

    /// Enable/disable performance metrics
    pub fn performance_metrics(mut self, enabled: bool) -> Self {
        self.config.monitoring.performance_metrics = enabled;
        self
    }

    /// Set logging level
    pub fn log_level(mut self, level: LogLevel) -> Self {
        self.config.monitoring.log_level = level;
        self
    }

    /// Configure bypass settings
    pub fn bypass_config(mut self, bypass: BypassConfig) -> Self {
        self.config.bypass = bypass;
        self
    }

    /// Enable/disable bypassing synchronous custom plugins and real-time workflows
    pub fn bypass_custom_sync(mut self, enabled: bool) -> Self {
        self.config.bypass.custom_sync = enabled;
        self
    }

    /// Enable/disable bypassing asynchronous custom plugins and workflows
    pub fn bypass_custom_async(mut self, enabled: bool) -> Self {
        self.config.bypass.custom_async = enabled;
        self
    }

    /// Set specific plugin step IDs to bypass
    pub fn bypass_step_ids(mut self, ids: Vec<String>) -> Self {
        self.config.bypass.step_ids = ids;
        self
    }

    /// Enable/disable bypassing Power Automate flows
    pub fn bypass_power_automate(mut self, enabled: bool) -> Self {
        self.config.bypass.power_automate_flows = enabled;
        self
    }

    /// Enable bypassing all custom business logic (sync, async, and Power Automate)
    pub fn bypass_all_custom_logic(mut self) -> Self {
        self.config.bypass = BypassConfig::all();
        self
    }

    /// Build the final configuration
    pub fn build(self) -> ResilienceConfig {
        self.config
    }
}

impl Default for ResilienceConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ResilienceConfig::default();

        assert_eq!(config.retry.max_attempts, 3);
        assert_eq!(config.rate_limit.requests_per_minute, 600);
        assert!(config.rate_limit.enabled);
        assert_eq!(config.concurrency.max_concurrent_requests, 20);
        assert_eq!(config.concurrency.max_queue_items, 10);
        assert!(config.concurrency.enabled);
        assert!(config.monitoring.correlation_ids);
        assert!(config.monitoring.request_logging);
    }

    #[test]
    fn test_conservative_config() {
        let config = ResilienceConfig::conservative();

        assert_eq!(config.retry.max_attempts, 2);
        assert_eq!(config.rate_limit.requests_per_minute, 300);
        assert!(config.rate_limit.enabled);
        assert_eq!(config.concurrency.max_concurrent_requests, 10);
        assert_eq!(config.concurrency.max_queue_items, 5);
        assert!(config.concurrency.enabled);
    }

    #[test]
    fn test_development_config() {
        let config = ResilienceConfig::development();

        assert_eq!(config.retry.max_attempts, 5);
        assert_eq!(config.rate_limit.requests_per_minute, 1000);
        assert!(!config.rate_limit.enabled); // Disabled in dev
        assert_eq!(config.concurrency.max_concurrent_requests, 40);
        assert!(!config.concurrency.enabled); // Disabled in dev
    }

    #[test]
    fn test_disabled_config() {
        let config = ResilienceConfig::disabled();

        assert_eq!(config.retry.max_attempts, 1);
        assert!(!config.rate_limit.enabled);
        assert!(!config.concurrency.enabled);
        assert!(!config.monitoring.correlation_ids);
        assert!(!config.monitoring.request_logging);
    }

    #[test]
    fn test_builder_pattern() {
        let config = ResilienceConfig::builder()
            .max_retries(5)
            .requests_per_minute(120)
            .enable_rate_limiting(false)
            .max_concurrent_requests(30)
            .max_queue_items(15)
            .enable_concurrency_limiting(true)
            .correlation_ids(true)
            .log_level(LogLevel::Debug)
            .build();

        assert_eq!(config.retry.max_attempts, 5);
        assert_eq!(config.rate_limit.requests_per_minute, 120);
        assert!(!config.rate_limit.enabled);
        assert_eq!(config.concurrency.max_concurrent_requests, 30);
        assert_eq!(config.concurrency.max_queue_items, 15);
        assert!(config.concurrency.enabled);
        assert!(config.monitoring.correlation_ids);
    }
}
