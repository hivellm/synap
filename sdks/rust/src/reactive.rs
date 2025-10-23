//! Reactive utilities for Synap SDK
//!
//! Provides Stream-based reactive patterns similar to RxJS.

use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;

/// Message stream for reactive consumption
pub struct MessageStream<T> {
    receiver: mpsc::UnboundedReceiver<T>,
}

impl<T> MessageStream<T> {
    /// Create a new message stream from a receiver
    pub(crate) fn new(receiver: mpsc::UnboundedReceiver<T>) -> Self {
        Self { receiver }
    }
}

impl<T> Stream for MessageStream<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}

/// Handle for stopping a reactive subscription
pub struct SubscriptionHandle {
    cancel_tx: mpsc::UnboundedSender<()>,
}

impl SubscriptionHandle {
    /// Create a new subscription handle
    pub(crate) fn new(cancel_tx: mpsc::UnboundedSender<()>) -> Self {
        Self { cancel_tx }
    }

    /// Stop the subscription
    pub fn unsubscribe(self) {
        let _ = self.cancel_tx.send(());
    }
}

impl Drop for SubscriptionHandle {
    fn drop(&mut self) {
        // Automatically unsubscribe when handle is dropped
        // Note: send() fails if receiver is already dropped, which is fine
        let _ = self.cancel_tx.send(());
    }
}
