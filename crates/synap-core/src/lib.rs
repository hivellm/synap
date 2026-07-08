//! Synap core engine crate.
//!
//! The data-structure engine and its support modules — the stable heart of
//! Synap, free of any HTTP/auth/replication/hub concerns. `synap-server` builds
//! its request handlers on top of these types.
//!
//! Modules:
//! - [`core`] — KV store, hash/list/set/sorted-set, bitmap, hyperloglog,
//!   geospatial, streams, partitions, consumer groups, queues, pub/sub,
//!   transactions, and the shared [`core::error::SynapError`].
//! - [`cache`] — adaptive L1/L2 caching.
//! - [`compression`] — LZ4/Zstd compressors.
//! - [`simd`] — SIMD-accelerated primitives.
//! - [`cluster`] — hash-slot routing, topology, migration and raft primitives
//!   (kept alongside `core` because the sharded KV store references them).

pub mod cache;
pub mod cluster;
pub mod compression;
pub mod core;
pub mod simd;
