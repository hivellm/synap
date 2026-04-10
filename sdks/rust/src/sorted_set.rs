//! Sorted Set data structure operations
//!
//! Sorted Sets are collections of unique members, each associated with a score.
//! Members are ordered by score, enabling range queries, ranking, and leaderboard functionality.
//!
//! Use cases:
//! - Gaming leaderboards
//! - Priority queues
//! - Rate limiting with timestamps
//! - Time-series data
//! - Auto-complete with relevance scores

use crate::client::SynapClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Sorted Set data structure interface (Redis-compatible)
///
/// Provides scored, ordered collections with ranking and range query capabilities.
#[derive(Clone)]
pub struct SortedSetManager {
    client: SynapClient,
}

/// A member with its score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredMember {
    pub member: String,
    pub score: f64,
}

impl SortedSetManager {
    /// Create a new Sorted Set manager interface
    pub(crate) fn new(client: SynapClient) -> Self {
        Self { client }
    }

    /// Add member with score to sorted set (ZADD)
    ///
    /// # Example
    /// ```no_run
    /// # use synap_sdk::SynapClient;
    /// # async fn example(client: &SynapClient) -> synap_sdk::Result<()> {
    /// client.sorted_set().add("leaderboard", "player1", 100.0).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn add<K, M>(&self, key: K, member: M, score: f64) -> Result<bool>
    where
        K: AsRef<str>,
        M: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "member": member.as_ref(),
            "score": score,
        });

        let response = self.client.send_command("sortedset.zadd", payload).await?;
        Ok(response.get("added").and_then(|v| v.as_u64()).unwrap_or(0) > 0)
    }

    /// Add multiple members with scores to sorted set (ZADD with array)
    ///
    /// # Example
    /// ```no_run
    /// # use synap_sdk::{SynapClient, sorted_set::ScoredMember};
    /// # async fn example(client: &SynapClient) -> synap_sdk::Result<()> {
    /// let members = vec![
    ///     ScoredMember { member: "player1".to_string(), score: 100.0 },
    ///     ScoredMember { member: "player2".to_string(), score: 200.0 },
    /// ];
    /// client.sorted_set().add_multiple("leaderboard", members).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn add_multiple<K>(&self, key: K, members: Vec<ScoredMember>) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "members": members.iter().map(|m| json!({
                "member": m.member,
                "score": m.score,
            })).collect::<Vec<_>>(),
        });

        let response = self.client.send_command("sortedset.zadd", payload).await?;
        Ok(response.get("added").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
    }

    /// Remove members from sorted set (ZREM)
    pub async fn rem<K>(&self, key: K, members: Vec<String>) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "members": members,
        });

        let response = self.client.send_command("sortedset.zrem", payload).await?;
        Ok(response
            .get("removed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize)
    }

    /// Get score of a member (ZSCORE)
    pub async fn score<K, M>(&self, key: K, member: M) -> Result<Option<f64>>
    where
        K: AsRef<str>,
        M: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "member": member.as_ref(),
        });

        let response = self
            .client
            .send_command("sortedset.zscore", payload)
            .await?;
        Ok(response.get("score").and_then(|v| v.as_f64()))
    }

    /// Get cardinality (number of members) (ZCARD)
    pub async fn card<K>(&self, key: K) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({"key": key.as_ref()});
        let response = self.client.send_command("sortedset.zcard", payload).await?;
        Ok(response.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
    }

    /// Increment score of member (ZINCRBY)
    pub async fn incr_by<K, M>(&self, key: K, member: M, increment: f64) -> Result<f64>
    where
        K: AsRef<str>,
        M: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "member": member.as_ref(),
            "increment": increment,
        });

        let response = self
            .client
            .send_command("sortedset.zincrby", payload)
            .await?;
        Ok(response
            .get("score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0))
    }

    /// Get range by rank (0-based index) (ZRANGE)
    ///
    /// # Example
    /// ```no_run
    /// # use synap_sdk::SynapClient;
    /// # async fn example(client: &SynapClient) -> synap_sdk::Result<()> {
    /// // Get top 10 from leaderboard
    /// let top10 = client.sorted_set().range("leaderboard", 0, 9, true).await?;
    /// for member in top10 {
    ///     tracing::info!("{}: {}", member.member, member.score);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn range<K>(
        &self,
        key: K,
        start: i64,
        stop: i64,
        with_scores: bool,
    ) -> Result<Vec<ScoredMember>>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "start": start,
            "stop": stop,
            "withscores": with_scores,
        });

        let response = self
            .client
            .send_command("sortedset.zrange", payload)
            .await?;

        if let Some(members_val) = response.get("members") {
            Ok(serde_json::from_value(members_val.clone()).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }

    /// Get reverse range by rank (highest to lowest) (ZREVRANGE)
    pub async fn rev_range<K>(
        &self,
        key: K,
        start: i64,
        stop: i64,
        with_scores: bool,
    ) -> Result<Vec<ScoredMember>>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "start": start,
            "stop": stop,
            "withscores": with_scores,
        });

        let response = self
            .client
            .send_command("sortedset.zrevrange", payload)
            .await?;

        if let Some(members_val) = response.get("members") {
            Ok(serde_json::from_value(members_val.clone()).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }

    /// Get rank of member (0-based, lowest score = rank 0) (ZRANK)
    pub async fn rank<K, M>(&self, key: K, member: M) -> Result<Option<usize>>
    where
        K: AsRef<str>,
        M: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "member": member.as_ref(),
        });

        let response = self.client.send_command("sortedset.zrank", payload).await?;
        Ok(response
            .get("rank")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize))
    }

    /// Get reverse rank of member (0-based, highest score = rank 0) (ZREVRANK)
    pub async fn rev_rank<K, M>(&self, key: K, member: M) -> Result<Option<usize>>
    where
        K: AsRef<str>,
        M: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "member": member.as_ref(),
        });

        let response = self
            .client
            .send_command("sortedset.zrevrank", payload)
            .await?;
        Ok(response
            .get("rank")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize))
    }

    /// Count members with scores in range (ZCOUNT)
    pub async fn count<K>(&self, key: K, min: f64, max: f64) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "min": min,
            "max": max,
        });

        let response = self
            .client
            .send_command("sortedset.zcount", payload)
            .await?;
        Ok(response.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
    }

    /// Get range by score (ZRANGEBYSCORE)
    pub async fn range_by_score<K>(
        &self,
        key: K,
        min: f64,
        max: f64,
        with_scores: bool,
    ) -> Result<Vec<ScoredMember>>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "min": min,
            "max": max,
            "withscores": with_scores,
        });

        let response = self
            .client
            .send_command("sortedset.zrangebyscore", payload)
            .await?;

        if let Some(members_val) = response.get("members") {
            Ok(serde_json::from_value(members_val.clone()).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }

    /// Pop minimum scored members (ZPOPMIN)
    pub async fn pop_min<K>(&self, key: K, count: usize) -> Result<Vec<ScoredMember>>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "count": count,
        });

        let response = self
            .client
            .send_command("sortedset.zpopmin", payload)
            .await?;

        if let Some(members_val) = response.get("members") {
            Ok(serde_json::from_value(members_val.clone()).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }

    /// Pop maximum scored members (ZPOPMAX)
    pub async fn pop_max<K>(&self, key: K, count: usize) -> Result<Vec<ScoredMember>>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "count": count,
        });

        let response = self
            .client
            .send_command("sortedset.zpopmax", payload)
            .await?;

        if let Some(members_val) = response.get("members") {
            Ok(serde_json::from_value(members_val.clone()).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }

    /// Remove members by rank range (ZREMRANGEBYRANK)
    pub async fn rem_range_by_rank<K>(&self, key: K, start: i64, stop: i64) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "start": start,
            "stop": stop,
        });

        let response = self
            .client
            .send_command("sortedset.zremrangebyrank", payload)
            .await?;
        Ok(response
            .get("removed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize)
    }

    /// Remove members by score range (ZREMRANGEBYSCORE)
    pub async fn rem_range_by_score<K>(&self, key: K, min: f64, max: f64) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "min": min,
            "max": max,
        });

        let response = self
            .client
            .send_command("sortedset.zremrangebyscore", payload)
            .await?;
        Ok(response
            .get("removed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize)
    }

    /// Compute intersection and store in destination (ZINTERSTORE)
    ///
    /// # Example
    /// ```no_run
    /// # use synap_sdk::SynapClient;
    /// # async fn example(client: &SynapClient) -> synap_sdk::Result<()> {
    /// // Intersect two leaderboards
    /// let count = client.sorted_set().inter_store(
    ///     "combined",
    ///     vec!["board1", "board2"],
    ///     None,
    ///     "sum",
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn inter_store<D>(
        &self,
        destination: D,
        keys: Vec<&str>,
        weights: Option<Vec<f64>>,
        aggregate: &str,
    ) -> Result<usize>
    where
        D: AsRef<str>,
    {
        let payload = json!({
            "destination": destination.as_ref(),
            "keys": keys,
            "weights": weights,
            "aggregate": aggregate,
        });

        let response = self
            .client
            .send_command("sortedset.zinterstore", payload)
            .await?;
        Ok(response.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
    }

    /// Compute union and store in destination (ZUNIONSTORE)
    pub async fn union_store<D>(
        &self,
        destination: D,
        keys: Vec<&str>,
        weights: Option<Vec<f64>>,
        aggregate: &str,
    ) -> Result<usize>
    where
        D: AsRef<str>,
    {
        let payload = json!({
            "destination": destination.as_ref(),
            "keys": keys,
            "weights": weights,
            "aggregate": aggregate,
        });

        let response = self
            .client
            .send_command("sortedset.zunionstore", payload)
            .await?;
        Ok(response.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
    }

    /// Compute difference and store in destination (ZDIFFSTORE)
    pub async fn diff_store<D>(&self, destination: D, keys: Vec<&str>) -> Result<usize>
    where
        D: AsRef<str>,
    {
        let payload = json!({
            "destination": destination.as_ref(),
            "keys": keys,
        });

        let response = self
            .client
            .send_command("sortedset.zdiffstore", payload)
            .await?;
        Ok(response.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
    }

    /// Get statistics
    pub async fn stats(&self) -> Result<SortedSetStats> {
        let payload = json!({});
        let response = self.client.send_command("sortedset.stats", payload).await?;
        Ok(serde_json::from_value(response).unwrap_or_default())
    }
}

/// Statistics for sorted sets
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SortedSetStats {
    pub total_keys: usize,
    pub total_members: usize,
    pub avg_members_per_key: f64,
    pub memory_bytes: usize,
}
