//! Synap's Thunder protocol configuration.
//!
//! Thunder ships one standard and zero product knowledge, so the description of
//! how *Synap* uses the shared wire lives here, in Synap's repository. Both the
//! server listener and the Rust SDK read this constant, so the two halves of one
//! product cannot disagree about the handshake shape.
//!
//! Where Synap diverges from `Config::standard()`, and why:
//!
//! | Dimension | Synap | Reason |
//! |---|---|---|
//! | `handshake` | `AuthCommand` | Synap authenticates with `AUTH <pass>` / `AUTH <user> <pass>`; it has never had a `HELLO` handler on the RPC port. |
//! | `hello_style` | `NotUsed` | Follows from the above: credentials travel in `AUTH`. |
//! | `push` | `Enabled` | `SUBSCRIBE` delivers pub/sub messages as push frames — Synap is the family's one shipping push producer. |
//! | `error_codes` | `Resp3Prefixes` | Synap's errors are the Redis-compatible `NOAUTH` / `WRONGPASS` / `NOPERM` strings, shared with its RESP3 port. |
//! | `max_frame_bytes` | 512 MiB | Preserves the pre-Thunder `synap-protocol` cap; lowering it would reject frames a deployment accepts today. |

use thunder::Config;
use thunder::wire::config::{ErrorConvention, Handshake, HelloStyle, PushPolicy};

/// Default SynapRPC port.
pub const DEFAULT_RPC_PORT: u16 = 15501;

/// Frame-body cap, carried over from `synap-protocol`'s `MAX_FRAME_SIZE`.
pub const MAX_FRAME_BYTES: usize = 512 * 1024 * 1024;

/// How Synap uses the Thunder wire (see the module docs for the divergences).
pub const fn synap_config() -> Config {
    Config::standard()
        .scheme("synap")
        .port(DEFAULT_RPC_PORT)
        .handshake(Handshake::AuthCommand)
        .hello_style(HelloStyle::NotUsed)
        .push(PushPolicy::Enabled)
        .error_codes(ErrorConvention::Resp3Prefixes)
        .max_frame_bytes(MAX_FRAME_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The SDKs hard-code these values in five languages; a silent change here
    /// would desynchronise them from the server.
    #[test]
    fn config_matches_what_the_sdks_assume() {
        let c = synap_config();
        assert_eq!(c.scheme, "synap");
        assert_eq!(c.default_port, 15501);
        assert_eq!(c.handshake, Handshake::AuthCommand);
        assert_eq!(c.hello_style, HelloStyle::NotUsed);
        assert_eq!(c.push, PushPolicy::Enabled);
        assert_eq!(c.error_codes, ErrorConvention::Resp3Prefixes);
        assert_eq!(c.max_frame_bytes, 512 * 1024 * 1024);
    }

    /// The cap must not silently shrink relative to the pre-Thunder server,
    /// which accepted frames up to 512 MiB.
    #[test]
    fn frame_cap_is_not_below_the_legacy_cap() {
        assert!(synap_config().max_frame_bytes >= 512 * 1024 * 1024);
    }
}
