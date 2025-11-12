//! Bitmap operations (SETBIT/GETBIT/BITCOUNT/BITPOS/BITOP)

use crate::client::SynapClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Bitmap operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitmapOperation {
    And,
    Or,
    Xor,
    Not,
}

impl BitmapOperation {
    fn as_str(&self) -> &'static str {
        match self {
            BitmapOperation::And => "AND",
            BitmapOperation::Or => "OR",
            BitmapOperation::Xor => "XOR",
            BitmapOperation::Not => "NOT",
        }
    }
}

/// Bitmap statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BitmapStats {
    pub total_bitmaps: usize,
    pub total_bits: usize,
    pub setbit_count: usize,
    pub getbit_count: usize,
    pub bitcount_count: usize,
    pub bitop_count: usize,
    pub bitpos_count: usize,
    pub bitfield_count: usize,
}

#[derive(Clone)]
pub struct BitmapManager {
    client: SynapClient,
}

impl BitmapManager {
    pub(crate) fn new(client: SynapClient) -> Self {
        Self { client }
    }

    /// Set bit at offset to value (SETBIT)
    ///
    /// # Arguments
    ///
    /// * `key` - Bitmap key
    /// * `offset` - Bit offset (0-based)
    /// * `value` - Bit value (0 or 1)
    ///
    /// # Returns
    ///
    /// Previous bit value (0 or 1)
    pub async fn setbit(&self, key: &str, offset: usize, value: u8) -> Result<u8> {
        if value > 1 {
            return Err(crate::error::SynapError::ServerError(
                "Bitmap value must be 0 or 1".to_string(),
            ));
        }

        let payload = json!({
            "key": key,
            "offset": offset,
            "value": value,
        });

        let response = self.client.send_command("bitmap.setbit", payload).await?;
        Ok(response["old_value"].as_u64().unwrap_or(0) as u8)
    }

    /// Get bit at offset (GETBIT)
    ///
    /// # Arguments
    ///
    /// * `key` - Bitmap key
    /// * `offset` - Bit offset (0-based)
    ///
    /// # Returns
    ///
    /// Bit value (0 or 1)
    pub async fn getbit(&self, key: &str, offset: usize) -> Result<u8> {
        let payload = json!({
            "key": key,
            "offset": offset,
        });

        let response = self.client.send_command("bitmap.getbit", payload).await?;
        Ok(response["value"].as_u64().unwrap_or(0) as u8)
    }

    /// Count set bits in bitmap (BITCOUNT)
    ///
    /// # Arguments
    ///
    /// * `key` - Bitmap key
    /// * `start` - Optional start offset (inclusive)
    /// * `end` - Optional end offset (inclusive)
    ///
    /// # Returns
    ///
    /// Number of set bits
    pub async fn bitcount(
        &self,
        key: &str,
        start: Option<usize>,
        end: Option<usize>,
    ) -> Result<usize> {
        let mut payload = json!({"key": key});
        if let Some(start_val) = start {
            payload["start"] = json!(start_val);
        }
        if let Some(end_val) = end {
            payload["end"] = json!(end_val);
        }

        let response = self.client.send_command("bitmap.bitcount", payload).await?;
        Ok(response["count"].as_u64().unwrap_or(0) as usize)
    }

    /// Find first bit set to value (BITPOS)
    ///
    /// # Arguments
    ///
    /// * `key` - Bitmap key
    /// * `value` - Bit value to search for (0 or 1)
    /// * `start` - Optional start offset (inclusive)
    /// * `end` - Optional end offset (inclusive)
    ///
    /// # Returns
    ///
    /// Position of first matching bit, or None if not found
    pub async fn bitpos(
        &self,
        key: &str,
        value: u8,
        start: Option<usize>,
        end: Option<usize>,
    ) -> Result<Option<usize>> {
        if value > 1 {
            return Err(crate::error::SynapError::ServerError(
                "Bitmap value must be 0 or 1".to_string(),
            ));
        }

        let mut payload = json!({
            "key": key,
            "value": value,
        });
        if let Some(start_val) = start {
            payload["start"] = json!(start_val);
        }
        if let Some(end_val) = end {
            payload["end"] = json!(end_val);
        }

        let response = self.client.send_command("bitmap.bitpos", payload).await?;

        if let Some(pos) = response["position"].as_u64() {
            Ok(Some(pos as usize))
        } else {
            Ok(None)
        }
    }

