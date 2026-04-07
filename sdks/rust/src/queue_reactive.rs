//! Reactive queue operations
//!
//! Provides Stream-based message consumption for queues.

use crate::error::Result;
use crate::reactive::{MessageStream, SubscriptionHandle};
use crate::types::Message;
use futures::Stream;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

impl crate::queue::QueueManager {
    /// Observe messages from a queue reactively
    ///
    /// Returns a Stream of messages that can be processed asynchronously.
    /// The stream will poll the queue at the specified interval.
    ///
    /// # Arguments
    /// * `queue_name` - Name of the queue
    /// * `consumer_id` - Unique consumer identifier
    /// * `poll_interval` - How often to poll for new messages
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
    /// let (mut stream, handle) = client.queue()
    ///     .observe_messages("tasks", "worker-1", Duration::from_millis(100));
    ///
    /// // Process messages reactively
    /// while let Some(message) = stream.next().await {
    ///     tracing::info!("Received: {:?}", message);
    ///     // ACK handled automatically
    /// }
    ///
    /// // Stop consuming
    /// handle.unsubscribe();
    /// # Ok(())
    /// # }
    /// ```
    pub fn observe_messages(
        &self,
        queue_name: impl Into<String>,
        consumer_id: impl Into<String>,
        poll_interval: Duration,
    ) -> (impl Stream<Item = Message> + 'static, SubscriptionHandle) {
        let queue_name = queue_name.into();
        let consumer_id = consumer_id.into();
        let client = self.client.clone();

        let (tx, rx) = mpsc::unbounded_channel();
        let (cancel_tx, mut cancel_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_rx.recv() => {
                        tracing::debug!("Message stream cancelled");
                        break;
                    }
                    _ = sleep(poll_interval) => {
                        match client.queue().consume(&queue_name, &consumer_id).await {
                            Ok(Some(message)) => {
                                if tx.send(message).is_err() {
                                    break; // Receiver dropped
                                }
                            }
                            Ok(None) => {
                                // No messages available, continue polling
                            }
                            Err(e) => {
                                tracing::error!("Error consuming message: {}", e);
                                // Continue polling despite errors
                            }
                        }
                    }
                }
            }
        });

        let stream: MessageStream<Message> =
            Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(rx));
        let handle = SubscriptionHandle::new(cancel_tx);

        (stream, handle)
    }

    /// Process messages from a queue with automatic ACK/NACK
    ///
    /// Automatically acknowledges successfully processed messages and
    /// requeues failed ones.
    ///
    /// # Example
    /// ```no_run
    /// use synap_sdk::{SynapClient, SynapConfig};
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;
    /// let handle = client.queue().process_messages(
    ///     "tasks",
    ///     "worker-1",
    ///     Duration::from_millis(100),
    ///     |message| async move {
    ///         // Process the message
    ///         tracing::info!("Processing: {:?}", message.id);
    ///         Ok(()) // Success = ACK, Err = NACK
    ///     }
    /// );
    ///
    /// // Stop processing
    /// handle.unsubscribe();
    /// # Ok(())
    /// # }
    /// ```
    pub fn process_messages<F, Fut>(
        &self,
        queue_name: impl Into<String>,
        consumer_id: impl Into<String>,
        poll_interval: Duration,
        handler: F,
    ) -> SubscriptionHandle
    where
        F: Fn(Message) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        let queue_name = queue_name.into();
        let consumer_id = consumer_id.into();
        let client = self.client.clone();

        let (cancel_tx, mut cancel_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_rx.recv() => {
                        tracing::debug!("Message processor cancelled");
                        break;
                    }
                    _ = sleep(poll_interval) => {
                        match client.queue().consume(&queue_name, &consumer_id).await {
                            Ok(Some(message)) => {
                                let msg_id = message.id.clone();

                                // Process message
                                match handler(message).await {
                                    Ok(()) => {
                                        // Success: ACK
                                        if let Err(e) = client.queue().ack(&queue_name, &msg_id).await {
                                            tracing::error!("Failed to ACK message {}: {}", msg_id, e);
                                        }
                                    }
                                    Err(e) => {
                                        // Error: NACK (requeue)
                                        tracing::warn!("Processing failed for {}: {}", msg_id, e);
                                        if let Err(e) = client.queue().nack(&queue_name, &msg_id).await {
                                            tracing::error!("Failed to NACK message {}: {}", msg_id, e);
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                // No messages, continue polling
                            }
                            Err(e) => {
                                tracing::error!("Error consuming message: {}", e);
                            }
                        }
                    }
                }
            }
        });

        SubscriptionHandle::new(cancel_tx)
    }
}
