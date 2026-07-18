//! RESP3 — the Redis-compatible text protocol.
//!
//! The wire layer ([`parser`] + [`writer`]) and the command-dispatch layer
//! ([`command`], which needs `AppState`) both live here. The parser and writer
//! used to sit in the published `synap-protocol` crate — now dissolved — which
//! shipped ~880 lines of server-internal parsing code to a public registry so
//! that the Rust SDK could reach the *SynapRPC* types next door. The SDK never
//! consumed RESP3 — it hand-rolls its own client-side parser — so with the RPC
//! types gone to Thunder, this is internal, and stays internal.

pub mod command;
pub mod parser;
pub mod server;
pub mod writer;
