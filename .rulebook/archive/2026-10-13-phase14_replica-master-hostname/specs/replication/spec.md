## MODIFIED Requirements

### Requirement: Master address by hostname
A replica MUST accept `replication.master_address` as either `IP:port` or
`hostname:port`, and MUST resolve a hostname on each connection attempt.

#### Scenario: Compose service name
Given a replica configured with `master_address: "master:15600"`
When the `master` service is reachable on the compose network
Then the replica connects and completes its initial sync

#### Scenario: Malformed address rejected at load
Given `master_address: "master"` with no port
When the configuration is loaded
Then loading fails with an error naming `replication.master_address`

### Requirement: Environment override
The server MUST let `SYNAP_MASTER_ADDRESS` override
`replication.master_address`.

#### Scenario: Override wins over the file
Given a config file with `master_address: "10.0.0.10:15600"`
And `SYNAP_MASTER_ADDRESS=master:15600`
When the server starts as a replica
Then it connects to `master:15600`
