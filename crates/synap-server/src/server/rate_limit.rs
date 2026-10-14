//! Per-client HTTP rate limiting.
//!
//! A token bucket per client: `burst_size` tokens, refilled at
//! `requests_per_second`, one token per request. When `rate_limit.enabled`, the
//! router installs two layers over every route:
//!
//! - [`limit_by_peer`], outside authentication: every request spends a token
//!   from the bucket of its TCP peer address. `X-Forwarded-For` and similar
//!   headers are ignored, since the client writes them. Running before
//!   authentication is what bounds credential checks (bcrypt for passwords),
//!   failed or not.
//! - [`limit_by_principal`], inside authentication: a request that
//!   authenticated also spends a token from the bucket of its principal (the
//!   user, else the API key; `hub:<uuid>` for a Hub tenant), so one principal
//!   is bounded across every address it connects from.
//!
//! A limited request answers 429 with `Retry-After` (whole seconds until the
//! bucket holds a token again). `/health` and `/metrics` are exempt: probes and
//! scrapers must not be throttled into reporting an outage.

use axum::{
    Json,
    extract::{ConnectInfo, Request},
    http::{HeaderValue, StatusCode, header::RETRY_AFTER},
    middleware::Next,
    response::{IntoResponse, Response},
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::auth::AuthContext;
use crate::config::RateLimitConfig;

use crate::hub::restrictions::{HubSaaSRestrictions, Plan};

/// Buckets kept before the full ones are pruned (see `RateLimiter::prune`).
const PRUNE_ABOVE: usize = 10_000;

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

    /// Time until the bucket holds `tokens`, as of its last refill; zero when
    /// it already does. A bucket that never refills never will.
    fn time_until(&self, tokens: f64) -> Duration {
        let missing = tokens - self.tokens;
        if missing <= 0.0 {
            return Duration::ZERO;
        }
        Duration::try_from_secs_f64(missing / self.refill_rate).unwrap_or(Duration::MAX)
    }

    /// Whether the bucket has refilled to capacity by `now`. A full bucket
    /// holds nothing a fresh one would not, so dropping it loses no state.
    fn is_full_at(&self, now: Instant) -> bool {
        let elapsed = now
            .saturating_duration_since(self.last_refill)
            .as_secs_f64();
        self.tokens + elapsed * self.refill_rate >= self.capacity
    }
}

/// Rate limiter state
pub struct RateLimiter {
    /// Buckets keyed by client: "ip:{address}", "principal:{user_id}",
    /// "api-key:{key_id}", or "user:{user_id}" for Hub plan limits.
    buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,
    /// Bucket count at which the next new client triggers a prune.
    prune_above: Arc<AtomicUsize>,
    config: RateLimitConfig,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            prune_above: Arc::new(AtomicUsize::new(PRUNE_ABOVE)),
            config,
        }
    }

    /// Check rate limit for IP address (standalone mode or fallback)
    pub fn check_rate_limit(&self, ip: &str) -> bool {
        self.check_key(&format!("ip:{}", ip)).allowed
    }

    /// Spend one token from the bucket of client `key`, created full with the
    /// configured `burst_size` and `requests_per_second` on first use.
    ///
    /// `reset_in` is how long until the bucket holds a token again: zero while
    /// it still has one, the wait a limited client must observe otherwise.
    pub fn check_key(&self, key: &str) -> RateLimitResult {
        let now = Instant::now();
        let mut buckets = self.buckets.write();
        if !buckets.contains_key(key) {
            self.prune(&mut buckets, now);
        }
        let bucket = buckets.entry(key.to_owned()).or_insert_with(|| {
            TokenBucket::new(self.config.burst_size, self.config.requests_per_second)
        });
        let allowed = bucket.try_consume(1.0);
        RateLimitResult {
            allowed,
            limit: self.config.requests_per_second,
            remaining: bucket.tokens.floor() as u64,
            reset_in: bucket.time_until(1.0),
        }
    }

    /// Drop the buckets that have refilled to capacity once there are many.
    ///
    /// One bucket per client address would otherwise grow without bound under
    /// address churn. Pruning only full buckets is lossless: a client that
    /// returns gets a fresh, full bucket, exactly what it had. Buckets still
    /// in use survive, and the next prune waits until the map has doubled past
    /// them, so the scan costs amortized O(1) per new client.
    fn prune(&self, buckets: &mut HashMap<String, TokenBucket>, now: Instant) {
        if buckets.len() < self.prune_above.load(Ordering::Relaxed) {
            return;
        }
        buckets.retain(|_, bucket| !bucket.is_full_at(now));
        self.prune_above
            .store((buckets.len() * 2).max(PRUNE_ABOVE), Ordering::Relaxed);
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
            prune_above: Arc::clone(&self.prune_above),
            config: self.config.clone(),
        }
    }
}

