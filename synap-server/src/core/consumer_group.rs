use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Consumer group member
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerMember {
    /// Member ID
    pub id: String,
    /// Group ID
    pub group_id: String,
    /// Assigned partitions
    pub partitions: Vec<usize>,
    /// Last heartbeat time
    #[serde(skip, default = "Instant::now")]
    pub last_heartbeat: Instant,
    /// Session timeout in seconds
    pub session_timeout_secs: u64,
    /// Metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl ConsumerMember {
    pub fn new(group_id: String, session_timeout_secs: u64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            group_id,
            partitions: Vec::new(),
            last_heartbeat: Instant::now(),
            session_timeout_secs,
            metadata: HashMap::new(),
        }
    }

    /// Check if member is alive
    pub fn is_alive(&self) -> bool {
        self.last_heartbeat.elapsed() < Duration::from_secs(self.session_timeout_secs)
    }

    /// Update heartbeat
    pub fn heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
    }
}

/// Partition assignment strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AssignmentStrategy {
    /// Round-robin assignment
    RoundRobin,
    /// Range-based assignment (partitions 0-2 to consumer 1, 3-5 to consumer 2, etc.)
    Range,
    /// Sticky assignment (minimize partition movement on rebalance)
    Sticky,
}

impl Default for AssignmentStrategy {
    fn default() -> Self {
        AssignmentStrategy::RoundRobin
    }
}

/// Consumer group configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerGroupConfig {
    /// Assignment strategy
    pub strategy: AssignmentStrategy,
    /// Session timeout in seconds
    pub session_timeout_secs: u64,
    /// Rebalance timeout in seconds
    pub rebalance_timeout_secs: u64,
    /// Enable auto-commit of offsets
    pub auto_commit: bool,
    /// Auto-commit interval in seconds
    pub auto_commit_interval_secs: u64,
}

impl Default for ConsumerGroupConfig {
    fn default() -> Self {
        Self {
            strategy: AssignmentStrategy::RoundRobin,
            session_timeout_secs: 30,
            rebalance_timeout_secs: 60,
            auto_commit: true,
            auto_commit_interval_secs: 5,
        }
    }
}

/// Consumer group state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GroupState {
    /// Group is empty, no members
    Empty,
    /// Group is stable, partitions assigned
    Stable,
    /// Group is rebalancing
    Rebalancing,
    /// Group is dead (marked for deletion)
    Dead,
}

/// Partition offset tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionOffset {
    pub partition_id: usize,
    pub offset: u64,
    pub timestamp: u64,
    pub metadata: String,
}

/// Consumer group
pub struct ConsumerGroup {
    /// Group ID
    id: String,
    /// Topic name
    topic: String,
    /// Total partitions in topic
    partition_count: usize,
    /// Members in group
    members: HashMap<String, ConsumerMember>,
    /// Current state
    state: GroupState,
    /// Configuration
    config: ConsumerGroupConfig,
    /// Committed offsets per partition
    committed_offsets: HashMap<usize, u64>,
    /// Last rebalance time
    last_rebalance: Instant,
    /// Generation ID (increments on each rebalance)
    generation: u64,
}

impl ConsumerGroup {
    pub fn new(id: String, topic: String, partition_count: usize, config: ConsumerGroupConfig) -> Self {
        Self {
            id,
            topic,
            partition_count,
            members: HashMap::new(),
            state: GroupState::Empty,
            config,
            committed_offsets: HashMap::new(),
            last_rebalance: Instant::now(),
            generation: 0,
        }
    }

    /// Join the consumer group
    pub fn join(&mut self, session_timeout_secs: u64) -> ConsumerMember {
        let mut member = ConsumerMember::new(self.id.clone(), session_timeout_secs);
        member.heartbeat();

        self.members.insert(member.id.clone(), member.clone());
        self.state = GroupState::Rebalancing;

        member
    }

    /// Leave the consumer group
    pub fn leave(&mut self, member_id: &str) -> Result<(), String> {
        if self.members.remove(member_id).is_some() {
            self.state = GroupState::Rebalancing;
            Ok(())
        } else {
            Err(format!("Member {} not found in group", member_id))
        }
    }

