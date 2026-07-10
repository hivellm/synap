use super::*;

/// GET /cluster/info - Get cluster information
pub async fn cluster_info(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST GET /cluster/info");

    // Check permission
    require_permission(&ctx, "cluster:info", Action::Read)?;

    let topology = state
        .cluster_topology
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Cluster mode not enabled".to_string()))?;

    let nodes = topology.get_all_nodes();
    let node_count = nodes.len();
    let slot_coverage = topology.slot_coverage();
    let has_full_coverage = topology.has_full_coverage();

    Ok(Json(json!({
        "state": if has_full_coverage { "ok" } else { "fail" },
        "slot_assignment": if has_full_coverage { "complete" } else { "incomplete" },
        "slots": {
            "assigned": (slot_coverage / 100.0 * 16384.0) as u32,
            "total": 16384,
            "coverage": slot_coverage
        },
        "nodes": {
            "count": node_count,
            "my_node_id": topology.my_node_id()
        },
        "cluster_enabled": true
    })))
}

/// GET /cluster/nodes - List all nodes
pub async fn cluster_nodes(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST GET /cluster/nodes");

    // Check permission
    require_permission(&ctx, "cluster:nodes", Action::Read)?;

    let topology = state
        .cluster_topology
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Cluster mode not enabled".to_string()))?;

    let nodes: Vec<serde_json::Value> = topology
        .get_all_nodes()
        .iter()
        .map(|node| {
            use crate::cluster::topology::NodeInfo;
            let info = NodeInfo::from(node);
            json!({
                "id": info.id,
                "address": info.address.to_string(),
                "state": format!("{:?}", info.state),
                "slot_count": info.slot_count,
                "is_master": node.flags.is_master,
                "is_replica": node.flags.is_replica,
                "is_myself": node.flags.is_myself
            })
        })
        .collect();

    Ok(Json(json!({
        "nodes": nodes,
        "count": nodes.len()
    })))
}

/// GET /cluster/nodes/{node_id} - Get node information
pub async fn cluster_node_info(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
    Path(node_id): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST GET /cluster/nodes/{}", node_id);

    // Check permission
    require_permission(&ctx, "cluster:nodes", Action::Read)?;

    let topology = state
        .cluster_topology
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Cluster mode not enabled".to_string()))?;

    let node = topology
        .get_node(&node_id)
        .map_err(|_| SynapError::NotFound)?;

    use crate::cluster::topology::NodeInfo;
    let info = NodeInfo::from(&node);

    Ok(Json(json!({
        "id": info.id,
        "address": info.address.to_string(),
        "state": format!("{:?}", info.state),
        "slot_count": info.slot_count,
        "slots": node.slots.iter().map(|r| json!({
            "start": r.start,
            "end": r.end,
            "count": r.count()
        })).collect::<Vec<_>>(),
        "is_master": node.flags.is_master,
        "is_replica": node.flags.is_replica,
        "is_myself": node.flags.is_myself,
        "master_id": node.master_id,
        "replica_ids": node.replica_ids
    })))
}

/// GET /cluster/slots - Get slot assignments
pub async fn cluster_slots(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST GET /cluster/slots");

    // Check permission
    require_permission(&ctx, "cluster:slots", Action::Read)?;

    let topology = state
        .cluster_topology
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Cluster mode not enabled".to_string()))?;

    use crate::cluster::types::TOTAL_SLOTS;

    // Build slot assignments array
    let mut slots = Vec::new();
    for slot in 0..TOTAL_SLOTS {
        if let Ok(owner) = topology.get_slot_owner(slot) {
            slots.push(json!({
                "slot": slot,
                "owner": owner
            }));
        }
    }

    Ok(Json(json!({
        "slots": slots,
        "total": TOTAL_SLOTS,
        "assigned": slots.len(),
        "coverage": topology.slot_coverage()
    })))
}

/// Request type for adding a node
#[derive(Debug, Deserialize)]
pub struct AddNodeRequest {
    pub node_id: String,
    pub address: String,
}

