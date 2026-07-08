//! Server-side protocol layer: connection loops (`resp3::server`,
//! `synap_rpc::server`) and command dispatch (`resp3::command`,
//! `synap_rpc::dispatch`). The pure wire types (parser/writer/codec/envelope)
//! live in the `synap-protocol` crate.

pub mod resp3;
pub mod synap_rpc;