    /// Heartbeat from a member
    pub fn heartbeat(&mut self, member_id: &str) -> Result<(), String> {
        if let Some(member) = self.members.get_mut(member_id) {
            member.heartbeat();
            Ok(())
        } else {
            Err(format!("Member {} not found in group", member_id))
        }
    }

    /// Rebalance partitions among members
    pub fn rebalance(&mut self) -> Result<(), String> {
        // Remove dead members
        self.members.retain(|_, m| m.is_alive());

        if self.members.is_empty() {
            self.state = GroupState::Empty;
            self.generation += 1;
            return Ok(());
        }

        self.state = GroupState::Rebalancing;

        // Assign partitions based on strategy
        match self.config.strategy {
            AssignmentStrategy::RoundRobin => self.assign_round_robin(),
            AssignmentStrategy::Range => self.assign_range(),
            AssignmentStrategy::Sticky => self.assign_sticky(),
        }

        self.state = GroupState::Stable;
        self.last_rebalance = Instant::now();
        self.generation += 1;

        Ok(())
    }

    /// Round-robin partition assignment
    fn assign_round_robin(&mut self) {
        let member_ids: Vec<String> = self.members.keys().cloned().collect();
        let member_count = member_ids.len();

        // Clear existing assignments
        for member in self.members.values_mut() {
            member.partitions.clear();
        }

        // Assign partitions in round-robin fashion
        for partition_id in 0..self.partition_count {
            let member_idx = partition_id % member_count;
            let member_id = &member_ids[member_idx];

            if let Some(member) = self.members.get_mut(member_id) {
                member.partitions.push(partition_id);
            }
        }
    }

    /// Range-based partition assignment
    fn assign_range(&mut self) {
        let member_ids: Vec<String> = self.members.keys().cloned().collect();
        let member_count = member_ids.len();

        // Clear existing assignments
        for member in self.members.values_mut() {
            member.partitions.clear();
        }

        let partitions_per_member = self.partition_count / member_count;
        let extra_partitions = self.partition_count % member_count;

        let mut current_partition = 0;

        for (idx, member_id) in member_ids.iter().enumerate() {
            let count = if idx < extra_partitions {
                partitions_per_member + 1
            } else {
                partitions_per_member
            };

            if let Some(member) = self.members.get_mut(member_id) {
                for _ in 0..count {
                    if current_partition < self.partition_count {
                        member.partitions.push(current_partition);
                        current_partition += 1;
                    }
                }
            }
        }
    }

    /// Sticky partition assignment (minimize movement)
    fn assign_sticky(&mut self) {
        let member_ids: Vec<String> = self.members.keys().cloned().collect();
        let member_count = member_ids.len();

        // Collect currently assigned partitions
        let mut assigned: HashSet<usize> = HashSet::new();
        for member in self.members.values() {
            for &partition_id in &member.partitions {
                if partition_id < self.partition_count {
                    assigned.insert(partition_id);
                }
            }
        }

        // Find unassigned partitions
        let mut unassigned: Vec<usize> = (0..self.partition_count)
            .filter(|p| !assigned.contains(p))
            .collect();

        // Distribute unassigned partitions
        for (idx, partition_id) in unassigned.drain(..).enumerate() {
            let member_idx = idx % member_count;
            let member_id = &member_ids[member_idx];

            if let Some(member) = self.members.get_mut(member_id) {
                member.partitions.push(partition_id);
            }
        }

        // Rebalance if distribution is too uneven
        let target = self.partition_count / member_count;
        let mut overloaded: Vec<(String, Vec<usize>)> = Vec::new();

        for (id, member) in &self.members {
            if member.partitions.len() > target + 1 {
                let excess: Vec<usize> = member.partitions[target + 1..].to_vec();
                overloaded.push((id.clone(), excess));
            }
        }

        // Redistribute excess partitions
        for (overloaded_id, excess_partitions) in overloaded {
            for partition_id in excess_partitions {
                // Remove from overloaded member
                if let Some(member) = self.members.get_mut(&overloaded_id) {
                    member.partitions.retain(|&p| p != partition_id);
                }

                // Find underloaded member
                let underloaded = member_ids
                    .iter()
                    .find(|id| {
                        self.members
                            .get(*id)
                            .map(|m| m.partitions.len() < target)
                            .unwrap_or(false)
                    });

                if let Some(underloaded_id) = underloaded {
                    if let Some(member) = self.members.get_mut(underloaded_id) {
                        member.partitions.push(partition_id);
                    }
                }
            }
        }
    }

