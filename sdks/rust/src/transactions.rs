//! Redis-compatible transaction support (MULTI/EXEC/WATCH/DISCARD)

use crate::client::SynapClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Options for transaction commands
#[derive(Debug, Clone, Default)]
pub struct TransactionOptions {
    pub client_id: Option<String>,
}

impl TransactionOptions {
    fn into_payload(self) -> Value {
        match self.client_id {
            Some(client_id) => json!({"client_id": client_id}),
            None => json!({}),
        }
    }
}

/// Standard response for MULTI/DISCARD/WATCH/UNWATCH
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResponse {
    pub success: bool,
    #[serde(default)]
    pub message: Option<String>,
}

/// Result returned by EXEC
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TransactionExecResult {
    Success {
        results: Vec<Value>,
    },
    Aborted {
        aborted: bool,
        #[serde(default)]
        message: Option<String>,
    },
}

/// Helper for sending raw commands within a transaction
pub struct TransactionCommandClient {
    client: SynapClient,
    client_id: String,
}

impl TransactionCommandClient {
    /// Send a raw command ensuring the `client_id` is attached
    pub async fn send_command(&self, command: &str, mut payload: Value) -> Result<Value> {
        if let Value::Object(ref mut map) = payload {
            map.insert(
                "client_id".to_string(),
                Value::String(self.client_id.clone()),
            );
        }

        self.client.send_command(command, payload).await
    }

    /// Access underlying client id
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
}

/// Transaction manager exposing MULTI/EXEC/WATCH/DISCARD
#[derive(Clone)]
pub struct TransactionManager {
    client: SynapClient,
}

impl TransactionManager {
    pub(crate) fn new(client: SynapClient) -> Self {
        Self { client }
    }

    /// Start a transaction (MULTI)
    pub async fn multi(&self, options: TransactionOptions) -> Result<TransactionResponse> {
        let response = self
            .client
            .send_command("transaction.multi", options.clone().into_payload())
            .await?;
        Self::parse_response(response)
    }

    /// Discard an active transaction (DISCARD)
    pub async fn discard(&self, options: TransactionOptions) -> Result<TransactionResponse> {
        let response = self
            .client
            .send_command("transaction.discard", options.clone().into_payload())
            .await?;
        Self::parse_response(response)
    }

    /// Watch keys for optimistic locking (WATCH)
    pub async fn watch(
        &self,
        keys: &[impl AsRef<str>],
        options: TransactionOptions,
    ) -> Result<TransactionResponse> {
        let mut payload = options.into_payload();
        if let Value::Object(ref mut map) = payload {
            map.insert(
                "keys".into(),
                Value::Array(
                    keys.iter()
                        .map(|k| Value::String(k.as_ref().to_string()))
                        .collect(),
                ),
            );
        }

        let response = self
            .client
            .send_command("transaction.watch", payload)
            .await?;
        Self::parse_response(response)
    }

    /// Remove all watched keys (UNWATCH)
    pub async fn unwatch(&self, options: TransactionOptions) -> Result<TransactionResponse> {
        let response = self
            .client
            .send_command("transaction.unwatch", options.clone().into_payload())
            .await?;
        Self::parse_response(response)
    }

    /// Execute queued commands (EXEC)
    pub async fn exec(&self, options: TransactionOptions) -> Result<TransactionExecResult> {
        let response = self
            .client
            .send_command("transaction.exec", options.clone().into_payload())
            .await?;

        if response["results"].is_array() {
            let result = serde_json::from_value::<Vec<Value>>(response["results"].clone())?;
            return Ok(TransactionExecResult::Success { results: result });
        }

        let aborted = response["aborted"].as_bool().unwrap_or(true);
        let message = response["message"].as_str().map(|s| s.to_string());
        Ok(TransactionExecResult::Aborted { aborted, message })
    }

    /// Create a helper client that automatically injects `client_id` for raw commands
    pub fn command_client(&self, client_id: impl Into<String>) -> TransactionCommandClient {
        TransactionCommandClient {
            client: self.client.clone(),
            client_id: client_id.into(),
        }
    }

    fn parse_response(response: Value) -> Result<TransactionResponse> {
        let success = response["success"].as_bool().unwrap_or(true);
        let message = response["message"].as_str().map(|s| s.to_string());
        Ok(TransactionResponse { success, message })
    }
}
