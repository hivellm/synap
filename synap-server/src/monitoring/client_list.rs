//! Client Connection Tracking
//!
//! Track active client connections (WebSocket, HTTP long-polling, etc.)

use serde::Serialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Client information
#[derive(Debug, Clone, Serialize)]
pub struct ClientInfo {
    pub id: String,
    pub addr: String,
    pub age: u64,
    pub idle: u64,
    pub flags: String,
    pub db: u32,
    pub sub: usize,
    pub psub: usize,
    pub multi: usize,
    pub qbuf: usize,
    pub qbuf_free: usize,
    pub obl: usize,
    pub oll: usize,
    pub omem: usize,
    pub events: String,
    pub cmd: String,
}

impl ClientInfo {
    /// Create from connection details
    pub fn new(id: String, addr: String, connected_at: SystemTime) -> Self {
        let now = SystemTime::now();
        let age = now
            .duration_since(connected_at)
            .unwrap_or_default()
            .as_secs();

        Self {
            id,
            addr,
            age,
            idle: 0,                // Would need last activity time
            flags: "N".to_string(), // Normal client
            db: 0,
            sub: 0,   // Subscriptions
            psub: 0,  // Pattern subscriptions
            multi: 0, // Transaction state
            qbuf: 0,  // Query buffer
            qbuf_free: 0,
            obl: 0,                  // Output buffer length
            oll: 0,                  // Output list length
            omem: 0,                 // Output memory
            events: "r".to_string(), // Read event
            cmd: "client".to_string(),
        }
    }

    /// Format as Redis CLIENT LIST format
    pub fn to_redis_format(&self) -> String {
        format!(
            "id={} addr={} age={} idle={} flags={} db={} sub={} psub={} multi={} qbuf={} qbuf-free={} obl={} oll={} omem={} events={} cmd={}",
            self.id,
            self.addr,
            self.age,
            self.idle,
            self.flags,
            self.db,
            self.sub,
            self.psub,
            self.multi,
            self.qbuf,
            self.qbuf_free,
            self.obl,
            self.oll,
            self.omem,
            self.events,
            self.cmd
        )
    }
}

/// Client list manager
pub struct ClientListManager {
    clients: Arc<RwLock<Vec<ClientInfo>>>,
}

impl ClientListManager {
    /// Create a new client list manager
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add a client connection
    pub async fn add(&self, client: ClientInfo) {
        let mut clients = self.clients.write().await;
        clients.push(client);
    }

    /// Remove a client connection
    pub async fn remove(&self, id: &str) {
        let mut clients = self.clients.write().await;
        clients.retain(|c| c.id != id);
    }

    /// Get all clients
    pub async fn list(&self) -> Vec<ClientInfo> {
        self.clients.read().await.clone()
    }

    /// Get client count
    pub async fn len(&self) -> usize {
        self.clients.read().await.len()
    }
}

impl Clone for ClientListManager {
    fn clone(&self) -> Self {
        Self {
            clients: self.clients.clone(),
        }
    }
}

/// ClientList response
#[derive(Debug, Serialize)]
pub struct ClientList {
    pub clients: Vec<ClientInfo>,
    pub count: usize,
}
