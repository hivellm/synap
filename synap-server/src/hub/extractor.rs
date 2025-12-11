//! Hub User Context Extractor
//!
//! Optional extractor for HubUserContext from request extensions.
//! Returns None if Hub integration is disabled or no Hub context is present.

use super::HubUserContext;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use std::convert::Infallible;

/// Optional Hub user context extractor
///
/// Extracts HubUserContext from request extensions if available.
/// Unlike AuthContextExtractor, this is optional and returns None if not found.
#[derive(Debug, Clone)]
pub struct HubContextExtractor(pub Option<HubUserContext>);

impl<S> FromRequestParts<S> for HubContextExtractor
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let hub_context = parts.extensions.get::<HubUserContext>().cloned();
        Ok(HubContextExtractor(hub_context))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::restrictions::Plan;
    use axum::body::Body;
    use axum::http::Request;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_extractor_with_context() {
        let user_id = Uuid::new_v4();
        let hub_ctx = HubUserContext::new(user_id, Plan::Pro, "test_key".to_string());

        let mut req = Request::builder().uri("/test").body(Body::empty()).unwrap();

        req.extensions_mut().insert(hub_ctx.clone());

        let (mut parts, _body) = req.into_parts();
        let HubContextExtractor(extracted) =
            HubContextExtractor::from_request_parts(&mut parts, &())
                .await
                .unwrap();

        assert!(extracted.is_some());
        let ctx = extracted.unwrap();
        assert_eq!(ctx.user_id(), &user_id);
        assert_eq!(ctx.plan(), Plan::Pro);
    }

    #[tokio::test]
    async fn test_extractor_without_context() {
        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();

        let (mut parts, _body) = req.into_parts();
        let HubContextExtractor(extracted) =
            HubContextExtractor::from_request_parts(&mut parts, &())
                .await
                .unwrap();

        assert!(extracted.is_none());
    }
}
