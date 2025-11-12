//! Rate Limiting Middleware for Synap
//!
//! Simple token bucket rate limiting with:
//! - Per-IP rate limiting
//! - Configurable requests per second
//! - Burst capacity

use axum::{extract::ConnectInfo, http::StatusCode, middleware::Next, response::Response};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::RateLimitConfig;

/// Token bucket for rate limiting
#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
    capacity: f64,
    refill_rate: f64, // tokens per second
}

impl TokenBucket {
    fn new(capacity: u64, refill_rate: u64) -> Self {
        Self {
            tokens: capacity as f64,
            last_refill: Instant::now(),
            capacity: capacity as f64,
            refill_rate: refill_rate as f64,
        }
    }

    fn try_consume(&mut self, tokens: f64) -> bool {
        // Refill tokens based on elapsed time
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let new_tokens = elapsed * self.refill_rate;

        self.tokens = (self.tokens + new_tokens).min(self.capacity);
        self.last_refill = now;

        // Try to consume
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }
}

/// Rate limiter state
pub struct RateLimiter {
    buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,
    config: RateLimitConfig,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub fn check_rate_limit(&self, ip: &str) -> bool {
        let mut buckets = self.buckets.write();

        let bucket = buckets.entry(ip.to_string()).or_insert_with(|| {
            TokenBucket::new(self.config.burst_size, self.config.requests_per_second)
        });

        bucket.try_consume(1.0)
    }

    /// Cleanup old entries periodically
    pub fn cleanup(&self) {
        let mut buckets = self.buckets.write();
        buckets.retain(|_, bucket| {
            // Keep if refilled in last 5 minutes
            bucket.last_refill.elapsed() < Duration::from_secs(300)
        });
    }
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self {
            buckets: Arc::clone(&self.buckets),
            config: self.config.clone(),
        }
    }
}

/// Rate limit middleware
pub async fn rate_limit_middleware(
    limiter: Arc<RateLimiter>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract IP from request
    let ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    if !limiter.check_rate_limit(&ip) {
        tracing::warn!("Rate limit exceeded for IP: {}", ip);
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket() {
        let mut bucket = TokenBucket::new(10, 100);

        // Should allow first request
        assert!(bucket.try_consume(1.0));

        // Should allow up to capacity
        for _ in 0..9 {
            assert!(bucket.try_consume(1.0));
        }

        // Should deny when empty
        assert!(!bucket.try_consume(1.0));
    }

    #[test]
    fn test_rate_limiter() {
        let config = RateLimitConfig {
            enabled: true,
            requests_per_second: 100,
            burst_size: 10,
        };

        let limiter = RateLimiter::new(config);

        // Should allow requests up to burst size
        for _ in 0..10 {
            assert!(limiter.check_rate_limit("192.168.1.1"));
        }

        // Should deny after burst
        assert!(!limiter.check_rate_limit("192.168.1.1"));

        // Different IP should have own bucket
        assert!(limiter.check_rate_limit("192.168.1.2"));
    }

    #[test]
    fn test_limiter_cleanup() {
        let config = RateLimitConfig {
            enabled: true,
            requests_per_second: 100,
            burst_size: 10,
        };

        let limiter = RateLimiter::new(config);

        // Add some entries
        limiter.check_rate_limit("192.168.1.1");
        limiter.check_rate_limit("192.168.1.2");

        // Cleanup shouldn't remove recent entries
        limiter.cleanup();

        assert_eq!(limiter.buckets.read().len(), 2);
    }
}