/// POST /cluster/nodes - Add a node to cluster
pub async fn cluster_add_node(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
    Json(req): Json<AddNodeRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST POST /cluster/nodes: node_id={}, address={}",
        req.node_id, req.address
    );

    // Check permission
    require_permission(&ctx, "cluster:nodes", Action::Write)?;

    let topology = state
        .cluster_topology
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Cluster mode not enabled".to_string()))?;

    let address: SocketAddr = req
        .address
        .parse()
        .map_err(|_| SynapError::InvalidRequest("Invalid address format".to_string()))?;

    let node = crate::cluster::types::ClusterNode {
        id: req.node_id.clone(),
        address,
        state: crate::cluster::types::ClusterState::Joining,
        slots: Vec::new(),
        master_id: None,
        replica_ids: Vec::new(),
        last_ping: 0,
        flags: crate::cluster::types::NodeFlags::default(),
    };

    topology
        .add_node(node)
        .map_err(|e| SynapError::InternalError(format!("Failed to add node: {}", e)))?;

    Ok(Json(json!({
        "success": true,
        "node_id": req.node_id,
        "message": "Node added successfully"
    })))
}

/// DELETE /cluster/nodes/{node_id} - Remove a node from cluster
pub async fn cluster_remove_node(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
    Path(node_id): Path<String>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST DELETE /cluster/nodes/{}", node_id);

    // Check permission
    require_permission(&ctx, "cluster:nodes", Action::Delete)?;

    let topology = state
        .cluster_topology
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Cluster mode not enabled".to_string()))?;

    topology
        .remove_node(&node_id)
        .map_err(|e| SynapError::InternalError(format!("Failed to remove node: {}", e)))?;

    Ok(Json(json!({
        "success": true,
        "node_id": node_id,
        "message": "Node removed successfully"
    })))
}

/// Request type for assigning slots
#[derive(Debug, Deserialize)]
pub struct AssignSlotsRequest {
    pub node_id: String,
    pub slots: Vec<SlotRangeRequest>,
}

#[derive(Debug, Deserialize)]
pub struct SlotRangeRequest {
    pub start: u16,
    pub end: u16,
}

/// POST /cluster/slots/assign - Assign slots to a node
pub async fn cluster_assign_slots(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
    Json(req): Json<AssignSlotsRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST POST /cluster/slots/assign: node_id={}, slots={}",
        req.node_id,
        req.slots.len()
    );

    // Check permission
    require_permission(&ctx, "cluster:slots", Action::Write)?;

    let topology = state
        .cluster_topology
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Cluster mode not enabled".to_string()))?;

    let slot_ranges: Vec<crate::cluster::types::SlotRange> = req
        .slots
        .iter()
        .map(|r| crate::cluster::types::SlotRange::new(r.start, r.end))
        .collect();

    topology
        .assign_slots(&req.node_id, slot_ranges.clone())
        .map_err(|e| SynapError::InternalError(format!("Failed to assign slots: {}", e)))?;

    Ok(Json(json!({
        "success": true,
        "node_id": req.node_id,
        "slots_assigned": slot_ranges.len(),
        "message": "Slots assigned successfully"
    })))
}

/// Request type for starting migration
#[derive(Debug, Deserialize)]
pub struct StartMigrationRequest {
    pub slot: u16,
    pub from_node: String,
    pub to_node: String,
}

/// POST /cluster/migration/start - Start slot migration
pub async fn cluster_start_migration(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
    Json(req): Json<StartMigrationRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!(
        "REST POST /cluster/migration/start: slot={}, from={}, to={}",
        req.slot, req.from_node, req.to_node
    );

    // Check permission
    require_permission(&ctx, "cluster:migration", Action::Write)?;

    let migration = state
        .cluster_migration
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Cluster mode not enabled".to_string()))?;

    migration
        .start_migration(req.slot, req.from_node.clone(), req.to_node.clone())
        .map_err(|e| SynapError::InternalError(format!("Failed to start migration: {}", e)))?;

    Ok(Json(json!({
        "success": true,
        "slot": req.slot,
        "from_node": req.from_node,
        "to_node": req.to_node,
        "message": "Migration started successfully"
    })))
}

