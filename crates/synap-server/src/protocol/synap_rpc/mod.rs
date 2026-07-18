//! SynapRPC — Synap's binary RPC surface, carried by Thunder.
//!
//! The wire layer (value model, `Request`/`Response`, length-prefixed
//! MessagePack framing, frame caps) is [`thunder`]'s. What stays here is what
//! Synap owns: the command catalog ([`dispatch`]), the protocol configuration
//! ([`config`]) and the listener that binds the two together ([`server`]).
//!
//! `SynapValue` is an alias of [`thunder::Value`] rather than a distinct type,
//! so the ~1 900-line dispatch tree — and every call site outside it — keeps
//! reading the same way it did before the swap.

pub mod config;
pub mod dispatch;
pub mod server;

pub use config::synap_config;

/// Synap's name for the shared wire value model.
pub type SynapValue = thunder::Value;

pub use thunder::{Request, Response};
