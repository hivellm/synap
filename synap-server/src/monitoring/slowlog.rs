//! Slow Query Logging
//!
//! Tracks commands that exceed a configurable time threshold

use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Slow log entry
#[derive(Debug, Clone, Serialize)]
pub struct SlowLogEntry {
    pub id: u64,
    pub timestamp: u64,
    pub duration_us: u64,
    pub command: String,
    pub args: Vec<String>,
}

impl SlowLogEntry {
    fn new(id: u64, command: String, args: Vec<String>, duration: Duration) -> Self {
        Self {
            id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            duration_us: duration.as_micros() as u64,
            command,
            args,
        }
    }
}

/// Slow log configuration
#[derive(Debug, Clone)]
pub struct SlowLogConfig {
    pub threshold_ms: u64,
    pub max_entries: usize,
}

impl Default for SlowLogConfig {
    fn default() -> Self {
        Self {
            threshold_ms: 10, // 10ms default (Redis default is 10000 microseconds = 10ms)
            max_entries: 128, // Keep last 128 slow queries
        }
    }
}

/// Slow log manager
pub struct SlowLogManager {
    entries: Arc<RwLock<Vec<SlowLogEntry>>>,
    config: SlowLogConfig,
    next_id: Arc<RwLock<u64>>,
}

impl Default for SlowLogManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SlowLogManager {
    /// Create a new slow log manager
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            config: SlowLogConfig::default(),
            next_id: Arc::new(RwLock::new(0)),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: SlowLogConfig) -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            config,
            next_id: Arc::new(RwLock::new(0)),
        }
    }

    /// Record a command execution if it exceeds threshold
    pub async fn record(&self, command: String, args: Vec<String>, duration: Duration) {
        let ultra_ms = duration.as_millis() as u64;

        if ultra_ms < self.config.threshold_ms {
            return; // Not slow enough
        }

        let mut entries = self.entries.write().await;
        let mut next_id = self.next_id.write().await;

        let entry = SlowLogEntry::new(*next_id, command, args, duration);
        *next_id += 1;

        entries.push(entry);

        // Keep only the last N entries
        if entries.len() > self.config.max_entries {
            entries.remove(0);
        }
    }

    /// Get slow log entries (most recent first)
    pub async fn get(&self, count: Option<usize>) -> Vec<SlowLogEntry> {
        let entries = self.entries.read().await;
        let take = count.unwrap_or(entries.len());

        // Return most recent entries (reversed)
        entries.iter().rev().take(take).cloned().collect()
    }

    /// Reset slow log
    pub async fn reset(&self) -> usize {
        let mut entries = self.entries.write().await;
        let count = entries.len();
        entries.clear();
        count
    }

    /// Get slow log length
    pub async fn len(&self) -> usize {
        self.entries.read().await.len()
    }

    /// Check if slow log is empty
    pub async fn is_empty(&self) -> bool {
        self.entries.read().await.is_empty()
    }

    /// Get configuration
    pub fn config(&self) -> &SlowLogConfig {
        &self.config
    }
}

impl Clone for SlowLogManager {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            config: self.config.clone(),
            next_id: self.next_id.clone(),
        }
    }
}

/// SlowLog response for API
#[derive(Debug, Serialize)]
pub struct SlowLog {
    pub entries: Vec<SlowLogEntry>,
    pub total: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_slowlog_new() {
        let slowlog = SlowLogManager::new();
        assert_eq!(slowlog.config().threshold_ms, 10);
        assert_eq!(slowlog.config().max_entries, 128);
        assert_eq!(slowlog.len().await, 0);
        assert!(slowlog.is_empty().await);
    }

    #[tokio::test]
    async fn test_slowlog_with_config() {
        let config = SlowLogConfig {
            threshold_ms: 5,
            max_entries: 64,
        };
        let slowlog = SlowLogManager::with_config(config);
        assert_eq!(slowlog.config().threshold_ms, 5);
        assert_eq!(slowlog.config().max_entries, 64);
    }

    #[tokio::test]
    async fn test_slowlog_record_below_threshold() {
        let slowlog = SlowLogManager::new();
        slowlog
            .record("test".to_string(), vec![], Duration::from_millis(5))
            .await;
        assert_eq!(slowlog.len().await, 0);
    }

    #[tokio::test]
    async fn test_slowlog_record_above_threshold() {
        let slowlog = SlowLogManager::new();
        slowlog
            .record(
                "test".to_string(),
                vec!["arg1".to_string()],
                Duration::from_millis(15),
            )
            .await;
        assert_eq!(slowlog.len().await, 1);

        let entries = slowlog.get(None).await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "test");
        assert_eq!(entries[0].args, vec!["arg1"]);
        assert!(entries[0].duration_us >= 15000);
    }

    #[tokio::test]
    async fn test_slowlog_get_with_count() {
        let slowlog = SlowLogManager::new();

        // Record multiple entries
        for i in 0..5 {
            slowlog
                .record(format!("cmd{}", i), vec![], Duration::from_millis(15))
                .await;
        }

        assert_eq!(slowlog.len().await, 5);

        // Get only 3 entries
        let entries = slowlog.get(Some(3)).await;
        assert_eq!(entries.len(), 3);

        // Should be most recent (reversed order)
        assert_eq!(entries[0].command, "cmd4");
        assert_eq!(entries[1].command, "cmd3");
        assert_eq!(entries[2].command, "cmd2");
    }

    #[tokio::test]
    async fn test_slowlog_reset() {
        let slowlog = SlowLogManager::new();

        slowlog
            .record("test".to_string(), vec![], Duration::from_millis(15))
            .await;
        assert_eq!(slowlog.len().await, 1);

        let count = slowlog.reset().await;
        assert_eq!(count, 1);
        assert_eq!(slowlog.len().await, 0);
        assert!(slowlog.is_empty().await);
    }

    #[tokio::test]
    async fn test_slowlog_max_entries() {
        let config = SlowLogConfig {
            threshold_ms: 1,
            max_entries: 3,
        };
        let slowlog = SlowLogManager::with_config(config);

        // Record more than max_entries
        for i in 0..5 {
            slowlog
                .record(format!("cmd{}", i), vec![], Duration::from_millis(2))
                .await;
        }

        // Should only keep last 3 entries
        assert_eq!(slowlog.len().await, 3);
        let entries = slowlog.get(None).await;
        assert_eq!(entries[0].command, "cmd4");
        assert_eq!(entries[1].command, "cmd3");
        assert_eq!(entries[2].command, "cmd2");
    }

    #[tokio::test]
    async fn test_slowlog_entry_ids() {
        let slowlog = SlowLogManager::new();

        for i in 0..3 {
            slowlog
                .record(format!("cmd{}", i), vec![], Duration::from_millis(15))
                .await;
        }

        let entries = slowlog.get(None).await;
        assert_eq!(entries[0].id, 2);
        assert_eq!(entries[1].id, 1);
        assert_eq!(entries[2].id, 0);
    }
}
