//! Server-side protocol layer.
//!
//! - [`resp3`] — the Redis-compatible text protocol, whole: parser, writer,
//!   connection loop and command dispatch.
//! - [`synap_rpc`] — Synap's binary RPC surface. The wire layer belongs to
//!   [`thunder`]; what lives here is the command catalog, the protocol
//!   configuration and the listener that binds them together.
//!
//! Nothing in this module is published. The `synap-protocol` crate that used to
//! carry the RESP3 parser and the HTTP envelope existed only because publishing
//! the Rust SDK forced publishing the wire types next to them — with those types
//! now coming from Thunder's registry crate, the rest is server-internal and
//! stays that way.

pub mod resp3;
pub mod synap_rpc;