    /// Get assignment for a member
    pub fn get_assignment(&self, member_id: &str) -> Result<Vec<usize>, String> {
        self.members
            .get(member_id)
            .map(|m| m.partitions.clone())
            .ok_or_else(|| format!("Member {} not found", member_id))
    }

    /// Commit offset for a partition
    pub fn commit_offset(&mut self, partition_id: usize, offset: u64) {
        self.committed_offsets.insert(partition_id, offset);
    }

    /// Get committed offset for a partition
    pub fn get_offset(&self, partition_id: usize) -> Option<u64> {
        self.committed_offsets.get(&partition_id).copied()
    }

    /// Get group state
    pub fn state(&self) -> GroupState {
        self.state.clone()
    }

    /// Get group statistics
    pub fn stats(&self) -> ConsumerGroupStats {
        ConsumerGroupStats {
            group_id: self.id.clone(),
            topic: self.topic.clone(),
            state: self.state.clone(),
            member_count: self.members.len(),
            generation: self.generation,
            partition_count: self.partition_count,
            committed_partitions: self.committed_offsets.len(),
            last_rebalance_secs: self.last_rebalance.elapsed().as_secs(),
        }
    }

    /// Get all members
    pub fn members(&self) -> Vec<ConsumerMember> {
        self.members.values().cloned().collect()
    }

    /// Check if group needs rebalancing
    pub fn needs_rebalance(&self) -> bool {
        // Check for dead members
        for member in self.members.values() {
            if !member.is_alive() {
                return true;
            }
        }

        // Check if in rebalancing state
        matches!(self.state, GroupState::Rebalancing)
    }
}

/// Consumer group statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerGroupStats {
    pub group_id: String,
    pub topic: String,
    pub state: GroupState,
    pub member_count: usize,
    pub generation: u64,
    pub partition_count: usize,
    pub committed_partitions: usize,
    pub last_rebalance_secs: u64,
}

/// Consumer group manager
#[derive(Clone)]
pub struct ConsumerGroupManager {
    groups: Arc<RwLock<HashMap<String, ConsumerGroup>>>,
    default_config: ConsumerGroupConfig,
}

impl ConsumerGroupManager {
    pub fn new(default_config: ConsumerGroupConfig) -> Self {
        Self {
            groups: Arc::new(RwLock::new(HashMap::new())),
            default_config,
        }
    }

    /// Create a new consumer group
    pub async fn create_group(
        &self,
        group_id: &str,
        topic: &str,
        partition_count: usize,
        config: Option<ConsumerGroupConfig>,
    ) -> Result<(), String> {
        let mut groups = self.groups.write();

        if groups.contains_key(group_id) {
            return Err(format!("Consumer group '{}' already exists", group_id));
        }

        let group_config = config.unwrap_or_else(|| self.default_config.clone());
        groups.insert(
            group_id.to_string(),
            ConsumerGroup::new(group_id.to_string(), topic.to_string(), partition_count, group_config),
        );

        Ok(())
    }

    /// Join a consumer group
    pub async fn join_group(
        &self,
        group_id: &str,
        session_timeout_secs: u64,
    ) -> Result<ConsumerMember, String> {
        let mut groups = self.groups.write();

        let group = groups
            .get_mut(group_id)
            .ok_or_else(|| format!("Consumer group '{}' not found", group_id))?;

        Ok(group.join(session_timeout_secs))
    }