/// Request type for completing migration
#[derive(Debug, Deserialize)]
pub struct CompleteMigrationRequest {
    pub slot: u16,
}

/// POST /cluster/migration/complete - Complete slot migration
pub async fn cluster_complete_migration(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
    Json(req): Json<CompleteMigrationRequest>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST POST /cluster/migration/complete: slot={}", req.slot);

    // Check permission
    require_permission(&ctx, "cluster:migration", Action::Write)?;

    let migration = state
        .cluster_migration
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Cluster mode not enabled".to_string()))?;

    migration
        .complete_migration(req.slot)
        .map_err(|e| SynapError::InternalError(format!("Failed to complete migration: {}", e)))?;

    Ok(Json(json!({
        "success": true,
        "slot": req.slot,
        "message": "Migration completed successfully"
    })))
}

/// GET /cluster/migration/{slot} - Get migration status
pub async fn cluster_migration_status(
    State(state): State<AppState>,
    AuthContextExtractor(ctx): AuthContextExtractor,
    Path(slot): Path<u16>,
) -> Result<Json<serde_json::Value>, SynapError> {
    debug!("REST GET /cluster/migration/{}", slot);

    // Check permission
    require_permission(&ctx, "cluster:migration", Action::Read)?;

    let migration = state
        .cluster_migration
        .as_ref()
        .ok_or_else(|| SynapError::InvalidRequest("Cluster mode not enabled".to_string()))?;

    if let Some(migration_status) = migration.get_migration(slot) {
        Ok(Json(json!({
            "slot": migration_status.slot,
            "from_node": migration_status.from_node,
            "to_node": migration_status.to_node,
            "state": format!("{:?}", migration_status.state),
            "keys_migrated": migration_status.keys_migrated,
            "total_keys": migration_status.total_keys,
            "started_at": migration_status.started_at,
            "completed_at": migration_status.completed_at
        })))
    } else {
        Ok(Json(json!({
            "slot": slot,
            "migration": null,
            "message": "No migration in progress for this slot"
        })))
    }
}

// ============================================================================
// HiveHub Integration - Quota Stats Handler
// ============================================================================

/// GET /hub/quota - Get user quota statistics
///
/// Returns quota information for the authenticated Hub user.
/// Only available when HiveHub integration is enabled.
pub async fn hub_quota_stats(
    State(state): State<AppState>,
    crate::hub::HubContextExtractor(hub_context_opt): crate::hub::HubContextExtractor,
) -> Result<Json<serde_json::Value>, SynapError> {
    // Get Hub context (user_id from Hub access key)
    let hub_context = hub_context_opt.ok_or_else(|| {
        SynapError::Unauthorized(
            "Hub integration enabled but no Hub authentication found".to_string(),
        )
    })?;

    // Get HubClient from app state
    let hub_client = state
        .hub_client
        .as_ref()
        .ok_or_else(|| SynapError::InternalError("HubClient not initialized".to_string()))?;

    // Get quota from cache or fetch from Hub
    let quota = hub_client
        .quota_manager()
        .get_quota(&hub_context.user_id)
        .ok_or_else(|| {
            SynapError::InternalError(format!(
                "Quota not found for user {}. Try authenticating again.",
                hub_context.user_id
            ))
        })?;

    // Return quota statistics
    Ok(Json(json!({
        "user_id": hub_context.user_id,
        "plan": format!("{:?}", quota.plan),
        "storage": {
            "used_bytes": quota.storage_used,
            "limit_bytes": quota.storage_limit,
            "remaining_bytes": quota.remaining_storage(),
            "usage_percent": if quota.storage_limit > 0 {
                (quota.storage_used as f64 / quota.storage_limit as f64 * 100.0).round()
            } else {
                0.0
            }
        },
        "operations": {
            "monthly_count": quota.monthly_operations,
            "monthly_limit": quota.monthly_operations_limit,
            "remaining": quota.remaining_operations(),
            "usage_percent": if quota.monthly_operations_limit > 0 {
                (quota.monthly_operations as f64 / quota.monthly_operations_limit as f64 * 100.0).round()
            } else {
                0.0
            }
        },
        "updated_at": format!("{:?}", quota.updated_at)
    })))
}
