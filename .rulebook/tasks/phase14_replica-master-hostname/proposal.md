# Proposal: phase14_replica-master-hostname

## Why

The docker-compose replication setup cannot work. `replication.master_address`
is an `Option<SocketAddr>`, so it accepts only `IP:port`, while
`config/config-replica.yml`, which the compose file mounts into every replica,
needs to point at the compose service name `master`. `docker-compose.yml` also
sets `SYNAP_MASTER_ADDRESS=master:15600` for each replica, but no code reads
that variable. Until 1.3.2 the replica file used `"master:15600"` and failed to
parse at startup; 1.3.2 made it load by switching to a placeholder IP and
documented the gap, so the compose replicas now start and fail to reach the
master instead.

## What Changes

- Accept a hostname in `master_address`, resolved when the replica connects
  (and re-resolved on reconnect, since a container's IP can change), while
  still rejecting a malformed value at config load.
- Read `SYNAP_MASTER_ADDRESS` as an override of `replication.master_address`,
  next to the existing `SYNAP_*` environment overrides.
- Point `config/config-replica.yml` back at `master:15600`.
- Bring the compose stack up and confirm the replicas sync from the master.

## Impact

- Affected specs: replication configuration
- Affected code: `crates/synap-server/src/replication/config.rs`, the replica
  connect path, the environment override loading, `config/config-replica.yml`
- Breaking change: NO, an `IP:port` value keeps working
- User benefit: master/replica replication works with service names in
  docker-compose and Kubernetes
