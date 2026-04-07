//! Multi-Tenant Resource Helpers
//!
//! Provides helper functions for scoping resources by user in Hub mode.
//! In standalone mode, these functions pass through resource names unchanged.

use super::naming::ResourceNaming;
use uuid::Uuid;

/// Multi-tenant resource scoping helper
pub struct MultiTenant;

impl MultiTenant {
    /// Scope a queue name by user ID (in Hub mode)
    ///
    /// Format: `user_{user_id}:{queue_name}`
    ///
    /// # Arguments
    /// * `user_id` - Optional user ID (None for standalone mode)
    /// * `queue_name` - The queue name provided by user
    ///
    /// # Returns
    /// Scoped queue name if user_id is provided, otherwise original name
    pub fn scope_queue_name(user_id: Option<&Uuid>, queue_name: &str) -> String {
        match user_id {
            Some(uid) => ResourceNaming::format(uid, queue_name),
            None => queue_name.to_string(),
        }
    }

    /// Scope a stream name by user ID (in Hub mode)
    ///
    /// Format: `user_{user_id}:{stream_name}`
    pub fn scope_stream_name(user_id: Option<&Uuid>, stream_name: &str) -> String {
        match user_id {
            Some(uid) => ResourceNaming::format(uid, stream_name),
            None => stream_name.to_string(),
        }
    }

    /// Scope a KV key by user ID (in Hub mode)
    ///
    /// Format: `user_{user_id}:{key}`
    pub fn scope_kv_key(user_id: Option<&Uuid>, key: &str) -> String {
        match user_id {
            Some(uid) => ResourceNaming::format(uid, key),
            None => key.to_string(),
        }
    }

    /// Scope a pub/sub topic by user ID (in Hub mode)
    ///
    /// Format: `user_{user_id}:{topic}`
    pub fn scope_topic(user_id: Option<&Uuid>, topic: &str) -> String {
        match user_id {
            Some(uid) => ResourceNaming::format(uid, topic),
            None => topic.to_string(),
        }
    }

    /// Get user ID prefix for filtering resources
    ///
    /// Returns a prefix pattern for listing/scanning user resources
    /// Format: `user_{user_id}:*`
    pub fn get_user_prefix(user_id: &Uuid) -> String {
        format!("user_{}:", user_id.as_simple())
    }

    /// Check if a resource name belongs to a specific user
    ///
    /// # Arguments
    /// * `resource_name` - The full resource name
    /// * `user_id` - The user ID to check ownership for
    ///
    /// # Returns
    /// true if the resource belongs to the user, false otherwise
    pub fn check_ownership(resource_name: &str, user_id: &Uuid) -> bool {
        ResourceNaming::validate_ownership(resource_name, user_id)
    }

    /// Extract the original resource name from a scoped name
    ///
    /// # Arguments
    /// * `scoped_name` - The scoped resource name
    ///
    /// # Returns
    /// (user_id, resource_name) if scoped, or None if not scoped
    pub fn parse_scoped_name(scoped_name: &str) -> Option<(Uuid, String)> {
        ResourceNaming::parse(scoped_name).ok()
    }

    /// Filter resource list to only show resources owned by user
    ///
    /// # Arguments
    /// * `resources` - List of resource names
    /// * `user_id` - Optional user ID (None = return all in standalone mode)
    ///
    /// # Returns
    /// Filtered list containing only user's resources
    pub fn filter_user_resources(resources: Vec<String>, user_id: Option<&Uuid>) -> Vec<String> {
        match user_id {
            Some(uid) => resources
                .into_iter()
                .filter(|name| Self::check_ownership(name, uid))
                .collect(),
            None => resources, // Standalone mode - return all
        }
    }