    /// Perform bitwise operation on multiple bitmaps (BITOP)
    ///
    /// # Arguments
    ///
    /// * `operation` - Bitwise operation (AND, OR, XOR, NOT)
    /// * `destination` - Destination key for result
    /// * `source_keys` - Source bitmap keys (NOT requires exactly 1 source)
    ///
    /// # Returns
    ///
    /// Length of resulting bitmap in bits
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - NOT operation is used with more than one source key
    /// - No source keys provided
    pub async fn bitop<S>(
        &self,
        operation: BitmapOperation,
        destination: &str,
        source_keys: &[S],
    ) -> Result<usize>
    where
        S: AsRef<str>,
    {
        if operation == BitmapOperation::Not && source_keys.len() != 1 {
            return Err(crate::error::SynapError::ServerError(
                "NOT operation requires exactly one source key".to_string(),
            ));
        }

        if source_keys.is_empty() {
            return Err(crate::error::SynapError::ServerError(
                "BITOP requires at least one source key".to_string(),
            ));
        }

        let payload = json!({
            "destination": destination,
            "operation": operation.as_str(),
            "source_keys": source_keys.iter().map(|s| s.as_ref()).collect::<Vec<_>>(),
        });

        let response = self.client.send_command("bitmap.bitop", payload).await?;
        Ok(response["length"].as_u64().unwrap_or(0) as usize)
    }

    /// Execute bitfield operations (BITFIELD)
    ///
    /// # Arguments
    ///
    /// * `key` - Bitmap key
    /// * `operations` - Vector of bitfield operations
    ///
    /// # Returns
    ///
    /// Vector of result values (one per operation)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use synap_sdk::bitmap::BitmapManager;
    /// use serde_json::json;
    ///
    /// # async fn example(bitmap: BitmapManager) -> Result<(), Box<dyn std::error::Error>> {
    /// let operations = vec![
    ///     json!({
    ///         "operation": "SET",
    ///         "offset": 0,
    ///         "width": 8,
    ///         "signed": false,
    ///         "value": 42
    ///     }),
    ///     json!({
    ///         "operation": "GET",
    ///         "offset": 0,
    ///         "width": 8,
    ///         "signed": false
    ///     }),
    ///     json!({
    ///         "operation": "INCRBY",
    ///         "offset": 0,
    ///         "width": 8,
    ///         "signed": false,
    ///         "increment": 10,
    ///         "overflow": "WRAP"
    ///     }),
    /// ];
    ///
    /// let results = bitmap.bitfield("mybitmap", &operations).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bitfield(&self, key: &str, operations: &[serde_json::Value]) -> Result<Vec<i64>> {
        let payload = json!({
            "key": key,
            "operations": operations,
        });

        let response = self.client.send_command("bitmap.bitfield", payload).await?;
        let results = response["results"]
            .as_array()
            .ok_or_else(|| {
                crate::error::SynapError::ServerError(
                    "Invalid response format for bitfield".to_string(),
                )
            })?
            .iter()
            .map(|v| v.as_i64().unwrap_or(0))
            .collect();

        Ok(results)
    }

    /// Retrieve bitmap statistics
    pub async fn stats(&self) -> Result<BitmapStats> {
        let response = self.client.send_command("bitmap.stats", json!({})).await?;
        let stats: BitmapStats = serde_json::from_value(response)?;
        Ok(stats)
    }
}
