//! Cluster Topology Management
//!
//! Manages cluster node topology, slot assignments, and node discovery.

use super::types::{
    ClusterError, ClusterNode, ClusterResult, ClusterState, NodeFlags, SlotRange, TOTAL_SLOTS,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, info};

/// Cluster topology manager
pub struct ClusterTopology {
    /// All nodes in cluster
    nodes: Arc<RwLock<HashMap<String, ClusterNode>>>,

    /// Slot assignments (slot -> node_id)
    slot_assignments: Arc<RwLock<HashMap<u16, String>>>,

    /// This node's ID
    my_node_id: String,
}

impl ClusterTopology {
    /// Create new topology manager
    pub fn new(my_node_id: String) -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            slot_assignments: Arc::new(RwLock::new(HashMap::new())),
            my_node_id,
        }
    }

    /// Add a node to the cluster
    pub fn add_node(&self, node: ClusterNode) -> ClusterResult<()> {
        let mut nodes = self.nodes.write();
        if nodes.contains_key(&node.id) {
            return Err(ClusterError::NodeExists(node.id.clone()));
        }

        info!("Adding node to cluster: {} at {}", node.id, node.address);
        nodes.insert(node.id.clone(), node);
        Ok(())
    }

    /// Remove a node from the cluster
    pub fn remove_node(&self, node_id: &str) -> ClusterResult<()> {
        let mut nodes = self.nodes.write();
        let mut slots = self.slot_assignments.write();

        if !nodes.contains_key(node_id) {
            return Err(ClusterError::NodeNotFound(node_id.to_string()));
        }

        // Remove slot assignments for this node
        slots.retain(|_, assigned_node_id| assigned_node_id != node_id);

        info!("Removing node from cluster: {}", node_id);
        nodes.remove(node_id);
        Ok(())
    }

    /// Update node state
    pub fn update_node_state(&self, node_id: &str, state: ClusterState) -> ClusterResult<()> {
        let mut nodes = self.nodes.write();
        if let Some(node) = nodes.get_mut(node_id) {
            node.state = state;
            debug!("Updated node {} state to {:?}", node_id, state);
            Ok(())
        } else {
            Err(super::types::ClusterError::NodeNotFound(
                node_id.to_string(),
            ))
        }
    }

    /// Assign slots to a node
    pub fn assign_slots(&self, node_id: &str, slot_ranges: Vec<SlotRange>) -> ClusterResult<()> {
        let mut nodes = self.nodes.write();
        let mut slots = self.slot_assignments.write();

        if !nodes.contains_key(node_id) {
            return Err(ClusterError::NodeNotFound(node_id.to_string()));
        }

        // Validate slot ranges
        for range in &slot_ranges {
            if range.end >= TOTAL_SLOTS {
                return Err(ClusterError::InvalidSlotRange(range.start, range.end));
            }
        }

        // Remove old assignments for this node
        slots.retain(|_, assigned_node_id| assigned_node_id != node_id);

        // Add new assignments
        for range in &slot_ranges {
            for slot in range.start..=range.end {
                slots.insert(slot, node_id.to_string());
            }
        }

        // Update node's slot list
        if let Some(node) = nodes.get_mut(node_id) {
            node.slots = slot_ranges.clone();
        }

        info!(
            "Assigned {} slot ranges to node {}",
            slot_ranges.len(),
            node_id
        );
        Ok(())
    }

    /// Get node that owns a slot
    pub fn get_slot_owner(&self, slot: u16) -> ClusterResult<String> {
        let slots = self.slot_assignments.read();
        slots
            .get(&slot)
            .cloned()
            .ok_or(super::types::ClusterError::SlotNotAssigned(slot))
    }

    /// Get node information
    pub fn get_node(&self, node_id: &str) -> ClusterResult<ClusterNode> {
        let nodes = self.nodes.read();
        nodes
            .get(node_id)
            .cloned()
            .ok_or_else(|| super::types::ClusterError::NodeNotFound(node_id.to_string()))
    }

    /// Get all nodes
    pub fn get_all_nodes(&self) -> Vec<ClusterNode> {
        let nodes = self.nodes.read();
        nodes.values().cloned().collect()
    }

    /// Get my node ID
    pub fn my_node_id(&self) -> &str {
        &self.my_node_id
    }

    /// Check if cluster has full slot coverage
    pub fn has_full_coverage(&self) -> bool {
        let slots = self.slot_assignments.read();
        slots.len() == TOTAL_SLOTS as usize
    }

    /// Get slot coverage percentage
    pub fn slot_coverage(&self) -> f64 {
        let slots = self.slot_assignments.read();
        (slots.len() as f64 / TOTAL_SLOTS as f64) * 100.0
    }

    /// Initialize cluster with N nodes (for testing/bootstrap)
    pub fn initialize_cluster(&self, node_count: usize) -> ClusterResult<()> {
        if node_count == 0 || node_count > 16384 {
            return Err(ClusterError::ConfigError("Invalid node count".to_string()));
        }

        let slots_per_node = TOTAL_SLOTS / node_count as u16;

        for i in 0..node_count {
            let node_id = format!("node-{}", i);
            let start_slot = (i as u16) * slots_per_node;
            let end_slot = if i == node_count - 1 {
                TOTAL_SLOTS - 1
            } else {
                start_slot + slots_per_node - 1
            };

            let slot_range = SlotRange::new(start_slot, end_slot);

            // Create node (simplified - real implementation would need addresses)
            let node = ClusterNode {
                id: node_id.clone(),
                address: "127.0.0.1:15502".parse().unwrap(),
                state: ClusterState::Connected,
                slots: vec![slot_range],
                master_id: None,
                replica_ids: Vec::new(),
                last_ping: 0,
                flags: NodeFlags {
                    is_master: true,
                    is_myself: i == 0,
                    ..Default::default()
                },
            };

            self.add_node(node)?;
            self.assign_slots(&node_id, vec![slot_range])?;
        }

        info!("Initialized cluster with {} nodes", node_count);
        Ok(())
    }
}

/// Node information (simplified for external use)
#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub id: String,
    pub address: SocketAddr,
    pub state: ClusterState,
    pub slot_count: usize,
}

impl From<&ClusterNode> for NodeInfo {
    fn from(node: &ClusterNode) -> Self {
        Self {
            id: node.id.clone(),
            address: node.address,
            state: node.state,
            slot_count: node.slots.iter().map(|r| r.count() as usize).sum(),
        }
    }
}
