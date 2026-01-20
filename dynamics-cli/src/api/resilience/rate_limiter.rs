//! Token bucket rate limiter implementation
//!
//! Provides a global rate limiter per client instance that implements
//! the token bucket algorithm for controlling request rates to Dynamics 365.

use super::config::RateLimitConfig;
use log::{debug, warn};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Token bucket rate limiter for controlling API request rates
#[derive(Debug, Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<RateLimiterInner>>,
    config: RateLimitConfig,
}

#[derive(Debug)]
struct RateLimiterInner {
    tokens: f64,
    last_refill: Instant,
    requests_made: u64,
    requests_rejected: u64,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration
    pub fn new(config: RateLimitConfig) -> Self {
        let initial_tokens = if config.enabled {
            config.burst_capacity as f64
        } else {
            f64::MAX // Unlimited when disabled
        };

        Self {
            inner: Arc::new(Mutex::new(RateLimiterInner {
                tokens: initial_tokens,
                last_refill: Instant::now(),
                requests_made: 0,
                requests_rejected: 0,
            })),
            config,
        }
    }

    /// Attempt to acquire a token for making a request
    /// Always succeeds, but may wait if rate limited
    pub async fn acquire(&self) -> bool {
        if !self.config.enabled {
            return true;
        }

        loop {
            let should_wait = {
                let mut inner = self.inner.lock().unwrap();

                // Refill tokens based on time passed
                self.refill_tokens(&mut inner);

                if inner.tokens >= 1.0 {
                    inner.tokens -= 1.0;
                    inner.requests_made += 1;
                    debug!(
                        "Rate limiter: Request approved, {} tokens remaining",
                        inner.tokens
                    );
                    false // Don't wait
                } else {
                    inner.requests_rejected += 1;
                    debug!(
                        "Rate limiter: Request needs to wait, {} tokens available",
                        inner.tokens
                    );
                    true // Need to wait
                }
            };

            if !should_wait {
                return true;
            }

            // Calculate wait time until next token is available
            let wait_duration = self.calculate_wait_time();
            debug!("Rate limiter: Waiting {:?} for next token", wait_duration);
            sleep(wait_duration).await;
        }
    }

    /// Try to acquire a token without waiting
    /// Returns true if acquired, false if rate limited
    pub fn try_acquire(&self) -> bool {
        if !self.config.enabled {
            return true;
        }

        let mut inner = self.inner.lock().unwrap();

        // Refill tokens based on time passed
        self.refill_tokens(&mut inner);

        if inner.tokens >= 1.0 {
            inner.tokens -= 1.0;
            inner.requests_made += 1;
            debug!(
                "Rate limiter: Request approved (try_acquire), {} tokens remaining",
                inner.tokens
            );
            true
        } else {
            inner.requests_rejected += 1;
            debug!(
                "Rate limiter: Request rejected (try_acquire), {} tokens available",
                inner.tokens
            );
            false
        }
    }

    /// Get current rate limiter statistics
    pub fn stats(&self) -> RateLimiterStats {
        let inner = self.inner.lock().unwrap();
        RateLimiterStats {
            tokens_available: inner.tokens,
            requests_made: inner.requests_made,
            requests_rejected: inner.requests_rejected,
            enabled: self.config.enabled,
            requests_per_minute: self.config.requests_per_minute,
            burst_capacity: self.config.burst_capacity,
        }
    }

    /// Reset the rate limiter statistics and token count
    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.tokens = if self.config.enabled {
            self.config.burst_capacity as f64
        } else {
            f64::MAX
        };
        inner.last_refill = Instant::now();
        inner.requests_made = 0;
        inner.requests_rejected = 0;
    }

    /// Refill tokens based on elapsed time
    fn refill_tokens(&self, inner: &mut RateLimiterInner) {
        if !self.config.enabled {
            return;
        }

        let now = Instant::now();
        let elapsed = now.duration_since(inner.last_refill);

        // Calculate tokens to add based on elapsed time
        let tokens_per_second = self.config.requests_per_minute as f64 / 60.0;
        let tokens_to_add = elapsed.as_secs_f64() * tokens_per_second;

        if tokens_to_add > 0.0 {
            inner.tokens = (inner.tokens + tokens_to_add).min(self.config.burst_capacity as f64);
            inner.last_refill = now;
            debug!(
                "Rate limiter: Added {:.2} tokens, now have {:.2}",
                tokens_to_add, inner.tokens
            );
        }
    }

    /// Calculate how long to wait for the next token
    fn calculate_wait_time(&self) -> Duration {
        if !self.config.enabled {
            return Duration::from_millis(0);
        }

        // Calculate time for one token to be added
        let seconds_per_token = 60.0 / self.config.requests_per_minute as f64;
        Duration::from_secs_f64(seconds_per_token)
    }
}

