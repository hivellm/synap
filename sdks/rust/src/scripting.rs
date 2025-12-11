//! Lua scripting support

use crate::client::SynapClient;
use crate::error::Result;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Options for executing scripts (keys, args, timeout)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScriptEvalOptions {
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub args: Vec<Value>,
    #[serde(rename = "timeout_ms")]
    pub timeout_ms: Option<u64>,
}

/// Response returned by EVAL/EVALSHA commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptEvalResponse<T> {
    pub result: T,
    pub sha1: String,
}

/// Response for SCRIPT EXISTS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptExistsResponse {
    pub exists: Vec<bool>,
}

/// Response for SCRIPT FLUSH
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptFlushResponse {
    pub cleared: u64,
}

/// Response for SCRIPT KILL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptKillResponse {
    pub terminated: bool,
}

/// Lua scripting manager
#[derive(Clone)]
pub struct ScriptManager {
    client: SynapClient,
}

impl ScriptManager {
    pub(crate) fn new(client: SynapClient) -> Self {
        Self { client }
    }

    /// Execute a Lua script using EVAL
    pub async fn eval<T>(
        &self,
        script: &str,
        options: ScriptEvalOptions,
    ) -> Result<ScriptEvalResponse<T>>
    where
        T: DeserializeOwned,
    {
        let payload = json!({
            "script": script,
            "keys": options.keys,
            "args": options.args,
            "timeout_ms": options.timeout_ms,
        });

        let response = self.client.send_command("script.eval", payload).await?;
        self.parse_eval_response(response)
    }

    /// Execute a cached script using SHA1 hash
    pub async fn evalsha<T>(
        &self,
        sha1: &str,
        options: ScriptEvalOptions,
    ) -> Result<ScriptEvalResponse<T>>
    where
        T: DeserializeOwned,
    {
        let payload = json!({
            "sha1": sha1,
            "keys": options.keys,
            "args": options.args,
            "timeout_ms": options.timeout_ms,
        });

        let response = self.client.send_command("script.evalsha", payload).await?;
        self.parse_eval_response(response)
    }

    /// Load a script into the cache and return its SHA1 hash
    pub async fn load(&self, script: &str) -> Result<String> {
        let payload = json!({ "script": script });
        let response = self.client.send_command("script.load", payload).await?;
        Ok(response["sha1"].as_str().unwrap_or_default().to_string())
    }

    /// Check whether scripts exist in cache
    pub async fn exists(&self, hashes: &[impl AsRef<str>]) -> Result<Vec<bool>> {
        let payload = json!({
            "hashes": hashes.iter().map(|h| h.as_ref()).collect::<Vec<_>>()
        });
        let response = self.client.send_command("script.exists", payload).await?;
        let parsed: ScriptExistsResponse = serde_json::from_value(response)?;
        Ok(parsed.exists)
    }

    /// Flush all cached scripts
    pub async fn flush(&self) -> Result<u64> {
        let response = self.client.send_command("script.flush", json!({})).await?;
        let parsed: ScriptFlushResponse = serde_json::from_value(response)?;
        Ok(parsed.cleared)
    }

    /// Kill the currently running script (if any)
    pub async fn kill(&self) -> Result<bool> {
        let response = self.client.send_command("script.kill", json!({})).await?;
        let parsed: ScriptKillResponse = serde_json::from_value(response)?;
        Ok(parsed.terminated)
    }

    fn parse_eval_response<T>(&self, response: Value) -> Result<ScriptEvalResponse<T>>
    where
        T: DeserializeOwned,
    {
        let sha1 = response["sha1"].as_str().unwrap_or_default().to_string();
        let result_value = response.get("result").cloned().unwrap_or(Value::Null);
        let result: T = serde_json::from_value(result_value)?;
        Ok(ScriptEvalResponse { result, sha1 })
    }
}
