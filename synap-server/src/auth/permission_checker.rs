//! Permission checking utilities
//!
//! Provides helper functions for checking permissions on different resource types

use super::{Action, AuthContext};

/// Check if context has permission for a KV operation
pub fn check_kv_permission(ctx: &AuthContext, key: &str, action: Action) -> bool {
    let resource = format!("kv:{}", key);
    ctx.has_permission(&resource, action)
}

/// Check if context has permission for a Hash operation
pub fn check_hash_permission(ctx: &AuthContext, key: &str, action: Action) -> bool {
    let resource = format!("hash:{}", key);
    ctx.has_permission(&resource, action)
}

/// Check if context has permission for a List operation
pub fn check_list_permission(ctx: &AuthContext, key: &str, action: Action) -> bool {
    let resource = format!("list:{}", key);
    ctx.has_permission(&resource, action)
}

/// Check if context has permission for a Set operation
pub fn check_set_permission(ctx: &AuthContext, key: &str, action: Action) -> bool {
    let resource = format!("set:{}", key);
    ctx.has_permission(&resource, action)
}

/// Check if context has permission for a Sorted Set operation
pub fn check_sortedset_permission(ctx: &AuthContext, key: &str, action: Action) -> bool {
    let resource = format!("sortedset:{}", key);
    ctx.has_permission(&resource, action)
}

/// Check if context has permission for a Queue operation
pub fn check_queue_permission(ctx: &AuthContext, queue_name: &str, action: Action) -> bool {
    let resource = format!("queue:{}", queue_name);
    ctx.has_permission(&resource, action)
}

/// Check if context has permission for a Stream operation
pub fn check_stream_permission(ctx: &AuthContext, room_name: &str, action: Action) -> bool {
    let resource = format!("stream:{}", room_name);
    ctx.has_permission(&resource, action)
}

/// Check if context has permission for a Pub/Sub operation
pub fn check_pubsub_permission(ctx: &AuthContext, topic: &str, action: Action) -> bool {
    let resource = format!("pubsub:{}", topic);
    ctx.has_permission(&resource, action)
}

/// Check if context has permission for a Transaction operation
pub fn check_transaction_permission(ctx: &AuthContext, action: Action) -> bool {
    ctx.has_permission("transaction:*", action)
}

/// Check if context has permission for a Script operation
pub fn check_script_permission(ctx: &AuthContext, action: Action) -> bool {
    ctx.has_permission("script:*", action)
}

/// Check if context has admin permissions
pub fn check_admin_permission(ctx: &AuthContext) -> bool {
    ctx.has_permission("admin:*", Action::Admin) || ctx.is_admin
}

/// Check if context has permission for a resource with custom format
pub fn check_custom_permission(
    ctx: &AuthContext,
    resource_type: &str,
    resource_name: &str,
    action: Action,
) -> bool {
    let resource = format!("{}:{}", resource_type, resource_name);
    ctx.has_permission(&resource, action)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Permission as Perm;
    use std::net::IpAddr;

    fn create_test_context(permissions: Vec<Perm>) -> AuthContext {
        AuthContext {
            user_id: Some("test_user".to_string()),
            api_key_id: None,
            client_ip: IpAddr::from([127, 0, 0, 1]),
            permissions,
            is_admin: false,
        }
    }

    #[test]
    fn test_check_kv_permission() {
        let ctx = create_test_context(vec![Perm::new("kv:*", Action::Read)]);
        assert!(check_kv_permission(&ctx, "users:123", Action::Read));
        assert!(!check_kv_permission(&ctx, "users:123", Action::Write));
    }

    #[test]
    fn test_check_queue_permission() {
        let ctx = create_test_context(vec![
            Perm::new("queue:orders", Action::Read),
            Perm::new("queue:*", Action::Write),
        ]);
        assert!(check_queue_permission(&ctx, "orders", Action::Read));
        assert!(check_queue_permission(&ctx, "orders", Action::Write));
        assert!(check_queue_permission(&ctx, "payments", Action::Write));
        assert!(!check_queue_permission(&ctx, "payments", Action::Read));
    }

    #[test]
    fn test_check_stream_permission() {
        let ctx = create_test_context(vec![Perm::new("stream:chat-*", Action::Read)]);
        assert!(check_stream_permission(&ctx, "chat-room1", Action::Read));
        assert!(check_stream_permission(&ctx, "chat-room2", Action::Read));
        assert!(!check_stream_permission(
            &ctx,
            "notifications",
            Action::Read
        ));
    }

    #[test]
    fn test_check_pubsub_permission() {
        let ctx = create_test_context(vec![Perm::new("pubsub:*", Action::Write)]);
        assert!(check_pubsub_permission(&ctx, "user.created", Action::Write));
        assert!(check_pubsub_permission(&ctx, "order.placed", Action::Write));
    }

    #[test]
    fn test_check_admin_permission() {
        let ctx = create_test_context(vec![Perm::new("admin:*", Action::Admin)]);
        assert!(check_admin_permission(&ctx));

        let admin_ctx = AuthContext {
            user_id: Some("admin".to_string()),
            api_key_id: None,
            client_ip: IpAddr::from([127, 0, 0, 1]),
            permissions: vec![],
            is_admin: true,
        };
        assert!(check_admin_permission(&admin_ctx));
    }

    #[test]
    fn test_check_custom_permission() {
        let ctx = create_test_context(vec![Perm::new("custom:*", Action::Read)]);
        assert!(check_custom_permission(
            &ctx,
            "custom",
            "resource1",
            Action::Read
        ));
        assert!(!check_custom_permission(
            &ctx,
            "custom",
            "resource1",
            Action::Write
        ));
    }
}
