//! Observable implementation (RxJS-like)

use futures::Stream;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::sync::mpsc;

/// Observer trait - similar to RxJS Observer
pub trait Observer<T>: Send {
    fn next(&mut self, value: T);
    fn error(&mut self, error: Box<dyn std::error::Error + Send>);
    fn complete(&mut self);
}

/// Subscription handle - similar to RxJS Subscription
#[derive(Clone)]
pub struct Subscription {
    is_active: Arc<Mutex<bool>>,
}

impl Subscription {
    pub fn new() -> Self {
        Self {
            is_active: Arc::new(Mutex::new(true)),
        }
    }

    /// Unsubscribe from the observable
    pub fn unsubscribe(&self) {
        if let Ok(mut active) = self.is_active.lock() {
            *active = false;
        }
    }

    /// Check if subscription is active
    pub fn is_active(&self) -> bool {
        self.is_active.lock().map(|a| *a).unwrap_or(false)
    }

    pub(crate) fn flag(&self) -> Arc<Mutex<bool>> {
        Arc::clone(&self.is_active)
    }
}

impl Default for Subscription {
    fn default() -> Self {
        Self::new()
    }
}

/// Observable wrapper - similar to RxJS Observable
pub struct Observable<T> {
    stream: Pin<Box<dyn Stream<Item = T> + Send + 'static>>,
}

impl<T: Send + 'static> Observable<T> {
    /// Create an Observable from a Stream
    pub fn from_stream<S>(stream: S) -> Self
    where
        S: Stream<Item = T> + Send + 'static,
    {
        Self {
            stream: Box::pin(stream),
        }
    }

    /// Subscribe with next/error/complete callbacks (RxJS style)
    ///
    /// # Example
    /// ```no_run
    /// # use synap_sdk::rx::Observable;
    /// # use futures::stream;
    /// let obs = Observable::from_stream(stream::iter(vec![1, 2, 3]));
    ///
    /// obs.subscribe(
    ///     |value| println!("Next: {}", value),
    ///     |err| eprintln!("Error: {}", err),
    ///     || println!("Complete!")
    /// );
    /// ```
    pub fn subscribe<N, E, C>(self, mut next: N, mut _on_error: E, mut complete: C) -> Subscription
    where
        N: FnMut(T) + Send + 'static,
        E: FnMut(Box<dyn std::error::Error + Send>) + Send + 'static,
        C: FnMut() + Send + 'static,
    {
        let subscription = Subscription::new();
        let is_active = subscription.flag();

        tokio::spawn(async move {
            use futures::StreamExt;
            let mut stream = self.stream;

            while is_active.lock().map(|a| *a).unwrap_or(false) {
                match stream.next().await {
                    Some(value) => next(value),
                    None => {
                        complete();
                        break;
                    }
                }
            }
        });

        subscription
    }

    /// Subscribe with only next callback (simplified)
    pub fn subscribe_next<F>(self, next: F) -> Subscription
    where
        F: FnMut(T) + Send + 'static,
    {
        self.subscribe(next, |_| {}, || {})
    }

    /// Convert back to Stream for chaining with StreamExt operators
    pub fn into_stream(self) -> Pin<Box<dyn Stream<Item = T> + Send + 'static>> {
        self.stream
    }

    /// Map operator - transform values
    pub fn map<F, R>(self, f: F) -> Observable<R>
    where
        F: FnMut(T) -> R + Send + 'static,
        R: Send + 'static,
    {
        use futures::StreamExt;
        Observable::from_stream(self.stream.map(f))
    }

    /// Filter operator - filter values
    pub fn filter<F>(self, mut f: F) -> Observable<T>
    where
        F: FnMut(&T) -> bool + Send + 'static,
    {
        use futures::StreamExt;
        Observable::from_stream(self.stream.filter(move |item| {
            let result = f(item);
            async move { result }
        }))
    }

    /// Take operator - take first N values
    pub fn take(self, n: usize) -> Observable<T> {
        use futures::StreamExt;
        Observable::from_stream(self.stream.take(n))
    }

    /// Skip operator - skip first N values
    pub fn skip(self, n: usize) -> Observable<T> {
        use futures::StreamExt;
        Observable::from_stream(self.stream.skip(n))
    }

    /// Take while predicate is true
    pub fn take_while<F>(self, mut f: F) -> Observable<T>
    where
        F: FnMut(&T) -> bool + Send + 'static,
    {
        use futures::StreamExt;
        Observable::from_stream(self.stream.take_while(move |item| {
            let result = f(item);
            async move { result }
        }))
    }
}

// Implement Stream for Observable so it can be used with StreamExt
impl<T> Stream for Observable<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.stream).poll_next(cx)
    }
}
