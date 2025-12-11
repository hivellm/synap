//! Observable implementation (RxJS-like)

use futures::Stream;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

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
    ///     |value| tracing::info!("Next: {}", value),
    ///     |err| tracing::error!("Error: {}", err),
    ///     || tracing::info!("Complete!")
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    #[test]
    fn test_subscription_creation() {
        let sub = Subscription::new();
        assert!(sub.is_active());
    }

    #[test]
    fn test_subscription_unsubscribe() {
        let sub = Subscription::new();
        sub.unsubscribe();
        assert!(!sub.is_active());
    }

    #[test]
    fn test_subscription_default() {
        let sub = Subscription::default();
        assert!(sub.is_active());
    }

    #[test]
    fn test_subscription_clone() {
        let sub1 = Subscription::new();
        let sub2 = sub1.clone();
        assert!(sub1.is_active());
        assert!(sub2.is_active());
        sub1.unsubscribe();
        assert!(!sub1.is_active());
        assert!(!sub2.is_active());
    }

    #[tokio::test]
    async fn test_observable_from_stream() {
        let obs = Observable::from_stream(stream::iter(vec![1, 2, 3]));
        let stream = obs.into_stream();
        use futures::StreamExt;
        let values: Vec<_> = stream.collect().await;
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_observable_map() {
        let obs = Observable::from_stream(stream::iter(vec![1, 2, 3]));
        let mapped = obs.map(|x| x * 2);
        use futures::StreamExt;
        let values: Vec<_> = mapped.into_stream().collect().await;
        assert_eq!(values, vec![2, 4, 6]);
    }

    #[tokio::test]
    async fn test_observable_filter() {
        let obs = Observable::from_stream(stream::iter(vec![1, 2, 3, 4, 5]));
        let filtered = obs.filter(|x| *x > 2);
        use futures::StreamExt;
        let values: Vec<_> = filtered.into_stream().collect().await;
        assert_eq!(values, vec![3, 4, 5]);
    }

    #[tokio::test]
    async fn test_observable_take() {
        let obs = Observable::from_stream(stream::iter(vec![1, 2, 3, 4, 5]));
        let taken = obs.take(3);
        use futures::StreamExt;
        let values: Vec<_> = taken.into_stream().collect().await;
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_observable_skip() {
        let obs = Observable::from_stream(stream::iter(vec![1, 2, 3, 4, 5]));
        let skipped = obs.skip(2);
        use futures::StreamExt;
        let values: Vec<_> = skipped.into_stream().collect().await;
        assert_eq!(values, vec![3, 4, 5]);
    }

    #[tokio::test]
    async fn test_observable_take_while() {
        let obs = Observable::from_stream(stream::iter(vec![1, 2, 3, 4, 5]));
        let taken = obs.take_while(|x| *x < 4);
        use futures::StreamExt;
        let values: Vec<_> = taken.into_stream().collect().await;
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_observable_chaining() {
        let obs = Observable::from_stream(stream::iter(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]));
        let result = obs
            .filter(|x| *x % 2 == 0) // Only even numbers
            .map(|x| x * 2) // Double them
            .take(3); // Take first 3

        use futures::StreamExt;
        let values: Vec<_> = result.into_stream().collect().await;
        assert_eq!(values, vec![4, 8, 12]);
    }
}
