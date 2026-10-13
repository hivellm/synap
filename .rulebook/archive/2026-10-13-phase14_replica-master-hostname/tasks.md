## 1. Implementation

- [x] 1.1 Accept `host:port` in `replication.master_address`, rejecting malformed values at load
- [x] 1.2 Resolve the master address on every connect and reconnect attempt
- [x] 1.3 Read `SYNAP_MASTER_ADDRESS` as an override of `replication.master_address`
- [x] 1.4 Point `config/config-replica.yml` back at `master:15600`
- [x] 1.5 Bring up the docker-compose stack and confirm the replicas sync

## 2. Tail (docs + tests — check or waive with tailWaiver)

- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass
