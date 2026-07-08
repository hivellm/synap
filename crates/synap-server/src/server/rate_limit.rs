//! Rate Limiting Middleware for Synap
//!
//! Token bucket rate limiting with:
//! - Per-user rate limiting (Hub mode) - uses Plan-based limits
//! - Per-IP rate limiting (standalone mode or fallback)
//! - Configurable requests per second
//! - Burst capacity

use axum::{extract::ConnectInfo, http::StatusCode, middleware::Next, response::Response};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::RateLimitConfig;

use crate::hub::{
    HubUserContext,
    restrictions::{HubSaaSRestrictions, Plan},
};

/// Rate limit check result with metadata for response headers
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,
    /// Rate limit (requests per second)
    pub limit: u64,
    /// Remaining tokens in bucket
    pub remaining: u64,
    /// Time until bucket refills
    pub reset_in: Duration,
}

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
    /// Buckets keyed by: "user:{user_id}" or "ip:{ip_address}"
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

    /// Check rate limit for IP address (standalone mode or fallback)
    pub fn check_rate_limit(&self, ip: &str) -> bool {
        let mut buckets = self.buckets.write();

        let key = format!("ip:{}", ip);
        let bucket = buckets.entry(key).or_insert_with(|| {
            TokenBucket::new(self.config.burst_size, self.config.requests_per_second)
        });

        bucket.try_consume(1.0)
    }

    /// Check rate limit for authenticated user (Hub mode)
    ///
    /// Uses Plan-based limits from HubSaaSRestrictions
    pub fn check_user_rate_limit(&self, user_id: &str, plan: Plan) -> RateLimitResult {
        let mut buckets = self.buckets.write();

        let key = format!("user:{}", user_id);

        // Get plan-specific limits
        let requests_per_second = HubSaaSRestrictions::max_requests_per_second(plan) as u64;
        let burst_size = requests_per_second * 2; // Burst = 2x rate limit

        let bucket = buckets
            .entry(key)
            .or_insert_with(|| TokenBucket::new(burst_size, requests_per_second));

        let allowed = bucket.try_consume(1.0);

        RateLimitResult {
            allowed,
            limit: requests_per_second,
            remaining: bucket.tokens.floor() as u64,
            reset_in: Duration::from_secs(1), // Tokens refill every second
        }
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
///
/// Uses per-user rate limiting in Hub mode (with Plan-based limits)
/// Falls back to IP-based rate limiting in standalone mode
pub async fn rate_limit_middleware(
    limiter: Arc<RateLimiter>,
    mut request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    {
        // Try to get Hub user context
        if let Some(hub_ctx) = request.extensions().get::<HubUserContext>().cloned() {
            // Use per-user rate limiting with Plan-based limits
            let user_id = hub_ctx.user_id.to_string();
            let result = limiter.check_user_rate_limit(&user_id, hub_ctx.plan);

            if !result.allowed {
                tracing::warn!(
                    "Rate limit exceeded for user {}: {} req/s limit ({:?} plan)",
                    hub_ctx.user_id,
                    result.limit,
                    hub_ctx.plan
                );
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }

            // Store rate limit info in request extensions for response headers
            request.extensions_mut().insert(result);

            let mut response = next.run(request).await;

            // Add rate limit headers to response (Task 5.4)
            add_rate_limit_headers(&mut response, &limiter, Some(&user_id), Some(hub_ctx.plan));

            return Ok(response);
        }
    }

    // Fallback to IP-based rate limiting (standalone mode or no Hub context)
    let ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    if !limiter.check_rate_limit(&ip) {
        tracing::warn!("Rate limit exceeded for IP: {}", ip);
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let mut response = next.run(request).await;

    // Add basic rate limit headers for IP-based limiting
    add_rate_limit_headers(&mut response, &limiter, None, None);

    Ok(response)
}

/// Add rate limit headers to response (Task 5.4)
///
/// Headers follow standard conventions:
/// - X-RateLimit-Limit: Requests per second allowed
/// - X-RateLimit-Remaining: Remaining requests in current window
/// - X-RateLimit-Reset: Seconds until rate limit resets
fn add_rate_limit_headers(
    response: &mut Response,
    limiter: &RateLimiter,
    user_id: Option<&str>,
    plan: Option<Plan>,
) {
    if let (Some(uid), Some(p)) = (user_id, plan) {
        // Get plan-specific limits
        let requests_per_second = HubSaaSRestrictions::max_requests_per_second(p) as u64;

        // Get current bucket state
        let buckets = limiter.buckets.read();
        let key = format!("user:{}", uid);

        if let Some(bucket) = buckets.get(&key) {
            let remaining = bucket.tokens.floor() as u64;

            response.headers_mut().insert(
                "X-RateLimit-Limit",
                requests_per_second.to_string().parse().unwrap(),
            );
            response.headers_mut().insert(
                "X-RateLimit-Remaining",
                remaining.to_string().parse().unwrap(),
            );
            response.headers_mut().insert(
                "X-RateLimit-Reset",
                "1".parse().unwrap(), // Refills every 1 second
            );

            return;
        }
    }

    // Fallback: Add global config-based headers
    response.headers_mut().insert(
        "X-RateLimit-Limit",
        limiter
            .config
            .requests_per_second
            .to_string()
            .parse()
            .unwrap(),
    );
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

    #[test]
    fn test_user_rate_limiting() {
        let config = RateLimitConfig {
            enabled: true,
            requests_per_second: 100,
            burst_size: 10,
        };

        let limiter = RateLimiter::new(config);

        let user_id = "user-123";

        // Free plan: 10 req/s, burst of 20
        for i in 0..20 {
            let result = limiter.check_user_rate_limit(user_id, Plan::Free);
            assert!(result.allowed, "Request {} should be allowed (burst)", i);
            assert_eq!(result.limit, 10); // Free plan limit
        }

        // Should deny after burst
        let result = limiter.check_user_rate_limit(user_id, Plan::Free);
        assert!(!result.allowed, "Request should be denied after burst");

        // Different user should have own bucket
        let user_id2 = "user-456";
        let result = limiter.check_user_rate_limit(user_id2, Plan::Free);
        assert!(result.allowed, "Different user should have own bucket");
    }

    #[test]
    fn test_plan_based_limits() {
        let config = RateLimitConfig {
            enabled: true,
            requests_per_second: 100,
            burst_size: 10,
        };

        let limiter = RateLimiter::new(config);

        // Test different plans have different limits
        let result_free = limiter.check_user_rate_limit("free-user", Plan::Free);
        assert_eq!(result_free.limit, 10); // Free: 10 req/s

        let result_pro = limiter.check_user_rate_limit("pro-user", Plan::Pro);
        assert_eq!(result_pro.limit, 100); // Pro: 100 req/s

        let result_enterprise = limiter.check_user_rate_limit("ent-user", Plan::Enterprise);
        assert_eq!(result_enterprise.limit, 1000); // Enterprise: 1000 req/s
    }
}
