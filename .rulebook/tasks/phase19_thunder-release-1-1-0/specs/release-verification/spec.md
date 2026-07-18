# Spec: 1.1.0 release verification

## ADDED Requirements

### Requirement: Cross-SDK interop matrix
Every Synap SDK SHALL complete authenticate, a binary-value SET/GET round-trip, a
SUBSCRIBE/PUBLISH delivery and an error round-trip against one Thunder-based server
build before 1.1.0 is tagged.

#### Scenario: Every SDK passes
Given a Thunder-based Synap server build
When the interop matrix runs for rust, typescript, python, csharp, go, php and java
Then every cell passes

#### Scenario: A cell fails
Given any red cell in the matrix
When the cause is Thunder's
Then an issue exists on `hivellm/thunder` with the failing frame bytes and the tag is not cut

### Requirement: Legacy client tolerance
A pre-Thunder Synap SDK build SHALL still interoperate with the Thunder-based server.

#### Scenario: Legacy client round-trip
Given an SDK build that encodes `Bytes` as an array of integers and expects the same on decode
When it runs SET/GET of a binary value against the new server
Then the value round-trips correctly

### Requirement: Release gate
The 1.1.0 tag SHALL NOT be created while any interop cell, lint check or test suite
is failing.

#### Scenario: Tag preconditions
Given the release commit
When the tag is created
Then `cargo clippy -- -D warnings` passes, `cargo test` passes, every SDK suite passes, and the interop matrix is fully green

### Requirement: Version consistency
The workspace and every SDK manifest SHALL declare version 1.1.0.

#### Scenario: Manifests agree
Given the release commit
When every manifest is inspected
Then each declares 1.1.0
