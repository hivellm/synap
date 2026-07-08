//! User-Scoped Resource Naming
//!
//! Implements the naming convention for multi-tenant resource isolation.
//! All resources are prefixed with `user_{user_id}:` to ensure complete
//! data isolation between users in SaaS mode.

use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum NamingError {
    #[error("Invalid resource name format")]
    InvalidFormat,

    #[error("Missing user_ prefix")]
    MissingPrefix,

    #[error("Invalid user ID format")]
    InvalidUserId,

    #[error("Resource name is empty")]
    EmptyResourceName,
}

/// Resource naming utilities for user-scoped isolation
pub struct ResourceNaming;

impl ResourceNaming {
    /// Format a resource name with user namespace prefix
    ///
    /// # Example
    /// ```ignore
    /// let user_id = Uuid::new_v4();
    /// let full_name = ResourceNaming::format(&user_id, "my_queue");
    /// // Result: "user_550e8400e29b41d4a716446655440000:my_queue"
    /// ```
    pub fn format(user_id: &Uuid, resource_name: &str) -> String {
        format!("user_{}:{}", user_id.as_simple(), resource_name)
    }

    /// Parse a full resource name to extract user ID and resource name
    ///
    /// # Returns
    /// - Ok((user_id, resource_name)) if valid
    /// - Err(NamingError) if format is invalid
    ///
    /// # Example
    /// ```ignore
    /// let (user_id, name) = ResourceNaming::parse("user_550e8400e29b41d4a716446655440000:my_queue")?;
    /// ```
    pub fn parse(full_name: &str) -> Result<(Uuid, String), NamingError> {
        let parts: Vec<&str> = full_name.split(':').collect();

        if parts.len() != 2 {
            return Err(NamingError::InvalidFormat);
        }

        let prefix = parts[0];
        let resource_name = parts[1];

        if !prefix.starts_with("user_") {
            return Err(NamingError::MissingPrefix);
        }

        if resource_name.is_empty() {
            return Err(NamingError::EmptyResourceName);
        }

        let user_id_str = &prefix[5..]; // Skip "user_"
        let user_id = Uuid::parse_str(user_id_str).map_err(|_| NamingError::InvalidUserId)?;

        Ok((user_id, resource_name.to_string()))
    }

    /// Validate that a resource belongs to a specific user
    ///
    /// # Returns
    /// - true if the resource belongs to the user
    /// - false otherwise
    pub fn validate_ownership(full_name: &str, user_id: &Uuid) -> bool {
        match Self::parse(full_name) {
            Ok((parsed_user_id, _)) => parsed_user_id == *user_id,
            Err(_) => false,
        }
    }

    /// Extract just the resource name without the user prefix
    ///
    /// # Example
    /// ```ignore
    /// let name = ResourceNaming::extract_name("user_550e8400e29b41d4a716446655440000:my_queue");
    /// // Result: Ok("my_queue")
    /// ```
    pub fn extract_name(full_name: &str) -> Result<String, NamingError> {
        let (_, resource_name) = Self::parse(full_name)?;
        Ok(resource_name)
    }

    /// Extract just the user ID from a full resource name
    pub fn extract_user_id(full_name: &str) -> Result<Uuid, NamingError> {
        let (user_id, _) = Self::parse(full_name)?;
        Ok(user_id)
    }

    /// Check if a resource name is already formatted with user prefix
    pub fn is_formatted(name: &str) -> bool {
        name.starts_with("user_") && name.contains(':')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_basic() {
        let user_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let full_name = ResourceNaming::format(&user_id, "my_queue");
        assert_eq!(full_name, "user_550e8400e29b41d4a716446655440000:my_queue");
    }

    #[test]
    fn test_parse_valid() {
        let full_name = "user_550e8400e29b41d4a716446655440000:my_queue";
        let (user_id, resource_name) = ResourceNaming::parse(full_name).unwrap();
        assert_eq!(user_id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(resource_name, "my_queue");
    }

    #[test]
    fn test_parse_invalid_format_no_colon() {
        let result = ResourceNaming::parse("user_550e8400e29b41d4a716446655440000");
        assert!(matches!(result, Err(NamingError::InvalidFormat)));
    }

    #[test]
    fn test_parse_invalid_format_multiple_colons() {
        let result = ResourceNaming::parse("user_550e8400e29b41d4a716446655440000:queue:test");
        assert!(matches!(result, Err(NamingError::InvalidFormat)));
    }

    #[test]
    fn test_parse_missing_prefix() {
        let result = ResourceNaming::parse("550e8400e29b41d4a716446655440000:my_queue");
        assert!(matches!(result, Err(NamingError::MissingPrefix)));
    }

    #[test]
    fn test_parse_invalid_user_id() {
        let result = ResourceNaming::parse("user_invalid:my_queue");
        assert!(matches!(result, Err(NamingError::InvalidUserId)));
    }

    #[test]
    fn test_parse_empty_resource_name() {
        let result = ResourceNaming::parse("user_550e8400e29b41d4a716446655440000:");
        assert!(matches!(result, Err(NamingError::EmptyResourceName)));
    }

    #[test]
    fn test_validate_ownership_valid() {
        let user_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let full_name = ResourceNaming::format(&user_id, "my_queue");
        assert!(ResourceNaming::validate_ownership(&full_name, &user_id));
    }

    #[test]
    fn test_validate_ownership_different_user() {
        let user_id_1 = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let user_id_2 = Uuid::parse_str("660e8400-e29b-41d4-a716-446655440000").unwrap();
        let full_name = ResourceNaming::format(&user_id_1, "my_queue");
        assert!(!ResourceNaming::validate_ownership(&full_name, &user_id_2));
    }

    #[test]
    fn test_validate_ownership_invalid_format() {
        let user_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        assert!(!ResourceNaming::validate_ownership("invalid", &user_id));
    }

    #[test]
    fn test_extract_name() {
        let full_name = "user_550e8400e29b41d4a716446655440000:my_queue";
        let name = ResourceNaming::extract_name(full_name).unwrap();
        assert_eq!(name, "my_queue");
    }

    #[test]
    fn test_extract_user_id() {
        let full_name = "user_550e8400e29b41d4a716446655440000:my_queue";
        let user_id = ResourceNaming::extract_user_id(full_name).unwrap();
        assert_eq!(user_id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn test_is_formatted() {
        assert!(ResourceNaming::is_formatted(
            "user_550e8400e29b41d4a716446655440000:my_queue"
        ));
        assert!(!ResourceNaming::is_formatted("my_queue"));
        assert!(!ResourceNaming::is_formatted(
            "user_550e8400e29b41d4a716446655440000"
        ));
    }

    #[test]
    fn test_round_trip() {
        let user_id = Uuid::new_v4();
        let original_name = "test_queue";
        let full_name = ResourceNaming::format(&user_id, original_name);
        let (parsed_id, parsed_name) = ResourceNaming::parse(&full_name).unwrap();
        assert_eq!(parsed_id, user_id);
        assert_eq!(parsed_name, original_name);
    }
}
