//! SynapRPC wire layer: MessagePack frame codec and value/request/response types.
//!
//! The dispatch layer that executes a `Request` against `AppState` lives in
//! `synap-server` (it needs the storage engine), not here.

pub mod codec;
pub mod types;

pub use types::{Request, Response, SynapValue};
