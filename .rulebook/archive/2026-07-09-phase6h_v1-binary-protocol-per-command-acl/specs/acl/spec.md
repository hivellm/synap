## ADDED Requirements

### Requirement: Per-command ACL on binary protocols
After authentication, the RESP3 and SynapRPC protocols MUST check the connection's
user permissions before executing each command, denying commands the user's ACL
does not allow — matching the HTTP surface.

#### Scenario: Non-admin denied a privileged command
Given an authenticated user whose role does not permit FLUSHALL
When the user issues FLUSHALL on either binary protocol
Then the command is denied and no state is mutated

#### Scenario: Permitted command allowed
Given an authenticated user whose role permits GET
When the user issues GET on either binary protocol
Then the command executes normally
