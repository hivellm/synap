//! Subject implementation (RxJS-like)

use super::{Observable, Subscription};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Subject - both Observable and Observer (RxJS-like)
///
/// A Subject is a special type of Observable that allows values to be
/// multicasted to many Observers.
///
/// # Example
/// ```no_run
/// # use synap_sdk::rx::Subject;
/// let subject = Subject::new();
///
/// // Subscribe
/// subject.subscribe(|value| {
///     println!("Subscriber 1: {}", value);
/// });
///
/// subject.subscribe(|value| {
///     println!("Subscriber 2: {}", value);
/// });
///
/// // Emit values
/// subject.next(1);
/// subject.next(2);
/// subject.complete();
/// ```
pub struct Subject<T: Clone + Send + 'static> {
    tx: Arc<broadcast::Sender<SubjectMessage<T>>>,
}

#[derive(Clone)]
enum SubjectMessage<T: Clone> {
    Next(T),
    Error(String),
    Complete,
}

impl<T: Clone + Send + 'static> Subject<T> {
    /// Create a new Subject
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self { tx: Arc::new(tx) }
    }

    /// Create a new Subject with custom buffer size
    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx: Arc::new(tx) }
    }

    /// Emit a value to all subscribers
    pub fn next(&self, value: T) {
        let _ = self.tx.send(SubjectMessage::Next(value));
    }

    /// Emit an error to all subscribers
    pub fn error(&self, error: String) {
        let _ = self.tx.send(SubjectMessage::Error(error));
    }

    /// Signal completion to all subscribers
    pub fn complete(&self) {
        let _ = self.tx.send(SubjectMessage::Complete);
    }

    /// Subscribe to this Subject
    ///
    /// Returns a Subscription handle that can be used to unsubscribe.
    pub fn subscribe<F>(&self, mut observer: F) -> Subscription
    where
        F: FnMut(T) + Send + 'static,
    {
        let mut rx = self.tx.subscribe();
        let subscription = Subscription::new();
        let is_active = subscription.flag();

        tokio::spawn(async move {
            while is_active.lock().map(|a| *a).unwrap_or(false) {
                match rx.recv().await {
                    Ok(SubjectMessage::Next(value)) => observer(value),
                    Ok(SubjectMessage::Error(_)) => break,
                    Ok(SubjectMessage::Complete) => break,
                    Err(_) => break,
                }
            }
        });

        subscription
    }

    /// Convert Subject to Observable
    pub fn as_observable(&self) -> Observable<T> {
        let rx = self.tx.subscribe();
        let stream = async_stream::stream! {
            let mut rx = rx;
            loop {
                match rx.recv().await {
                    Ok(SubjectMessage::Next(value)) => yield value,
                    Ok(SubjectMessage::Complete) | Err(_) => break,
                    Ok(SubjectMessage::Error(_)) => break,
                }
            }
        };

        Observable::from_stream(stream)
    }
}

impl<T: Clone + Send + 'static> Default for Subject<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Send + 'static> Clone for Subject<T> {
    fn clone(&self) -> Self {
        Self {
            tx: Arc::clone(&self.tx),
        }
    }
}
