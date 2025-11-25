//! Reactive stream operations
//!
//! Provides Stream-based event consumption for event streams.

use crate::reactive::{MessageStream, SubscriptionHandle};
use crate::types::Event;
use futures::Stream;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

impl crate::stream::StreamManager {
    /// Observe events from a stream room reactively
    ///
    /// Returns a Stream of events that can be processed asynchronously.
    /// The stream will poll for new events at the specified interval.
    ///
    /// # Example
    /// ```no_run
    /// use futures::StreamExt;
    /// use synap_sdk::{SynapClient, SynapConfig};
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;
    /// let (mut stream, handle) = client.stream()
    ///     .observe_events("chat-room-1", Some(0), Duration::from_millis(100));
    ///
    /// // Process events reactively
    /// while let Some(event) = stream.next().await {
    ///     println!("Event {}: {:?}", event.offset, event.data);
    /// }
    ///
    /// // Stop observing
    /// handle.unsubscribe();
    /// # Ok(())
    /// # }
    /// ```
    pub fn observe_events(
        &self,
        room: impl Into<String>,
        start_offset: Option<u64>,
        poll_interval: Duration,
    ) -> (impl Stream<Item = Event> + 'static, SubscriptionHandle) {
        let room = room.into();
        let client = self.client.clone();
        let mut current_offset = start_offset.unwrap_or(0);

        let (tx, rx) = mpsc::unbounded_channel::<Event>();
        let (cancel_tx, mut cancel_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_rx.recv() => {
                        tracing::debug!("Event stream cancelled");
                        break;
                    }
                    _ = sleep(poll_interval) => {
                        match client.stream().consume(&room, Some(current_offset), Some(100)).await {
                            Ok(events) => {
                                for event in events {
                                    current_offset = event.offset + 1;
                                    if tx.send(event).is_err() {
                                        return; // Receiver dropped
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("Error consuming events: {}", e);
                            }
                        }
                    }
                }
            }
        });

        let stream: MessageStream<Event> =
            Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(rx));
        let handle = SubscriptionHandle::new(cancel_tx);

        (stream, handle)
    }

    /// Observe specific event types from a stream
    ///
    /// Filters events by event type before delivering them.
    pub fn observe_event(
        &self,
        room: impl Into<String>,
        event_type: impl Into<String>,
        start_offset: Option<u64>,
        poll_interval: Duration,
    ) -> (impl Stream<Item = Event> + 'static, SubscriptionHandle) {
        let room = room.into();
        let event_type = event_type.into();
        let client = self.client.clone();
        let mut current_offset = start_offset.unwrap_or(0);

        let (tx, rx) = mpsc::unbounded_channel::<Event>();
        let (cancel_tx, mut cancel_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_rx.recv() => {
                        break;
                    }
                    _ = sleep(poll_interval) => {
                        match client.stream().consume(&room, Some(current_offset), Some(100)).await {
                            Ok(events) => {
                                for event in events {
                                    current_offset = event.offset + 1;

                                    // Filter by event type
                                    if event.event == event_type && tx.send(event).is_err() {
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("Error consuming events: {}", e);
                            }
                        }
                    }
                }
            }
        });

        let stream: MessageStream<Event> =
            Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(rx));
        let handle = SubscriptionHandle::new(cancel_tx);

        (stream, handle)
    }
}
