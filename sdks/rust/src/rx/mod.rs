//! RxJS-style reactive programming for Rust
//!
//! This module provides RxJS-like operators and patterns while maintaining Rust's performance.

pub mod observable;
pub mod operators;
pub mod subject;

pub use observable::{Observable, Observer, Subscription};
pub use subject::Subject;

// Re-export common operators
pub use operators::{buffer_time, debounce, retry};
