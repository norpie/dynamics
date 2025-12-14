//! API-related options registration

use crate::config::options::{OptionDefBuilder, OptionsRegistry};
use anyhow::Result;

/// Register all API-related options
pub fn register(registry: &OptionsRegistry) -> Result<()> {
    // Retry options
    registry.register(
        OptionDefBuilder::new("api", "retry.enabled")
            .display_name("Enable Retries")
            .description("Automatically retry failed API requests with exponential backoff")
            .bool_type(true)
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("api", "retry.max_attempts")
            .display_name("Max Retry Attempts")
            .description("Maximum number of retry attempts for failed requests (1-10)")
            .uint_type(3, Some(1), Some(10))
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("api", "retry.base_delay_ms")
            .display_name("Base Retry Delay (ms)")
            .description("Initial delay in milliseconds before first retry (100-10000)")
            .uint_type(1000, Some(100), Some(10000))
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("api", "retry.max_delay_ms")
            .display_name("Max Retry Delay (ms)")
            .description("Maximum delay in milliseconds between retries (1000-60000)")
            .uint_type(30000, Some(1000), Some(60000))
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("api", "retry.backoff_multiplier")
            .display_name("Backoff Multiplier")
            .description("Multiplier for exponential backoff (1.0-10.0)")
            .float_type(2.0, Some(1.0), Some(10.0))
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("api", "retry.jitter")
            .display_name("Enable Jitter")
            .description("Add random jitter to retry delays to avoid thundering herd")
            .bool_type(true)
            .build()?
    )?;

    // Rate limiting options
    registry.register(
        OptionDefBuilder::new("api", "rate_limit.enabled")
            .display_name("Enable Rate Limiting")
            .description("Apply rate limiting to prevent exceeding API quotas")
            .bool_type(true)
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("api", "rate_limit.requests_per_minute")
            .display_name("Requests Per Minute")
            .description("Maximum number of requests per minute (1-1200)")
            .uint_type(600, Some(1), Some(1200))
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("api", "rate_limit.burst_capacity")
            .display_name("Burst Capacity")
            .description("Number of requests that can burst above the rate limit (1-100)")
            .uint_type(30, Some(1), Some(100))
            .build()?
    )?;

    // Concurrency options
    registry.register(
        OptionDefBuilder::new("api", "concurrency.enabled")
            .display_name("Enable Concurrency Limiting")
            .description("Limit the number of concurrent API requests to prevent exceeding Dataverse limits")
            .bool_type(true)
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("api", "concurrency.max_concurrent_requests")
            .display_name("Max Concurrent Requests")
            .description("Maximum number of concurrent HTTP requests to Dataverse (1-52)")
            .uint_type(20, Some(1), Some(52))
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("api", "concurrency.max_queue_items")
            .display_name("Max Queue Items")
            .description("Maximum number of queue items that can run concurrently (1-50)")
            .uint_type(10, Some(1), Some(50))
            .build()?
    )?;

    // Monitoring options
    registry.register(
        OptionDefBuilder::new("api", "monitoring.correlation_ids")
            .display_name("Correlation IDs")
            .description("Include correlation IDs in API requests for tracing")
            .bool_type(true)
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("api", "monitoring.request_logging")
            .display_name("Request Logging")
            .description("Log detailed information about API requests and responses")
            .bool_type(true)
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("api", "monitoring.performance_metrics")
            .display_name("Performance Metrics")
            .description("Collect and log performance metrics for API operations")
            .bool_type(true)
            .build()?
    )?;

    registry.register(
        OptionDefBuilder::new("api", "monitoring.log_level")
            .display_name("Log Level")
            .description("Minimum severity level for API operation logs")
            .enum_type(
                vec!["error", "warn", "info", "debug", "trace"],
                "info"
            )
            .build()?
    )?;

    log::info!("Registered {} API options", 16);
    Ok(())
}
