//! Synap wire protocol crate.
//!
//! Pure serialization/parsing for the RESP3 (Redis-compatible) and SynapRPC
//! (MessagePack) transports. This crate is intentionally free of any dependency
//! on server state (`AppState`), the storage engine (`synap-core`), or the
//! request-dispatch layer — those live in `synap-server`. Keeping the wire
//! layer separate lets the Rust SDK share the exact same frame types as the
//! server without linking the whole binary.
//!
//! Layout:
//! - [`envelope`] — generic request/response envelope types.
//! - [`resp3`] — RESP3 parser and writer (`Resp3Value`).
//! - [`synap_rpc`] — SynapRPC MessagePack codec and value types.

pub mod envelope;
pub mod resp3;
pub mod synap_rpc;

pub use envelope::{Request, Response};
