//! Reactive streaming primitives
//!
//! Provides base types for reactive message/event consumption.

use futures::Stream;
use std::pin::Pin;
use tokio::sync::mpsc;

/// A stream of messages
pub type MessageStream<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;

/// Subscription handle for controlling message streams
pub struct SubscriptionHandle {
    cancel_tx: mpsc::UnboundedSender<()>,
}

impl SubscriptionHandle {
    /// Create a new subscription handle
    pub(crate) fn new(cancel_tx: mpsc::UnboundedSender<()>) -> Self {
        Self { cancel_tx }
    }

    /// Unsubscribe from the stream
    pub fn unsubscribe(&self) {
        let _ = self.cancel_tx.send(());
    }

    /// Check if subscription is still active
    pub fn is_active(&self) -> bool {
        !self.cancel_tx.is_closed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscription_handle() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = SubscriptionHandle::new(tx);
        
        assert!(handle.is_active());
        handle.unsubscribe();
        assert!(rx.recv().await.is_some());
    }
}
