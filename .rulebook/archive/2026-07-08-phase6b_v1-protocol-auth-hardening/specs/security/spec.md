## ADDED Requirements

### Requirement: Authentication enforced on all command protocols
When authentication is enabled, the RESP3 and SynapRPC listeners MUST reject commands from
unauthenticated connections and MUST enforce ACL permissions per command, identical to the
HTTP path.

#### Scenario: Unauthenticated SynapRPC command is rejected
Given auth is enabled and a client connects to the SynapRPC port without authenticating
When it sends a SET command
Then the server returns an auth error and does not mutate state

#### Scenario: Unauthenticated RESP3 command is rejected
Given auth is enabled and a client connects to port 6379 without a valid AUTH
When it sends any non-AUTH/HELLO/QUIT command
Then the server responds NOAUTH and does not execute the command

#### Scenario: ACL denies an unpermitted command
Given an authenticated user whose ACL forbids FLUSHALL
When the user issues FLUSHALL on either binary protocol
Then the command is denied

## MODIFIED Requirements

### Requirement: Strong password hashing
User passwords MUST be hashed with a salted, computationally hard function (bcrypt/argon2)
and verified with a constant-time comparison. Plain unsalted SHA-512 is prohibited.

#### Scenario: Password stored with bcrypt
Given a newly created or password-changed user
When the stored hash is inspected
Then it is a bcrypt (or argon2) hash with a per-user salt, not a bare SHA-512 digest

#### Scenario: Legacy hash migrates on login
Given a user whose password is still stored as a legacy SHA-512 hash
When the user logs in successfully
Then the stored hash is transparently upgraded to bcrypt
