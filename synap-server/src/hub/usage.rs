//! Usage Tracking and Reporting
//!
//! Background task that aggregates usage metrics per user and
//! reports to HiveHub API every 5 minutes for billing purposes.

use super::client::HubClient;
use super::sdk_stubs::ResourceType;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;
use tracing::{debug, error, info};
use uuid::Uuid;

/// Usage metrics per user
#[derive(Debug, Clone)]
pub struct UserUsageMetrics {
    /// Storage bytes used
    pub storage_bytes: u64,
    /// Number of operations performed since last report
    pub operations_count: u64,
    /// API requests made since last report
    pub api_requests: u64,
    /// Last updated timestamp
    pub last_updated: Instant,
}

impl Default for UserUsageMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl UserUsageMetrics {
    pub fn new() -> Self {
        Self {
            storage_bytes: 0,
            operations_count: 0,
            api_requests: 0,
            last_updated: Instant::now(),
        }
    }

    /// Reset counters (after successful report)
    pub fn reset_counters(&mut self) {
        self.operations_count = 0;
        self.api_requests = 0;
        self.last_updated = Instant::now();
    }
}

/// Usage reporter for tracking and reporting metrics to HiveHub
pub struct UsageReporter {
    /// Usage metrics per user
    metrics: Arc<RwLock<HashMap<Uuid, UserUsageMetrics>>>,
    /// Reporting interval
    report_interval: Duration,
}

impl UsageReporter {
    /// Create a new usage reporter
    pub fn new(report_interval: Duration) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            report_interval,
        }
    }

    /// Record storage usage for a user
    pub fn record_storage(&self, user_id: &Uuid, bytes: u64) {
        let mut metrics = self.metrics.write();
        let entry = metrics.entry(*user_id).or_default();
        entry.storage_bytes = bytes; // Absolute value
        entry.last_updated = Instant::now();
    }

    /// Record an operation for a user
    pub fn record_operation(&self, user_id: &Uuid) {
        let mut metrics = self.metrics.write();
        let entry = metrics.entry(*user_id).or_default();
        entry.operations_count = entry.operations_count.saturating_add(1);
        entry.last_updated = Instant::now();
    }

    /// Record an API request for a user
    pub fn record_api_request(&self, user_id: &Uuid) {
        let mut metrics = self.metrics.write();
        let entry = metrics.entry(*user_id).or_default();
        entry.api_requests = entry.api_requests.saturating_add(1);
        entry.last_updated = Instant::now();
    }

    /// Get current metrics for a user
    pub fn get_metrics(&self, user_id: &Uuid) -> Option<UserUsageMetrics> {
        let metrics = self.metrics.read();
        metrics.get(user_id).cloned()
    }

    /// Get all user metrics (for reporting)
    pub fn get_all_metrics(&self) -> HashMap<Uuid, UserUsageMetrics> {
        let metrics = self.metrics.read();
        metrics.clone()
    }

    /// Reset metrics for a user after successful report
    pub fn reset_user_metrics(&self, user_id: &Uuid) {
        let mut metrics = self.metrics.write();
        if let Some(entry) = metrics.get_mut(user_id) {
            entry.reset_counters();
        }
    }

    /// Clear all metrics
    pub fn clear_all_metrics(&self) {
        let mut metrics = self.metrics.write();
        metrics.clear();
    }

    /// Get statistics
    pub fn get_stats(&self) -> UsageReporterStats {
        let metrics = self.metrics.read();
        let total_users = metrics.len();
        let total_operations: u64 = metrics.values().map(|m| m.operations_count).sum();
        let total_requests: u64 = metrics.values().map(|m| m.api_requests).sum();
        let total_storage: u64 = metrics.values().map(|m| m.storage_bytes).sum();

        UsageReporterStats {
            total_users,
            total_operations,
            total_requests,
            total_storage_bytes: total_storage,
            report_interval_seconds: self.report_interval.as_secs(),
        }
    }

    /// Start background reporting task
    ///
    /// Reports aggregated usage metrics to HiveHub API every N seconds
    ///
    /// # Phase 6.4 & 6.5 - Usage reporting with error handling
    pub async fn start_reporting_task(&self, hub_client: Arc<HubClient>) {
        let mut ticker = interval(self.report_interval);
        let metrics_ref = self.metrics.clone();

        info!(
            "Usage reporter started - reporting interval: {} seconds",
            self.report_interval.as_secs()
        );

        loop {
            ticker.tick().await;

            // Get snapshot of current metrics
            let metrics_snapshot = {
                let metrics = metrics_ref.read();
                metrics.clone()
            };

            if metrics_snapshot.is_empty() {
                debug!("No usage metrics to report");
                continue;
            }

            info!(
                "Reporting usage metrics for {} users",
                metrics_snapshot.len()
            );

            // Phase 6.4: Send usage to HiveHub API for each user
            for (user_id, metrics) in metrics_snapshot.iter() {
                debug!(
                    "Reporting for user {}: operations={}, requests={}, storage={} bytes",
                    user_id, metrics.operations_count, metrics.api_requests, metrics.storage_bytes
                );

                // Send aggregated usage to Hub
                let result = hub_client
                    .update_usage(
                        user_id,
                        ResourceType::KeyValue,
                        "synap_aggregate", // Aggregate usage across all Synap resources
                        Some(metrics.operations_count),
                        Some(metrics.storage_bytes),
                    )
                    .await;

                // Phase 6.5: Handle errors gracefully - don't fail the task
                match result {
                    Ok(_) => {
                        debug!("Successfully reported usage for user {}", user_id);
                        // Reset counters after successful report
                        self.reset_user_metrics(user_id);
                    }
                    Err(e) => {
                        // Log error but continue with other users
                        // Metrics will accumulate and be sent on next successful report
                        error!(
                            "Failed to report usage for user {}: {}. Will retry in {} seconds",
                            user_id,
                            e,
                            self.report_interval.as_secs()
                        );
                    }
                }
            }

            debug!("Usage reporting cycle completed");
        }
    }
}

