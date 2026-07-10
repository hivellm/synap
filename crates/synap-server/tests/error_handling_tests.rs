// Error Handling Tests
// Tests for SynapError types and HTTP response conversion

use axum::http::StatusCode;
use axum::response::IntoResponse;
use synap_server::SynapError;

#[test]
fn test_key_not_found_error() {
    let error = SynapError::KeyNotFound("mykey".to_string());

    assert_eq!(error.status_code(), StatusCode::NOT_FOUND);
    assert_eq!(error.to_string(), "Key not found: mykey");
}

#[test]
fn test_key_exists_error() {
    let error = SynapError::KeyExists("duplicate".to_string());

    assert_eq!(error.status_code(), StatusCode::CONFLICT);
    assert!(error.to_string().contains("already exists"));
}

#[test]
fn test_invalid_value_error() {
    let error = SynapError::InvalidValue("bad data".to_string());

    assert_eq!(error.status_code(), StatusCode::BAD_REQUEST);
}

#[test]
fn test_memory_limit_exceeded_error() {
    let error = SynapError::MemoryLimitExceeded;

    assert_eq!(error.status_code(), StatusCode::INSUFFICIENT_STORAGE);
    assert_eq!(error.to_string(), "Memory limit exceeded");
}

#[test]
fn test_ttl_invalid_error() {
    let error = SynapError::TTLInvalid("negative value".to_string());

    assert_eq!(error.status_code(), StatusCode::BAD_REQUEST);
}

#[test]
fn test_cas_failed_error() {
    let error = SynapError::CASFailed {
        expected: "v1".to_string(),
        actual: "v2".to_string(),
    };

    assert_eq!(error.status_code(), StatusCode::CONFLICT);
    assert!(error.to_string().contains("CAS failed"));
}

#[test]
fn test_unknown_command_error() {
    let error = SynapError::UnknownCommand("invalid.cmd".to_string());

    assert_eq!(error.status_code(), StatusCode::BAD_REQUEST);
}

#[test]
fn test_invalid_request_error() {
    let error = SynapError::InvalidRequest("malformed data".to_string());

    assert_eq!(error.status_code(), StatusCode::BAD_REQUEST);
}

#[test]
fn test_serialization_error() {
    let error = SynapError::SerializationError("JSON parse failed".to_string());

    assert_eq!(error.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn test_internal_error() {
    let error = SynapError::InternalError("unexpected state".to_string());

    assert_eq!(error.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn test_queue_not_found_error() {
    let error = SynapError::QueueNotFound("orders_queue".to_string());

    assert_eq!(error.status_code(), StatusCode::NOT_FOUND);
    assert!(error.to_string().contains("Queue not found"));
}

#[test]
fn test_queue_full_error() {
    let error = SynapError::QueueFull("jobs_queue".to_string());

    assert_eq!(error.status_code(), StatusCode::INSUFFICIENT_STORAGE);
    assert!(error.to_string().contains("Queue is full"));
}

#[test]
fn test_message_not_found_error() {
    let error = SynapError::MessageNotFound("msg-123".to_string());

    assert_eq!(error.status_code(), StatusCode::NOT_FOUND);
    assert!(error.to_string().contains("Message not found"));
}

#[test]
fn test_consumer_not_found_error() {
    let error = SynapError::ConsumerNotFound("consumer-1".to_string());

    assert_eq!(error.status_code(), StatusCode::NOT_FOUND);
}

#[test]
fn test_error_into_response() {
    let error = SynapError::KeyNotFound("test".to_string());

    let response = error.into_response();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn test_all_error_variants_have_status_codes() {
    // This ensures every error variant has a status code mapping
    let errors = vec![
        SynapError::KeyNotFound("k".to_string()),
        SynapError::KeyExists("k".to_string()),
        SynapError::InvalidValue("v".to_string()),
        SynapError::MemoryLimitExceeded,
        SynapError::TTLInvalid("t".to_string()),
        SynapError::CASFailed {
            expected: "e".to_string(),
            actual: "a".to_string(),
        },
        SynapError::UnknownCommand("c".to_string()),
        SynapError::InvalidRequest("r".to_string()),
        SynapError::SerializationError("s".to_string()),
        SynapError::InternalError("i".to_string()),
        SynapError::QueueNotFound("q".to_string()),
        SynapError::QueueFull("q".to_string()),
        SynapError::MessageNotFound("m".to_string()),
        SynapError::ConsumerNotFound("c".to_string()),
    ];

    for error in errors {
        // Should not panic
        let status = error.status_code();
        assert!(status.as_u16() >= 400 && status.as_u16() < 600);
    }
}

#[test]
fn test_error_display_formatting() {
    let errors = vec![
        (
            SynapError::KeyNotFound("test".to_string()),
            "Key not found: test",
        ),
        (
            SynapError::QueueFull("jobs".to_string()),
            "Queue is full: jobs",
        ),
        (SynapError::MemoryLimitExceeded, "Memory limit exceeded"),
    ];

    for (error, expected_msg) in errors {
        assert_eq!(error.to_string(), expected_msg);
    }
}
