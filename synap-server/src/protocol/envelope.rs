use serde::{Deserialize, Serialize};

/// StreamableHTTP request envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Command to execute (e.g., "kv.set", "kv.get")
    pub command: String,
    /// Unique request identifier
    pub request_id: String,
    /// Command payload
    pub payload: serde_json::Value,
}

/// StreamableHTTP response envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Whether the operation succeeded
    pub success: bool,
    /// Matching request identifier
    pub request_id: String,
    /// Response payload (if successful)
    pub payload: Option<serde_json::Value>,
    /// Error message (if failed)
    pub error: Option<String>,
}

impl Request {
    /// Create a new request
    pub fn new(command: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            command: command.into(),
            request_id: uuid::Uuid::new_v4().to_string(),
            payload,
        }
    }
}

impl Response {
    /// Create a successful response
    pub fn success(request_id: String, payload: serde_json::Value) -> Self {
        Self {
            success: true,
            request_id,
            payload: Some(payload),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(request_id: String, error: impl Into<String>) -> Self {
        Self {
            success: false,
            request_id,
            payload: None,
            error: Some(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_request_new() {
        let payload = json!({"key": "value"});
        let request = Request::new("kv.set", payload.clone());

        assert_eq!(request.command, "kv.set");
        assert_eq!(request.payload, payload);
        assert!(!request.request_id.is_empty());
    }

    #[test]
    fn test_request_serialization() {
        let payload = json!({"key": "test", "value": "data"});
        let request = Request::new("kv.get", payload);

        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: Request = serde_json::from_str(&serialized).unwrap();

        assert_eq!(request.command, deserialized.command);
        assert_eq!(request.request_id, deserialized.request_id);
        assert_eq!(request.payload, deserialized.payload);
    }

    #[test]
    fn test_response_success() {
        let request_id = "req-123".to_string();
        let payload = json!({"result": "ok"});
        let response = Response::success(request_id.clone(), payload.clone());

        assert!(response.success);
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.payload, Some(payload));
        assert_eq!(response.error, None);
    }

    #[test]
    fn test_response_error() {
        let request_id = "req-456".to_string();
        let error_msg = "Something went wrong";
        let response = Response::error(request_id.clone(), error_msg);

        assert!(!response.success);
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.payload, None);
        assert_eq!(response.error, Some(error_msg.to_string()));
    }

    #[test]
    fn test_response_serialization() {
        let response = Response::success("req-789".to_string(), json!({"data": "test"}));

        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: Response = serde_json::from_str(&serialized).unwrap();

        assert_eq!(response.success, deserialized.success);
        assert_eq!(response.request_id, deserialized.request_id);
        assert_eq!(response.payload, deserialized.payload);
        assert_eq!(response.error, deserialized.error);
    }

    #[test]
    fn test_request_unique_ids() {
        let req1 = Request::new("test", json!({}));
        let req2 = Request::new("test", json!({}));

        // Each request should have a unique ID
        assert_ne!(req1.request_id, req2.request_id);
    }

    #[test]
    fn test_response_error_string_conversion() {
        let response = Response::error("req-1".to_string(), String::from("error"));
        assert_eq!(response.error, Some("error".to_string()));

        let response = Response::error("req-2".to_string(), "error");
        assert_eq!(response.error, Some("error".to_string()));
    }
}
