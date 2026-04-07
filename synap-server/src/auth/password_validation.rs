//! Password validation and requirements

use super::AuthResult;
use crate::core::SynapError;

/// Password requirements configuration
#[derive(Debug, Clone)]
pub struct PasswordRequirements {
    /// Minimum password length
    pub min_length: usize,
    /// Require uppercase letters
    pub require_uppercase: bool,
    /// Require lowercase letters
    pub require_lowercase: bool,
    /// Require numbers
    pub require_numbers: bool,
    /// Require special characters
    pub require_special: bool,
    /// Common passwords to reject
    pub reject_common_passwords: bool,
}

impl Default for PasswordRequirements {
    fn default() -> Self {
        Self {
            min_length: 8,
            require_uppercase: false,
            require_lowercase: false,
            require_numbers: false,
            require_special: false,
            reject_common_passwords: true,
        }
    }
}

impl PasswordRequirements {
    /// Create strict password requirements (production-ready)
    pub fn strict() -> Self {
        Self {
            min_length: 12,
            require_uppercase: true,
            require_lowercase: true,
            require_numbers: true,
            require_special: true,
            reject_common_passwords: true,
        }
    }

    /// Create relaxed password requirements (development)
    pub fn relaxed() -> Self {
        Self {
            min_length: 6,
            require_uppercase: false,
            require_lowercase: false,
            require_numbers: false,
            require_special: false,
            reject_common_passwords: false,
        }
    }

    /// Validate a password against requirements
    pub fn validate(&self, password: &str) -> AuthResult<()> {
        // Check minimum length
        if password.len() < self.min_length {
            return Err(SynapError::InvalidRequest(format!(
                "Password must be at least {} characters long",
                self.min_length
            )));
        }

        // Check for uppercase
        if self.require_uppercase && !password.chars().any(|c| c.is_uppercase()) {
            return Err(SynapError::InvalidRequest(
                "Password must contain at least one uppercase letter".to_string(),
            ));
        }

        // Check for lowercase
        if self.require_lowercase && !password.chars().any(|c| c.is_lowercase()) {
            return Err(SynapError::InvalidRequest(
                "Password must contain at least one lowercase letter".to_string(),
            ));
        }

        // Check for numbers
        if self.require_numbers && !password.chars().any(|c| c.is_ascii_digit()) {
            return Err(SynapError::InvalidRequest(
                "Password must contain at least one number".to_string(),
            ));
        }

        // Check for special characters
        if self.require_special {
            let has_special = password
                .chars()
                .any(|c| c.is_ascii_punctuation() || c.is_ascii_graphic() && !c.is_alphanumeric());
            if !has_special {
                return Err(SynapError::InvalidRequest(
                    "Password must contain at least one special character".to_string(),
                ));
            }
        }

        // Check against common passwords
        if self.reject_common_passwords && Self::is_common_password(password) {
            return Err(SynapError::InvalidRequest(
                "Password is too common. Please choose a stronger password".to_string(),
            ));
        }

        Ok(())
    }

    /// Check if password is in common passwords list
    fn is_common_password(password: &str) -> bool {
        let common_passwords = [
            "password",
            "123456",
            "123456789",
            "12345678",
            "12345",
            "1234567",
            "1234567890",
            "qwerty",
            "abc123",
            "monkey",
            "123123",
            "dragon",
            "111111",
            "baseball",
            "iloveyou",
            "trustno1",
            "1234567",
            "sunshine",
            "master",
            "123321",
            "welcome",
            "shadow",
            "ashley",
            "football",
            "jesus",
            "michael",
            "ninja",
            "mustang",
            "password1",
            "root",
            "admin",
            "administrator",
            "letmein",
            "pass",
            "passw0rd",
            "root123",
        ];

        let password_lower = password.to_lowercase();
        common_passwords
            .iter()
            .any(|&common| password_lower == common)
    }
}

/// Validate password with default requirements
pub fn validate_password(password: &str) -> AuthResult<()> {
    PasswordRequirements::default().validate(password)
}

/// Validate password with strict requirements
pub fn validate_password_strict(password: &str) -> AuthResult<()> {
    PasswordRequirements::strict().validate(password)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_length() {
        let req = PasswordRequirements::default();
        assert!(req.validate("short").is_err());
        assert!(req.validate("longenough").is_ok());
    }

    #[test]
    fn test_strict_requirements() {
        let req = PasswordRequirements::strict();

        // Too short
        assert!(req.validate("Short1!").is_err());

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
    fn test_common_password_rejection() {
        let req = PasswordRequirements {
            min_length: 6,
            reject_common_passwords: true,
            ..Default::default()
        };

        assert!(req.validate("password").is_err());
        assert!(req.validate("123456").is_err());
        assert!(req.validate("admin").is_err());
        assert!(req.validate("root").is_err());
    }

    #[test]
    fn test_relaxed_requirements() {
        let req = PasswordRequirements::relaxed();

        // Just needs minimum length
        assert!(req.validate("short").is_err());
        assert!(req.validate("longenough").is_ok());
    }
}
