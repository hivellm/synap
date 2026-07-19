## ADDED Requirements

### Requirement: Single source of truth for wire types
The Rust SDK MUST import shared wire types (`SynapValue`, `Request`, `Response`, codecs)
from `synap-protocol` instead of maintaining local copies, re-exporting them from the
SDK's existing public module paths.

#### Scenario: No duplicate wire type definitions
Given the SDK sources after this task
When searched for local re-definitions of types that exist in `synap-protocol`
Then none remain except documented client-ergonomic wrappers

### Requirement: Wire compatibility unchanged
SDK requests/responses MUST remain byte-compatible with the server after the switch.

#### Scenario: S2S round-trip
Given a running synap-server and the updated SDK
When the S2S test suite (`cargo test --features s2s-tests`) executes KV, hash, list,
set, sorted-set and queue operations
Then all round-trips succeed with identical semantics to the pre-task SDK