    /// Unscope resource names (remove user prefix)
    ///
    /// Removes the `user_{user_id}:` prefix from resource names
    /// Useful for returning clean names to users in API responses
    ///
    /// # Arguments
    /// * `scoped_names` - List of scoped resource names
    ///
    /// # Returns
    /// List of unscoped resource names
    pub fn unscope_names(scoped_names: Vec<String>) -> Vec<String> {
        scoped_names
            .into_iter()
            .map(|name| {
                Self::parse_scoped_name(&name)
                    .map(|(_, resource_name)| resource_name)
                    .unwrap_or(name) // Return original if not scoped
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_queue_name_with_user() {
        let user_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let scoped = MultiTenant::scope_queue_name(Some(&user_id), "my-queue");
        assert_eq!(scoped, "user_550e8400e29b41d4a716446655440000:my-queue");
    }

    #[test]
    fn test_scope_queue_name_without_user() {
        let scoped = MultiTenant::scope_queue_name(None, "my-queue");
        assert_eq!(scoped, "my-queue");
    }

    #[test]
    fn test_scope_stream_name() {
        let user_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let scoped = MultiTenant::scope_stream_name(Some(&user_id), "my-stream");
        assert_eq!(scoped, "user_550e8400e29b41d4a716446655440000:my-stream");
    }

    #[test]
    fn test_scope_kv_key() {
        let user_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let scoped = MultiTenant::scope_kv_key(Some(&user_id), "my-key");
        assert_eq!(scoped, "user_550e8400e29b41d4a716446655440000:my-key");
    }

    #[test]
    fn test_scope_topic() {
        let user_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let scoped = MultiTenant::scope_topic(Some(&user_id), "my-topic");
        assert_eq!(scoped, "user_550e8400e29b41d4a716446655440000:my-topic");
    }

    #[test]
    fn test_get_user_prefix() {
        let user_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let prefix = MultiTenant::get_user_prefix(&user_id);
        assert_eq!(prefix, "user_550e8400e29b41d4a716446655440000:");
    }

    #[test]
    fn test_check_ownership_valid() {
        let user_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let resource = "user_550e8400e29b41d4a716446655440000:my-queue";
        assert!(MultiTenant::check_ownership(resource, &user_id));
    }

    #[test]
    fn test_check_ownership_invalid() {
        let user_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let other_user_id = Uuid::parse_str("650e8400-e29b-41d4-a716-446655440000").unwrap();
        let resource = format!("user_{}:my-queue", other_user_id.as_simple());
        assert!(!MultiTenant::check_ownership(&resource, &user_id));
    }

    #[test]
    fn test_parse_scoped_name_valid() {
        let scoped = "user_550e8400e29b41d4a716446655440000:my-queue";
        let result = MultiTenant::parse_scoped_name(scoped);
        assert!(result.is_some());
        let (uid, name) = result.unwrap();
        assert_eq!(
            uid,
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
        );
        assert_eq!(name, "my-queue");
    }

    #[test]
    fn test_parse_scoped_name_invalid() {
        let result = MultiTenant::parse_scoped_name("not-scoped");
        assert!(result.is_none());
    }

    #[test]
    fn test_filter_user_resources_with_user() {
        let user_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let other_id = Uuid::parse_str("650e8400-e29b-41d4-a716-446655440000").unwrap();

        let resources = vec![
            format!("user_{}:queue1", user_id.as_simple()),
            format!("user_{}:queue2", other_id.as_simple()),
            format!("user_{}:queue3", user_id.as_simple()),
        ];

        let filtered = MultiTenant::filter_user_resources(resources, Some(&user_id));
        assert_eq!(filtered.len(), 2);
        assert!(filtered[0].contains("queue1"));
        assert!(filtered[1].contains("queue3"));
    }

    #[test]
    fn test_filter_user_resources_without_user() {
        let resources = vec![
            "queue1".to_string(),
            "queue2".to_string(),
            "queue3".to_string(),
        ];

        let filtered = MultiTenant::filter_user_resources(resources.clone(), None);
        assert_eq!(filtered, resources);
    }

    #[test]
    fn test_unscope_names() {
        let scoped_names = vec![
            "user_550e8400e29b41d4a716446655440000:queue1".to_string(),
            "user_550e8400e29b41d4a716446655440000:queue2".to_string(),
            "standalone-queue".to_string(), // Not scoped
        ];

        let unscoped = MultiTenant::unscope_names(scoped_names);
        assert_eq!(unscoped.len(), 3);
        assert_eq!(unscoped[0], "queue1");
        assert_eq!(unscoped[1], "queue2");
        assert_eq!(unscoped[2], "standalone-queue");
    }
}
