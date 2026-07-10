# Proposal: phase11_cluster-raft-peer-networking

Source: GitHub issue #233 (follow-up — multi-node consensus over the wire)

## Why
phase10 (#233) implemented the cluster consensus *logic* — Raft `request_vote`
term-based granting, `receive_heartbeat` follower term tracking, failover
detect/promote, migration rollback, and env config load, all unit-tested. What
remains is the distributed *networking*: the Raft worker currently becomes leader
immediately (single-node) because it does not actually send `RequestVote` to peers
or collect a majority, and a leader does not send `AppendEntries` heartbeats over
the wire. Without peer RPC, a real multi-node cluster cannot elect a leader,
detect a peer's failure by missed heartbeats, or fail over. This is a focused
distributed-systems effort split out of #233 so it gets a dedicated, careful pass.

## What Changes
1. A Raft peer RPC transport (length-prefixed bincode over TCP, mirroring the
   quota RPC in `hub/cluster_quota.rs`): `RequestVote{term, candidate_id}` →
   `VoteGranted{term, granted}`, and `AppendEntries{term, leader_id}` → `Ack`.
2. The Raft worker sends `RequestVote` to all peers on election timeout, tallies
   votes, and only becomes leader on a majority; a leader sends periodic
   `AppendEntries` heartbeats to all peers.
3. Failover uses missed heartbeats across the peer set to detect a dead node and
   promote the most-caught-up replica.
4. End-to-end multi-node tests: a 3-node cluster elects exactly one leader; on
   leader loss a new leader is elected.

## Impact
- Affected specs: cluster multi-node consensus (ADDED)
- Affected code: crates/synap-core/src/cluster/{raft,failover}.rs, a new peer RPC
  transport, main.rs wiring of peer addresses from config
- Breaking change: NO (cluster disabled by default)
- User benefit: a real multi-node clustered deployment with leader election and
  automatic failover
