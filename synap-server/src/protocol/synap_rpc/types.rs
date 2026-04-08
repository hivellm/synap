//! SynapRPC wire types — shared between client and server.

use serde::{Deserialize, Serialize};

/// A dynamically-typed value that can cross the SynapRPC wire.
///
/// Encoded with rmp-serde's default externally-tagged representation:
/// unit variants become a bare string, newtype/tuple variants become
/// a single-key map `{"Variant": payload}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SynapValue {
    /// SQL NULL / Redis nil
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    /// Raw bytes — stored without base64 encoding (unlike HTTP/JSON transport)
    Bytes(Vec<u8>),
    Str(String),
    Array(Vec<SynapValue>),
    Map(Vec<(SynapValue, SynapValue)>),
}

impl SynapValue {
    /// Convenience: extract inner string slice.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Convenience: extract bytes (also accepts Str as UTF-8 bytes).
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(b) => Some(b.as_slice()),
            Self::Str(s) => Some(s.as_bytes()),
            _ => None,
        }
    }

    /// Convenience: extract i64.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            _ => None,
        }
    }
}

impl From<String> for SynapValue {
    fn from(s: String) -> Self {
        Self::Str(s)
    }
}

impl From<&str> for SynapValue {
    fn from(s: &str) -> Self {
        Self::Str(s.to_owned())
    }
}

impl From<Vec<u8>> for SynapValue {
    fn from(b: Vec<u8>) -> Self {
        Self::Bytes(b)
    }
}

impl From<i64> for SynapValue {
    fn from(i: i64) -> Self {
        Self::Int(i)
    }
}

impl From<bool> for SynapValue {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

// ── Wire frames ───────────────────────────────────────────────────────────────

/// A request from client to server.
///
/// `id` is chosen by the client and echoed back in the matching `Response`,
/// enabling out-of-order (multiplexed) responses on a single TCP connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Caller-chosen monotonic ID; used to match responses on multiplexed connections.
    pub id: u32,
    /// Command name, e.g. "SET", "GET", "HSET".
    pub command: String,
    /// Positional arguments — same order as the REST/RESP APIs.
    pub args: Vec<SynapValue>,
}

/// A response from server to client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Echoes the `id` from the corresponding `Request`.
    pub id: u32,
    /// `Ok(value)` on success, `Err(message)` on error.
    pub result: Result<SynapValue, String>,
}

impl Response {
    pub fn ok(id: u32, value: SynapValue) -> Self {
        Self {
            id,
            result: Ok(value),
        }
    }

    pub fn err(id: u32, msg: impl Into<String>) -> Self {
        Self {
            id,
            result: Err(msg.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synap_value_roundtrip_all_variants() {
        let variants: Vec<SynapValue> = vec![
            SynapValue::Null,
            SynapValue::Bool(true),
            SynapValue::Bool(false),
            SynapValue::Int(i64::MIN),
            SynapValue::Int(0),
            SynapValue::Int(i64::MAX),
            SynapValue::Float(1.5_f64),
            SynapValue::Float(f64::NEG_INFINITY),
            SynapValue::Bytes(vec![0, 1, 2, 255]),
            SynapValue::Bytes(vec![]),
            SynapValue::Str("hello".into()),
            SynapValue::Str(String::new()),
            SynapValue::Array(vec![SynapValue::Int(1), SynapValue::Str("two".into())]),
            SynapValue::Map(vec![(SynapValue::Str("k".into()), SynapValue::Int(99))]),
        ];

        for v in variants {
            let encoded = rmp_serde::to_vec(&v).expect("encode");
            let decoded: SynapValue = rmp_serde::from_slice(&encoded).expect("decode");
            assert_eq!(v, decoded);
        }
    }

    #[test]
    fn request_response_serde() {
        let req = Request {
            id: 42,
            command: "SET".into(),
            args: vec![
                SynapValue::Str("key".into()),
                SynapValue::Bytes(b"val".to_vec()),
            ],
        };
        let enc = rmp_serde::to_vec(&req).unwrap();
        let dec: Request = rmp_serde::from_slice(&enc).unwrap();
        assert_eq!(dec.id, 42);
        assert_eq!(dec.command, "SET");

        let resp = Response::ok(42, SynapValue::Str("OK".into()));
        let enc = rmp_serde::to_vec(&resp).unwrap();
        let dec: Response = rmp_serde::from_slice(&enc).unwrap();
        assert_eq!(dec.id, 42);
        assert!(dec.result.is_ok());
    }
}