    /// Leave a consumer group
    pub async fn leave_group(&self, group_id: &str, member_id: &str) -> Result<(), String> {
        let mut groups = self.groups.write();

        let group = groups
            .get_mut(group_id)
            .ok_or_else(|| format!("Consumer group '{}' not found", group_id))?;

        group.leave(member_id)
    }

    /// Heartbeat from a member
    pub async fn heartbeat(&self, group_id: &str, member_id: &str) -> Result<(), String> {
        let mut groups = self.groups.write();

        let group = groups
            .get_mut(group_id)
            .ok_or_else(|| format!("Consumer group '{}' not found", group_id))?;

        group.heartbeat(member_id)
    }

    /// Trigger rebalance for a group
    pub async fn rebalance_group(&self, group_id: &str) -> Result<(), String> {
        let mut groups = self.groups.write();

        let group = groups
            .get_mut(group_id)
            .ok_or_else(|| format!("Consumer group '{}' not found", group_id))?;

        group.rebalance()
    }

    /// Get partition assignment for a member
    pub async fn get_assignment(&self, group_id: &str, member_id: &str) -> Result<Vec<usize>, String> {
        let groups = self.groups.read();

        let group = groups
            .get(group_id)
            .ok_or_else(|| format!("Consumer group '{}' not found", group_id))?;

        group.get_assignment(member_id)
    }

    /// Commit offset
    pub async fn commit_offset(
        &self,
        group_id: &str,
        partition_id: usize,
        offset: u64,
    ) -> Result<(), String> {
        let mut groups = self.groups.write();

        let group = groups
            .get_mut(group_id)
            .ok_or_else(|| format!("Consumer group '{}' not found", group_id))?;

        group.commit_offset(partition_id, offset);
        Ok(())
    }

    /// Get committed offset
    pub async fn get_offset(&self, group_id: &str, partition_id: usize) -> Result<Option<u64>, String> {
        let groups = self.groups.read();

        let group = groups
            .get(group_id)
            .ok_or_else(|| format!("Consumer group '{}' not found", group_id))?;

        Ok(group.get_offset(partition_id))
    }

    /// Get group statistics
    pub async fn group_stats(&self, group_id: &str) -> Result<ConsumerGroupStats, String> {
        let groups = self.groups.read();

        groups
            .get(group_id)
            .ok_or_else(|| format!("Consumer group '{}' not found", group_id))
            .map(|g| g.stats())
    }

    /// List all groups
    pub async fn list_groups(&self) -> Vec<String> {
        let groups = self.groups.read();
        groups.keys().cloned().collect()
    }

    /// Delete a group
    pub async fn delete_group(&self, group_id: &str) -> Result<(), String> {
        let mut groups = self.groups.write();

        if groups.remove(group_id).is_some() {
            Ok(())
        } else {
            Err(format!("Consumer group '{}' not found", group_id))
        }
    }

