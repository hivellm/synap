## 1. Implementation

- [ ] 1.1 Accept `host:port` in `replication.master_address`, rejecting malformed values at load
- [ ] 1.2 Resolve the master address on every connect and reconnect attempt
- [ ] 1.3 Read `SYNAP_MASTER_ADDRESS` as an override of `replication.master_address`
- [ ] 1.4 Point `config/config-replica.yml` back at `master:15600`
- [ ] 1.5 Bring up the docker-compose stack and confirm the replicas sync

## 2. Tail (docs + tests — check or waive with tailWaiver)

- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
