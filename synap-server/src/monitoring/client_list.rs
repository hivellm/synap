//! Client Connection Tracking
//!
//! Track active client connections (WebSocket, HTTP long-polling, etc.)

use serde::Serialize;
use std::sync::Arc;
use std::time::SystemTime;
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

impl Default for ClientListManager {
    fn default() -> Self {
        Self::new()
    }
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

    /// Check if client list is empty
    pub async fn is_empty(&self) -> bool {
        self.clients.read().await.is_empty()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_list_new() {
        let manager = ClientListManager::new();
        assert_eq!(manager.len().await, 0);
        assert!(manager.is_empty().await);
    }

    #[tokio::test]
    async fn test_client_list_add() {
        let manager = ClientListManager::new();
        let client = ClientInfo::new(
            "client1".to_string(),
            "127.0.0.1:12345".to_string(),
            SystemTime::now(),
        );

        manager.add(client).await;
        assert_eq!(manager.len().await, 1);
        assert!(!manager.is_empty().await);
    }

    #[tokio::test]
    async fn test_client_list_remove() {
        let manager = ClientListManager::new();
        let client1 = ClientInfo::new(
            "client1".to_string(),
            "127.0.0.1:12345".to_string(),
            SystemTime::now(),
        );
        let client2 = ClientInfo::new(
            "client2".to_string(),
            "127.0.0.1:12346".to_string(),
            SystemTime::now(),
        );

        manager.add(client1).await;
        manager.add(client2).await;
        assert_eq!(manager.len().await, 2);

        manager.remove("client1").await;
        assert_eq!(manager.len().await, 1);

        let clients = manager.list().await;
        assert_eq!(clients[0].id, "client2");
    }

    #[tokio::test]
    async fn test_client_list_list() {
        let manager = ClientListManager::new();

        for i in 0..3 {
            let client = ClientInfo::new(
                format!("client{}", i),
                format!("127.0.0.1:1234{}", i),
                SystemTime::now(),
            );
            manager.add(client).await;
        }

        let clients = manager.list().await;
        assert_eq!(clients.len(), 3);
        assert_eq!(clients[0].id, "client0");
        assert_eq!(clients[1].id, "client1");
        assert_eq!(clients[2].id, "client2");
    }

    #[tokio::test]
    async fn test_client_info_new() {
        let connected_at = SystemTime::now();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let client = ClientInfo::new(
            "test_id".to_string(),
            "127.0.0.1:12345".to_string(),
            connected_at,
        );

        assert_eq!(client.id, "test_id");
        assert_eq!(client.addr, "127.0.0.1:12345");
        assert_eq!(client.flags, "N");
        assert_eq!(client.db, 0);
    }

    #[test]
    fn test_client_info_to_redis_format() {
        let client = ClientInfo::new(
            "123".to_string(),
            "127.0.0.1:12345".to_string(),
            SystemTime::now(),
        );

        let formatted = client.to_redis_format();
        assert!(formatted.contains("id=123"));
        assert!(formatted.contains("addr=127.0.0.1:12345"));
        assert!(formatted.contains("flags=N"));
    }
}
