# phase5: SDK shares synap-protocol wire types via WireValue=SynapValue alias
**Source**: manual
**Date**: 2026-07-09
**Related Task**: phase5_v1-wire-rust-sdk-to-protocol
**Tags**: analysis:synap-v1-release, phase5, sdk, synap-protocol, wire-types, e2e
phase5 removed the Rust SDK's duplicated SynapRPC wire types. The SDK had local WireValue/RpcRequest/RpcResponse in sdks/rust/src/transport/mod.rs that mirrored synap_protocol::synap_rpc::{SynapValue,Request,Response} by hand.

Key technique: `pub(crate) use synap_protocol::synap_rpc::SynapValue as WireValue;` (a re-export alias) keeps every existing call site working — enum variant construction (`WireValue::Str(..)`), patterns, and even function-reference use `.map(WireValue::to_json)` all resolve through the alias to the shared type's inherent methods. The SDK's three extra client helpers (as_float/is_null/to_json) were folded INTO SynapValue in synap-protocol (completing its as_str/as_bytes/as_int accessor set; the crate already depends on serde_json so to_json fits). Request/Response imported as `use ... as RpcRequest/RpcResponse` — their pub fields allow struct-literal construction from the SDK crate. Write path switched to synap_protocol::synap_rpc::codec::encode_frame; read path keeps rmp_serde::from_slice on the shared Response type. Wire bytes unchanged (only added methods, never touched enum shape or serde derives), so it is wire-compatible by construction.

Intentional divergence kept: the SDK's domain DTOs in sdks/rust/src/types.rs (Message, QueueStats, Event, StreamStats, PubSubMessage, KVStats, HyperLogLogStats) are client response shapes deserialized from the JSON normalization layer, NOT wire frames — documented in the SDK README "Wire types" section.

Gotcha found+fixed: the earlier config/ relocation broke the SDK e2e harness (sdks/rust/tests/e2e_test.rs read workspace/config.yml; now config/config.yml). The live e2e (spawns the release synap-server, exercises HTTP+SynapRPC+RESP3 with cross-transport consistency) passes 8/8. It requires the RELEASE binary (target/release/synap-server.exe) built first.