/// Rate limit by peer address. Installed outside authentication, so it bounds
/// every request a client host sends, credential checks included.
///
/// The peer is the TCP peer from `ConnectInfo`, never a forwarding header. A
/// router served without connection info has no peer to tell clients apart
/// by, so all its clients share one bucket rather than going unlimited.
pub async fn limit_by_peer(limiter: Arc<RateLimiter>, request: Request, next: Next) -> Response {
    if super::router::is_public_path(request.uri().path()) {
        return next.run(request).await;
    }
    let key = match request.extensions().get::<ConnectInfo<SocketAddr>>() {
        Some(ConnectInfo(addr)) => format!("ip:{}", addr.ip()),
        None => "ip:unknown".to_string(),
    };
    admit(&limiter, &key, request, next).await
}

/// Rate limit by authenticated principal. Installed inside authentication,
/// where the `AuthContext` of the request names it: the user when there is
/// one, else the API key. Anonymous requests pass; the peer limit already
/// bounds them.
pub async fn limit_by_principal(
    limiter: Arc<RateLimiter>,
    request: Request,
    next: Next,
) -> Response {
    let principal = request.extensions().get::<AuthContext>().and_then(|ctx| {
        ctx.user_id
            .as_ref()
            .map(|user| format!("principal:{user}"))
            .or_else(|| ctx.api_key_id.as_ref().map(|key| format!("api-key:{key}")))
    });
    match principal {
        Some(key) => admit(&limiter, &key, request, next).await,
        None => next.run(request).await,
    }
}

/// Run the request if client `key` has a token, else answer 429.
async fn admit(limiter: &RateLimiter, key: &str, request: Request, next: Next) -> Response {
    let result = limiter.check_key(key);
    if result.allowed {
        return next.run(request).await;
    }
    // Debug, not warn: a client hammering the limit would otherwise turn
    // every rejected request into a log line.
    tracing::debug!("Rate limit exceeded for {}", key);
    too_many_requests(result.reset_in)
}

/// 429 with `Retry-After` set to the whole seconds until a token is back.
fn too_many_requests(wait: Duration) -> Response {
    let retry_after = retry_after_secs(wait);
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(serde_json::json!({
            "error": format!("Rate limit exceeded; retry after {retry_after} s"),
            "code": 429,
        })),
    )
        .into_response();
    response
        .headers_mut()
        .insert(RETRY_AFTER, HeaderValue::from(retry_after));
    response
}

/// `Retry-After` carries whole seconds: round up, and never say 0, which would
/// invite an immediate retry that is still refused.
fn retry_after_secs(wait: Duration) -> u64 {
    wait.as_secs()
        .saturating_add(u64::from(wait.subsec_nanos() > 0))
        .max(1)
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

    fn limiter(requests_per_second: u64, burst_size: u64) -> RateLimiter {
        RateLimiter::new(RateLimitConfig {
            enabled: true,
            requests_per_second,
            burst_size,
        })
    }

    #[test]
    fn a_limited_key_learns_when_a_token_is_back() {
        let limiter = limiter(10, 2);
        assert_eq!(limiter.check_key("ip:a").reset_in, Duration::ZERO);
        assert!(limiter.check_key("ip:a").allowed);
        let limited = limiter.check_key("ip:a");
        assert!(!limited.allowed);
        // One token at 10/s is at most 100 ms away.
        assert!(limited.reset_in > Duration::ZERO);
        assert!(limited.reset_in <= Duration::from_millis(100));
        assert!(limiter.check_key("ip:b").allowed, "keys are independent");
    }

    #[test]
    fn retry_after_rounds_up_to_whole_seconds() {
        assert_eq!(retry_after_secs(Duration::ZERO), 1);
        assert_eq!(retry_after_secs(Duration::from_millis(100)), 1);
        assert_eq!(retry_after_secs(Duration::from_secs(1)), 1);
        assert_eq!(retry_after_secs(Duration::from_millis(1001)), 2);
        assert_eq!(retry_after_secs(Duration::MAX), u64::MAX);
    }

    #[test]
    fn a_bucket_that_never_refills_reports_an_unbounded_wait() {
        let mut bucket = TokenBucket::new(1, 0);
        assert!(bucket.try_consume(1.0));
        assert!(!bucket.try_consume(1.0));
        assert_eq!(bucket.time_until(1.0), Duration::MAX);
    }

    #[test]
    fn pruning_drops_only_full_buckets() {
        let limiter = limiter(1, 2);
        for client in 0..PRUNE_ABOVE {
            // A full bucket is indistinguishable from a fresh one.
            limiter
                .buckets
                .write()
                .insert(format!("ip:idle-{client}"), TokenBucket::new(2, 1));
        }
        // A new client triggers the prune; its own bucket is not full.
        assert!(limiter.check_key("ip:active").allowed);
        {
            let buckets = limiter.buckets.read();
            assert_eq!(buckets.len(), 1, "every full bucket was pruned");
            assert!(buckets.contains_key("ip:active"));
        }
        // The next prune waits until the map grows back to the floor.
        assert_eq!(limiter.prune_above.load(Ordering::Relaxed), PRUNE_ABOVE);
        assert!(limiter.check_key("ip:active").allowed, "its state was kept");
        assert!(!limiter.check_key("ip:active").allowed);
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
