//! Migration of a standalone Synap data directory into one tenant's
//! namespace for HiveHub.Cloud (Hub) mode.
//!
//! The data directory is read with the server's own persistence code
//! ([`source`]), every resource is renamed the way Hub mode names it for the
//! tenant ([`scope`]), and the result is written to a new directory as one
//! snapshot in the server's current format ([`migrate`]). The `synap-migrate`
//! binary is the command-line front end.

pub mod migrate;
pub mod report;
pub mod scope;
pub mod snapshot;
pub mod source;
pub mod validate;
