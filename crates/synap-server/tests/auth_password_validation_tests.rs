//! Tests for password validation

use synap_server::auth::{
    UserManager,
    password_validation::{PasswordRequirements, validate_password, validate_password_strict},
};

#[test]
fn test_password_min_length() {
    let req = PasswordRequirements::default();

    // Too short
    assert!(req.validate("short").is_err());

    // Minimum length
    assert!(req.validate("longenough").is_ok());
}

#[test]
fn test_password_strict_requirements() {
    let req = PasswordRequirements::strict();

    // Too short
    assert!(req.validate("Short1!").is_err());
    assert!(req.validate("LongPassword123!").is_ok());

    // Missing uppercase
    assert!(req.validate("longpassword123!").is_err());

    // Missing lowercase
    assert!(req.validate("LONGPASSWORD123!").is_err());

    // Missing number
    assert!(req.validate("LongPassword!").is_err());

    // Missing special
    assert!(req.validate("LongPassword123").is_err());

    // Valid
    assert!(req.validate("LongPassword123!").is_ok());
}

#[test]
fn test_password_common_rejection() {
    let mut req = PasswordRequirements::default();
    req.reject_common_passwords = true;

    assert!(req.validate("password").is_err());
    assert!(req.validate("123456").is_err());
    assert!(req.validate("admin").is_err());
    assert!(req.validate("root").is_err());

    // Non-common password should pass
    assert!(req.validate("mypassword123").is_ok());
}

#[test]
fn test_password_relaxed_requirements() {
    let req = PasswordRequirements::relaxed();

    // Just needs minimum length
    assert!(req.validate("short").is_err());
    assert!(req.validate("longenough").is_ok());
}

#[test]
fn test_validate_password_default() {
    // Default requirements (min 8 chars, no complexity)
    assert!(validate_password("short").is_err());
    assert!(validate_password("longenough").is_ok());
}

#[test]
fn test_validate_password_strict() {
    // Strict requirements (min 12 chars, all complexity)
    assert!(validate_password_strict("Short1!").is_err());
    assert!(validate_password_strict("LongPassword123!").is_ok());
}

#[test]
fn test_user_creation_with_password_validation() {
    let manager = UserManager::new();

    // Should fail with short password
    assert!(manager.create_user("user1", "short", false).is_err());

    // Should succeed with valid password
    assert!(manager.create_user("user1", "longenough", false).is_ok());
}

#[test]
fn test_password_change_with_validation() {
    let manager = UserManager::new();
    manager.create_user("user1", "oldpassword", false).unwrap();

    // Should fail with short password
    let user = manager.get_user("user1").unwrap();
    let mut user_clone = user.clone();
    assert!(user_clone.change_password("short").is_err());

    // Should succeed with valid password
    assert!(user_clone.change_password("newlongpassword").is_ok());
}