/// Rate limiter statistics
#[derive(Debug, Clone)]
pub struct RateLimiterStats {
    /// Current number of tokens available
    pub tokens_available: f64,
    /// Total requests that were approved
    pub requests_made: u64,
    /// Total requests that were rejected due to rate limiting
    pub requests_rejected: u64,
    /// Whether rate limiting is enabled
    pub enabled: bool,
    /// Configured requests per minute limit
    pub requests_per_minute: u32,
    /// Configured burst capacity
    pub burst_capacity: u32,
}

impl RateLimiterStats {
    /// Calculate the acceptance rate (approved / total)
    pub fn acceptance_rate(&self) -> f64 {
        let total = self.requests_made + self.requests_rejected;
        if total == 0 {
            1.0
        } else {
            self.requests_made as f64 / total as f64
        }
    }

    /// Calculate the rejection rate (rejected / total)
    pub fn rejection_rate(&self) -> f64 {
        1.0 - self.acceptance_rate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn test_rate_limiter_disabled() {
        let config = RateLimitConfig {
            requests_per_minute: 60,
            burst_capacity: 10,
            enabled: false,
        };

        let limiter = RateLimiter::new(config);

        // Should allow unlimited requests when disabled
        for _ in 0..100 {
            assert!(limiter.try_acquire());
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_burst_capacity() {
        let config = RateLimitConfig {
            requests_per_minute: 60,
            burst_capacity: 5,
            enabled: true,
        };

        let limiter = RateLimiter::new(config);

        // Should allow burst capacity requests immediately
        for _ in 0..5 {
            assert!(limiter.try_acquire());
        }

        // Next request should be rejected
        assert!(!limiter.try_acquire());
    }

    #[tokio::test]
    async fn test_rate_limiter_token_refill() {
        let config = RateLimitConfig {
            requests_per_minute: 120, // 2 requests per second
            burst_capacity: 2,
            enabled: true,
        };

        let limiter = RateLimiter::new(config);

        // Use up burst capacity
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire());

        // Wait for token refill (0.5 seconds = 1 token at 2 tokens/sec)
        sleep(Duration::from_millis(500)).await;

        // Should have one token available now
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire());
    }

    #[tokio::test]
    async fn test_rate_limiter_stats() {
        let config = RateLimitConfig {
            requests_per_minute: 60,
            burst_capacity: 3,
            enabled: true,
        };

        let limiter = RateLimiter::new(config);

        // Make some requests
        limiter.try_acquire(); // approved
        limiter.try_acquire(); // approved
        limiter.try_acquire(); // approved
        limiter.try_acquire(); // rejected
        limiter.try_acquire(); // rejected

        let stats = limiter.stats();
        assert_eq!(stats.requests_made, 3);
        assert_eq!(stats.requests_rejected, 2);
        assert_eq!(stats.acceptance_rate(), 0.6);
        assert_eq!(stats.rejection_rate(), 0.4);
    }

    #[tokio::test]
    async fn test_rate_limiter_acquire_with_wait() {
        let config = RateLimitConfig {
            requests_per_minute: 120, // Very fast for testing
            burst_capacity: 1,
            enabled: true,
        };

        let limiter = RateLimiter::new(config);

        // First request should succeed immediately
        assert!(limiter.acquire().await);

        // Second request should wait and then succeed
        let start = Instant::now();
        assert!(limiter.acquire().await);
        let elapsed = start.elapsed();

        // Should have waited approximately 0.5 seconds (60/120 = 0.5s per token)
        assert!(elapsed >= Duration::from_millis(400)); // Allow some tolerance
    }
}
