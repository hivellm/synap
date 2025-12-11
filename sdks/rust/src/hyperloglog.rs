//! HyperLogLog operations (PFADD/PFCOUNT/PFMERGE)

use crate::client::SynapClient;
use crate::error::Result;
use crate::types::HyperLogLogStats;
use serde_json::json;

#[derive(Clone)]
pub struct HyperLogLogManager {
    client: SynapClient,
}

impl HyperLogLogManager {
    pub(crate) fn new(client: SynapClient) -> Self {
        Self { client }
    }

    /// Add elements to a HyperLogLog structure (PFADD)
    pub async fn pfadd<I, T>(&self, key: &str, elements: I) -> Result<usize>
    where
        I: IntoIterator<Item = T>,
        T: AsRef<[u8]>,
    {
        let encoded: Vec<Vec<u8>> = elements
            .into_iter()
            .map(|el| el.as_ref().to_vec())
            .collect();
        if encoded.is_empty() {
            return Ok(0);
        }

        let payload = json!({
            "key": key,
            "elements": encoded,
        });

        let response = self
            .client
            .send_command("hyperloglog.pfadd", payload)
            .await?;
        Ok(response["added"].as_u64().unwrap_or(0) as usize)
    }

    /// Estimate cardinality of a HyperLogLog structure (PFCOUNT)
    pub async fn pfcount(&self, key: &str) -> Result<u64> {
        let payload = json!({"key": key});
        let response = self
            .client
            .send_command("hyperloglog.pfcount", payload)
            .await?;
        Ok(response["count"].as_u64().unwrap_or(0))
    }

    /// Merge multiple HyperLogLog structures into destination (PFMERGE)
    pub async fn pfmerge<S>(&self, destination: &str, sources: &[S]) -> Result<u64>
    where
        S: AsRef<str>,
    {
        let payload = json!({
            "destination": destination,
            "sources": sources.iter().map(|s| s.as_ref()).collect::<Vec<_>>(),
        });
        let response = self
            .client
            .send_command("hyperloglog.pfmerge", payload)
            .await?;
        Ok(response["count"].as_u64().unwrap_or(0))
    }

    /// Retrieve HyperLogLog statistics
    pub async fn stats(&self) -> Result<HyperLogLogStats> {
        let response = self
            .client
            .send_command("hyperloglog.stats", json!({}))
            .await?;
        let stats: HyperLogLogStats = serde_json::from_value(response)?;
        Ok(stats)
    }
}
