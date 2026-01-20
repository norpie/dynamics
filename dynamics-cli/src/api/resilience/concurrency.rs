//! Concurrency limiter implementation
//!
//! Provides a semaphore-based limiter for controlling concurrent API requests
//! to Dynamics 365, preventing exceeding the 52 concurrent connection limit.

use super::config::ConcurrencyConfig;
use log::debug;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Semaphore-based concurrency limiter for controlling concurrent API requests
#[derive(Debug, Clone)]
pub struct ConcurrencyLimiter {
    semaphore: Arc<Semaphore>,
    config: ConcurrencyConfig,
    requests_acquired: Arc<AtomicU64>,
    requests_waited: Arc<AtomicU64>,
}

impl ConcurrencyLimiter {
    /// Create a new concurrency limiter with the given configuration
    pub fn new(config: ConcurrencyConfig) -> Self {
        let permits = if config.enabled {
            config.max_concurrent_requests
        } else {
            // Use a large but valid number when disabled (Tokio Semaphore max is 2^61-1)
            1_000_000
        };

        Self {
            semaphore: Arc::new(Semaphore::new(permits)),
            config,
            requests_acquired: Arc::new(AtomicU64::new(0)),
            requests_waited: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Acquire a permit for making a request. Waits if at capacity.
    /// Returns an owned permit that releases automatically when dropped.
    pub async fn acquire(&self) -> OwnedSemaphorePermit {
        if !self.config.enabled {
            // When disabled, still return a permit but from unlimited pool
            return self.semaphore.clone().acquire_owned().await.unwrap();
        }

        let available_before = self.semaphore.available_permits();
        let will_wait = available_before == 0;

        if will_wait {
            self.requests_waited.fetch_add(1, Ordering::Relaxed);
            debug!(
                "Concurrency limiter: waiting for permit ({} in use)",
                self.config.max_concurrent_requests
            );
        }

        let permit = self.semaphore.clone().acquire_owned().await.unwrap();
        self.requests_acquired.fetch_add(1, Ordering::Relaxed);

        debug!(
            "Concurrency limiter: acquired permit ({}/{} in use)",
            self.config.max_concurrent_requests - self.semaphore.available_permits(),
            self.config.max_concurrent_requests
        );

        permit
    }

    /// Try to acquire a permit without waiting.
    /// Returns None if no permits are available.
    pub fn try_acquire(&self) -> Option<OwnedSemaphorePermit> {
        if !self.config.enabled {
            return Some(self.semaphore.clone().try_acquire_owned().ok()?);
        }

        match self.semaphore.clone().try_acquire_owned() {
            Ok(permit) => {
                self.requests_acquired.fetch_add(1, Ordering::Relaxed);
                debug!(
                    "Concurrency limiter: acquired permit (try_acquire) ({}/{} in use)",
                    self.config.max_concurrent_requests - self.semaphore.available_permits(),
                    self.config.max_concurrent_requests
                );
                Some(permit)
            }
            Err(_) => {
                debug!(
                    "Concurrency limiter: no permits available ({}/{} in use)",
                    self.config.max_concurrent_requests, self.config.max_concurrent_requests
                );
                None
            }
        }
    }

    /// Get the number of available permits (requests that can start immediately)
    pub fn available_permits(&self) -> usize {
        if !self.config.enabled {
            return usize::MAX;
        }
        self.semaphore.available_permits()
    }

    /// Get the maximum number of queue items that can run concurrently
    pub fn max_queue_items(&self) -> usize {
        self.config.max_queue_items
    }

    /// Get the maximum number of concurrent HTTP requests
    pub fn max_concurrent_requests(&self) -> usize {
        self.config.max_concurrent_requests
    }

    /// Check if the limiter is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get current statistics
    pub fn stats(&self) -> ConcurrencyStats {
        ConcurrencyStats {
            available_permits: self.available_permits(),
            max_concurrent_requests: self.config.max_concurrent_requests,
            max_queue_items: self.config.max_queue_items,
            requests_acquired: self.requests_acquired.load(Ordering::Relaxed),
            requests_waited: self.requests_waited.load(Ordering::Relaxed),
            enabled: self.config.enabled,
        }
    }

    /// Reset statistics (but not the semaphore state)
    pub fn reset_stats(&self) {
        self.requests_acquired.store(0, Ordering::Relaxed);
        self.requests_waited.store(0, Ordering::Relaxed);
    }
}

/// Statistics for the concurrency limiter
#[derive(Debug, Clone)]
pub struct ConcurrencyStats {
    /// Number of permits currently available
    pub available_permits: usize,
    /// Maximum concurrent requests allowed
    pub max_concurrent_requests: usize,
    /// Maximum queue items that can run concurrently
    pub max_queue_items: usize,
    /// Total permits acquired since creation/reset
    pub requests_acquired: u64,
    /// Number of times a request had to wait for a permit
    pub requests_waited: u64,
    /// Whether limiting is enabled
    pub enabled: bool,
}

impl ConcurrencyStats {
    /// Calculate the percentage of requests that had to wait
    pub fn wait_rate(&self) -> f64 {
        if self.requests_acquired == 0 {
            0.0
        } else {
            self.requests_waited as f64 / self.requests_acquired as f64
        }
    }

    /// Get the number of permits currently in use
    pub fn in_use(&self) -> usize {
        if !self.enabled {
            return 0;
        }
        self.max_concurrent_requests
            .saturating_sub(self.available_permits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_concurrency_limiter_disabled() {
        let config = ConcurrencyConfig {
            max_concurrent_requests: 5,
            max_queue_items: 3,
            enabled: false,
        };

        let limiter = ConcurrencyLimiter::new(config);

        // Should allow unlimited when disabled
        let mut permits = Vec::new();
        for _ in 0..100 {
            permits.push(limiter.try_acquire().unwrap());
        }
        assert_eq!(permits.len(), 100);
    }

    #[tokio::test]
    async fn test_concurrency_limiter_max_permits() {
        let config = ConcurrencyConfig {
            max_concurrent_requests: 3,
            max_queue_items: 2,
            enabled: true,
        };

        let limiter = ConcurrencyLimiter::new(config);

        // Should allow up to max_concurrent_requests
        let p1 = limiter.try_acquire();
        let p2 = limiter.try_acquire();
        let p3 = limiter.try_acquire();
        let p4 = limiter.try_acquire();

        assert!(p1.is_some());
        assert!(p2.is_some());
        assert!(p3.is_some());
        assert!(p4.is_none()); // Should fail - at capacity

        assert_eq!(limiter.available_permits(), 0);
    }

    #[tokio::test]
    async fn test_concurrency_limiter_release() {
        let config = ConcurrencyConfig {
            max_concurrent_requests: 2,
            max_queue_items: 1,
            enabled: true,
        };

        let limiter = ConcurrencyLimiter::new(config);

        // Acquire all permits
        let p1 = limiter.try_acquire().unwrap();
        let _p2 = limiter.try_acquire().unwrap();
        assert!(limiter.try_acquire().is_none());

        // Release one
        drop(p1);

        // Should be able to acquire again
        assert!(limiter.try_acquire().is_some());
    }

    #[tokio::test]
    async fn test_concurrency_limiter_acquire_waits() {
        let config = ConcurrencyConfig {
            max_concurrent_requests: 1,
            max_queue_items: 1,
            enabled: true,
        };

        let limiter = ConcurrencyLimiter::new(config);
        let limiter_clone = limiter.clone();

        // Acquire the only permit
        let permit = limiter.acquire().await;
        assert_eq!(limiter.available_permits(), 0);

        // Spawn a task that will wait for a permit
        let handle = tokio::spawn(async move {
            let _permit = limiter_clone.acquire().await;
            true
        });

        // Give the spawned task time to start waiting
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Release the permit
        drop(permit);

        // The waiting task should complete
        let result = tokio::time::timeout(tokio::time::Duration::from_millis(100), handle).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_concurrency_limiter_stats() {
        let config = ConcurrencyConfig {
            max_concurrent_requests: 3,
            max_queue_items: 2,
            enabled: true,
        };

        let limiter = ConcurrencyLimiter::new(config);

        let _p1 = limiter.acquire().await;
        let _p2 = limiter.acquire().await;

        let stats = limiter.stats();
        assert_eq!(stats.max_concurrent_requests, 3);
        assert_eq!(stats.max_queue_items, 2);
        assert_eq!(stats.available_permits, 1);
        assert_eq!(stats.requests_acquired, 2);
        assert!(stats.enabled);
    }

    #[test]
    fn test_max_queue_items() {
        let config = ConcurrencyConfig {
            max_concurrent_requests: 20,
            max_queue_items: 10,
            enabled: true,
        };

        let limiter = ConcurrencyLimiter::new(config);
        assert_eq!(limiter.max_queue_items(), 10);
        assert_eq!(limiter.max_concurrent_requests(), 20);
    }
}
