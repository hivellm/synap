//! Synap-specific helpers over Thunder's wire value.
//!
//! The wire value model belongs to `thunder`. The one convenience that is
//! Synap's rather than the protocol's stays here: the SDK's managers speak
//! `serde_json::Value`, so every response crosses that boundary exactly once,
//! through [`WireValueExt::to_json`].
//!
//! Numeric coercion is deliberately *not* here. RESP3 returns scalars as
//! strings, so the response mapper parses them at the call site
//! (`.as_float().or_else(|| …as_str()…parse())`), which keeps Thunder's
//! inherent `as_float` — including its `Int` widening, which the pre-Thunder
//! accessor lacked — in play.

use serde_json::{Value, json};

use super::WireValue;

/// Synap's extensions to [`WireValue`].
pub(crate) trait WireValueExt {
    /// Convert to the JSON shape the SDK's managers consume.
    ///
    /// `Bytes` decode as UTF-8 when valid, otherwise as a lowercase hex string;
    /// `Map`s become objects keyed by their string keys (non-string keys are
    /// dropped, since JSON object keys must be strings).
    fn to_json(&self) -> Value;
}

impl WireValueExt for WireValue {
    fn to_json(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(b) => json!(b),
            Self::Int(i) => json!(i),
            Self::Float(f) => json!(f),
            Self::Bytes(b) => {
                if let Ok(s) = std::str::from_utf8(b) {
                    json!(s)
                } else {
                    json!(
                        b.iter()
                            .map(|byte| format!("{:02x}", byte))
                            .collect::<String>()
                    )
                }
            }
            Self::Str(s) => json!(s),
            Self::Array(arr) => Value::Array(arr.iter().map(WireValueExt::to_json).collect()),
            Self::Map(pairs) => {
                let obj: serde_json::Map<String, Value> = pairs
                    .iter()
                    .filter_map(|(k, v)| k.as_str().map(|s| (s.to_string(), v.to_json())))
                    .collect();
                Value::Object(obj)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_json_variants() {
        assert_eq!(WireValue::Null.to_json(), Value::Null);
        assert_eq!(WireValue::Bool(true).to_json(), json!(true));
        assert_eq!(WireValue::Int(42).to_json(), json!(42));
        assert_eq!(WireValue::Str("hi".into()).to_json(), json!("hi"));
        // UTF-8 bytes decode as a string.
        assert_eq!(WireValue::bytes(b"hi".to_vec()).to_json(), json!("hi"));
        // Non-UTF-8 bytes fall back to lowercase hex.
        assert_eq!(
            WireValue::bytes(vec![0xffu8, 0x00]).to_json(),
            json!("ff00")
        );
        // Map becomes an object keyed by string keys.
        let m = WireValue::Map(vec![(WireValue::Str("k".into()), WireValue::Int(1))]);
        assert_eq!(m.to_json(), json!({"k": 1}));
    }
}
