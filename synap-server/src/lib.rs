pub mod core;
pub mod protocol;
pub mod server;

// Re-export commonly used types
pub use core::{KVConfig, KVStore, SynapError};
pub use protocol::{Request, Response};
pub use server::create_router;
