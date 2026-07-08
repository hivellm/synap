//! RESP3 wire layer: parser (`Resp3Value` + reader) and writer.
//!
//! The command-dispatch layer that maps a parsed `Resp3Value` to store
//! operations lives in `synap-server` (it needs `AppState`), not here.

pub mod parser;
pub mod writer;