    /// Start background rebalancing task
    pub fn start_rebalance_task(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                interval.tick().await;

                let group_ids: Vec<String> = {
                    let groups = self.groups.read();
                    groups.keys().cloned().collect()
                };

                for group_id in group_ids {
                    let needs_rebalance = {
                        let groups = self.groups.read();
                        groups
                            .get(&group_id)
                            .map(|g| g.needs_rebalance())
                            .unwrap_or(false)
                    };

                    if needs_rebalance {
                        let _ = self.rebalance_group(&group_id).await;
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_consumer_group_creation() {
        let manager = ConsumerGroupManager::new(ConsumerGroupConfig::default());

        manager
            .create_group("test-group", "test-topic", 6, None)
            .await
            .unwrap();

        let groups = manager.list_groups().await;
        assert_eq!(groups.len(), 1);
    }

    #[tokio::test]
    async fn test_consumer_join_leave() {
        let manager = ConsumerGroupManager::new(ConsumerGroupConfig::default());
        manager
            .create_group("group1", "topic1", 6, None)
            .await
            .unwrap();

        let member = manager.join_group("group1", 30).await.unwrap();
        assert_eq!(member.group_id, "group1");

        manager.leave_group("group1", &member.id).await.unwrap();
    }

    #[tokio::test]
    async fn test_round_robin_assignment() {
        let config = ConsumerGroupConfig {
            strategy: AssignmentStrategy::RoundRobin,
            ..Default::default()
        };

        let manager = ConsumerGroupManager::new(config);
        manager.create_group("rr-group", "topic", 6, None).await.unwrap();

        // Join 3 members
        let m1 = manager.join_group("rr-group", 30).await.unwrap();
        let m2 = manager.join_group("rr-group", 30).await.unwrap();
        let m3 = manager.join_group("rr-group", 30).await.unwrap();

        // Trigger rebalance
        manager.rebalance_group("rr-group").await.unwrap();

        // Each member should have 2 partitions
        let a1 = manager.get_assignment("rr-group", &m1.id).await.unwrap();
        let a2 = manager.get_assignment("rr-group", &m2.id).await.unwrap();
        let a3 = manager.get_assignment("rr-group", &m3.id).await.unwrap();

        assert_eq!(a1.len(), 2);
        assert_eq!(a2.len(), 2);
        assert_eq!(a3.len(), 2);
    }

    #[tokio::test]
    async fn test_range_assignment() {
        let config = ConsumerGroupConfig {
            strategy: AssignmentStrategy::Range,
            ..Default::default()
        };

        let manager = ConsumerGroupManager::new(config);
        manager.create_group("range-group", "topic", 7, None).await.unwrap();

        // Join 3 members
        let m1 = manager.join_group("range-group", 30).await.unwrap();
        let m2 = manager.join_group("range-group", 30).await.unwrap();
        let m3 = manager.join_group("range-group", 30).await.unwrap();

        manager.rebalance_group("range-group").await.unwrap();

        let a1 = manager.get_assignment("range-group", &m1.id).await.unwrap();
        let a2 = manager.get_assignment("range-group", &m2.id).await.unwrap();
        let a3 = manager.get_assignment("range-group", &m3.id).await.unwrap();

        // Total should be 7
        assert_eq!(a1.len() + a2.len() + a3.len(), 7);
    }

    #[tokio::test]
    async fn test_offset_commit() {
        let manager = ConsumerGroupManager::new(ConsumerGroupConfig::default());
        manager.create_group("offset-group", "topic", 3, None).await.unwrap();

        // Commit offsets
        manager.commit_offset("offset-group", 0, 100).await.unwrap();
        manager.commit_offset("offset-group", 1, 200).await.unwrap();

        // Get offsets
        let offset0 = manager.get_offset("offset-group", 0).await.unwrap();
        let offset1 = manager.get_offset("offset-group", 1).await.unwrap();

        assert_eq!(offset0, Some(100));
        assert_eq!(offset1, Some(200));
    }

    #[tokio::test]
    async fn test_heartbeat() {
        let manager = ConsumerGroupManager::new(ConsumerGroupConfig::default());
        manager.create_group("hb-group", "topic", 3, None).await.unwrap();

        let member = manager.join_group("hb-group", 30).await.unwrap();

        // Heartbeat should succeed
        manager.heartbeat("hb-group", &member.id).await.unwrap();
    }

    #[tokio::test]
    async fn test_rebalance_on_member_leave() {
        let manager = ConsumerGroupManager::new(ConsumerGroupConfig::default());
        manager.create_group("rb-group", "topic", 6, None).await.unwrap();

        let m1 = manager.join_group("rb-group", 30).await.unwrap();
        let m2 = manager.join_group("rb-group", 30).await.unwrap();

        manager.rebalance_group("rb-group").await.unwrap();

        // Member 2 leaves
        manager.leave_group("rb-group", &m2.id).await.unwrap();
        manager.rebalance_group("rb-group").await.unwrap();

        // Member 1 should have all partitions
        let assignment = manager.get_assignment("rb-group", &m1.id).await.unwrap();
        assert_eq!(assignment.len(), 6);
    }
}

