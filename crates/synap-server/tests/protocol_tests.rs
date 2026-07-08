// Protocol Module Tests
// Tests for Request/Response envelope structures

use serde_json::json;
use synap_server::protocol::{Request, Response};

#[test]
fn test_request_new() {
    let req = Request::new("kv.set", json!({"key": "test", "value": "data"}));

    assert_eq!(req.command, "kv.set");
    assert!(!req.request_id.is_empty());
    assert_eq!(req.payload["key"], "test");
    assert_eq!(req.payload["value"], "data");
}

#[test]
fn test_request_serialization() {
    let req = Request {
        command: "kv.get".to_string(),
        request_id: "test-id-123".to_string(),
        payload: json!({"key": "mykey"}),
    };

    let json_str = serde_json::to_string(&req).unwrap();
    let deserialized: Request = serde_json::from_str(&json_str).unwrap();

    assert_eq!(deserialized.command, "kv.get");
    assert_eq!(deserialized.request_id, "test-id-123");
    assert_eq!(deserialized.payload["key"], "mykey");
}

#[test]
fn test_response_success() {
    let res = Response::success("req-123".to_string(), json!({"result": "ok", "value": 42}));

    assert!(res.success);
    assert_eq!(res.request_id, "req-123");
    assert!(res.payload.is_some());
    assert_eq!(res.payload.unwrap()["result"], "ok");
    assert!(res.error.is_none());
}

#[test]
fn test_response_error() {
    let res = Response::error("req-456".to_string(), "Something went wrong");

    assert!(!res.success);
    assert_eq!(res.request_id, "req-456");
    assert!(res.payload.is_none());
    assert_eq!(res.error.unwrap(), "Something went wrong");
}

#[test]
fn test_response_serialization() {
    let res = Response {
        success: true,
        request_id: "test-789".to_string(),
        payload: Some(json!({"data": "test"})),
        error: None,
    };

    let json_str = serde_json::to_string(&res).unwrap();
    let deserialized: Response = serde_json::from_str(&json_str).unwrap();

    assert!(deserialized.success);
    assert_eq!(deserialized.request_id, "test-789");
    assert!(deserialized.payload.is_some());
    assert!(deserialized.error.is_none());
}

#[test]
fn test_request_empty_payload() {
    let req = Request::new("ping", json!({}));

    assert_eq!(req.command, "ping");
    assert!(req.payload.is_object());
    assert_eq!(req.payload.as_object().unwrap().len(), 0);
}

#[test]
fn test_response_with_complex_payload() {
    let complex_data = json!({
        "users": [
            {"id": 1, "name": "Alice"},
            {"id": 2, "name": "Bob"}
        ],
        "total": 2,
        "page": 1
    });

    let res = Response::success("req-001".to_string(), complex_data.clone());

    assert!(res.success);
    assert_eq!(res.payload.unwrap(), complex_data);
}

#[test]
fn test_request_with_array_payload() {
    let arr_payload = json!([1, 2, 3, 4, 5]);
    let req = Request::new("batch.process", arr_payload.clone());

    assert_eq!(req.payload, arr_payload);
    assert!(req.payload.is_array());
}

#[test]
fn test_response_roundtrip_json() {
    let original = Response::success(
        "roundtrip-123".to_string(),
        json!({"test": "data", "num": 42}),
    );

    // Serialize
    let json_str = serde_json::to_string(&original).unwrap();

    // Deserialize
    let recovered: Response = serde_json::from_str(&json_str).unwrap();

    assert_eq!(recovered.success, original.success);
    assert_eq!(recovered.request_id, original.request_id);
    assert_eq!(recovered.payload, original.payload);
    assert_eq!(recovered.error, original.error);
}

#[test]
fn test_request_clone() {
    let req1 = Request::new("test.cmd", json!({"data": "value"}));
    let req2 = req1.clone();

    assert_eq!(req1.command, req2.command);
    assert_eq!(req1.request_id, req2.request_id);
    assert_eq!(req1.payload, req2.payload);
}

#[test]
fn test_response_clone() {
    let res1 = Response::success("id-1".to_string(), json!({"a": 1}));
    let res2 = res1.clone();

    assert_eq!(res1.success, res2.success);
    assert_eq!(res1.request_id, res2.request_id);
    assert_eq!(res1.payload, res2.payload);
}