#[derive(Debug, Clone)]
pub struct UsageReporterStats {
    pub total_users: usize,
    pub total_operations: u64,
    pub total_requests: u64,
    pub total_storage_bytes: u64,
    pub report_interval_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_usage_metrics_new() {
        let metrics = UserUsageMetrics::new();
        assert_eq!(metrics.storage_bytes, 0);
        assert_eq!(metrics.operations_count, 0);
        assert_eq!(metrics.api_requests, 0);
    }

    #[test]
    fn test_user_usage_metrics_reset() {
        let mut metrics = UserUsageMetrics {
            storage_bytes: 1000,
            operations_count: 50,
            api_requests: 100,
            last_updated: Instant::now(),
        };

        metrics.reset_counters();
        assert_eq!(metrics.storage_bytes, 1000); // Storage not reset
        assert_eq!(metrics.operations_count, 0);
        assert_eq!(metrics.api_requests, 0);
    }

    #[test]
    fn test_usage_reporter_record_storage() {
        let reporter = UsageReporter::new(Duration::from_secs(300));
        let user_id = Uuid::new_v4();

        reporter.record_storage(&user_id, 5000);

        let metrics = reporter.get_metrics(&user_id).unwrap();
        assert_eq!(metrics.storage_bytes, 5000);
    }

    #[test]
    fn test_usage_reporter_record_operation() {
        let reporter = UsageReporter::new(Duration::from_secs(300));
        let user_id = Uuid::new_v4();

        reporter.record_operation(&user_id);
        reporter.record_operation(&user_id);

        let metrics = reporter.get_metrics(&user_id).unwrap();
        assert_eq!(metrics.operations_count, 2);
    }

    #[test]
    fn test_usage_reporter_record_api_request() {
        let reporter = UsageReporter::new(Duration::from_secs(300));
        let user_id = Uuid::new_v4();

        reporter.record_api_request(&user_id);
        reporter.record_api_request(&user_id);
        reporter.record_api_request(&user_id);

        let metrics = reporter.get_metrics(&user_id).unwrap();
        assert_eq!(metrics.api_requests, 3);
    }

    #[test]
    fn test_usage_reporter_get_all_metrics() {
        let reporter = UsageReporter::new(Duration::from_secs(300));
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        reporter.record_operation(&user1);
        reporter.record_operation(&user2);

        let all_metrics = reporter.get_all_metrics();
        assert_eq!(all_metrics.len(), 2);
        assert!(all_metrics.contains_key(&user1));
        assert!(all_metrics.contains_key(&user2));
    }

    #[test]
    fn test_usage_reporter_reset_user_metrics() {
        let reporter = UsageReporter::new(Duration::from_secs(300));
        let user_id = Uuid::new_v4();

        reporter.record_operation(&user_id);
        reporter.record_api_request(&user_id);
        reporter.record_storage(&user_id, 1000);

        let before = reporter.get_metrics(&user_id).unwrap();
        assert_eq!(before.operations_count, 1);
        assert_eq!(before.api_requests, 1);
        assert_eq!(before.storage_bytes, 1000);

        reporter.reset_user_metrics(&user_id);

        let after = reporter.get_metrics(&user_id).unwrap();
        assert_eq!(after.operations_count, 0);
        assert_eq!(after.api_requests, 0);
        assert_eq!(after.storage_bytes, 1000); // Storage not reset
    }

    #[test]
    fn test_usage_reporter_stats() {
        let reporter = UsageReporter::new(Duration::from_secs(300));
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        reporter.record_operation(&user1);
        reporter.record_operation(&user1);
        reporter.record_operation(&user2);
        reporter.record_storage(&user1, 5000);
        reporter.record_storage(&user2, 3000);

        let stats = reporter.get_stats();
        assert_eq!(stats.total_users, 2);
        assert_eq!(stats.total_operations, 3);
        assert_eq!(stats.total_storage_bytes, 8000);
        assert_eq!(stats.report_interval_seconds, 300);
    }

    #[test]
    fn test_usage_reporter_clear_all() {
        let reporter = UsageReporter::new(Duration::from_secs(300));
        let user_id = Uuid::new_v4();

        reporter.record_operation(&user_id);
        assert!(reporter.get_metrics(&user_id).is_some());

        reporter.clear_all_metrics();
        assert!(reporter.get_metrics(&user_id).is_none());
    }
}
