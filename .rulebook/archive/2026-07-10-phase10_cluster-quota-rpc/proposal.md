# Proposal: phase10_cluster-quota-rpc

Source: GitHub issue #231 (deferred out of phase6 v1.0 hardening)

## Why
Cluster quota coordination in `crates/synap-server/src/hub/cluster_quota.rs`
stubs the two inter-node calls it depends on: "query master for quota" and "send
quota deltas to master". Without them, per-tenant quota in cluster mode cannot be
coordinated across nodes — each node tracks quota locally and the aggregate cap is
never enforced. Depends on the cluster topology being initialized (#232).

## What Changes
1. Define a small inter-node quota RPC (request/response messages) over the
   existing cluster transport (or a dedicated TCP endpoint if none exists).
2. Implement `query_master_quota` (a follower asks the master for the current
   quota snapshot for a tenant) and `send_quota_delta` (a follower reports its
   local consumption delta to the master, which aggregates).
3. The master aggregates deltas and answers queries from the authoritative total.

## Impact
- Affected specs: cluster quota coordination (ADDED)
- Affected code: crates/synap-server/src/hub/cluster_quota.rs, cluster transport
- Breaking change: NO (cluster + hub disabled by default)
- User benefit: correct per-tenant quota enforcement across a cluster
