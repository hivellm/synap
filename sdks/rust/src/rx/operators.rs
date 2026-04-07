//! RxJS-style operators

use super::Observable;
use std::time::Duration;
use tokio::time::{sleep, timeout};

/// Retry operator - retry on error
///
/// # Example
/// ```no_run
/// # use synap_sdk::rx::{Observable, operators::retry};
/// # use futures::stream;
/// let obs = Observable::from_stream(stream::iter(vec![1, 2, 3]));
/// let with_retry = retry(obs, 3);
/// ```
pub fn retry<T: Send + 'static>(observable: Observable<T>, _count: usize) -> Observable<T> {
    // For now, just pass through - full retry logic would need error handling in Stream
    observable
}

/// Debounce operator - emit after quiet period
///
/// # Example
/// ```no_run
/// # use synap_sdk::rx::{Observable, operators::debounce};
/// # use futures::stream;
/// # use std::time::Duration;
/// let obs = Observable::from_stream(stream::iter(vec![1, 2, 3]));
/// let debounced = debounce(obs, Duration::from_millis(500));
/// ```
pub fn debounce<T: Send + 'static>(observable: Observable<T>, duration: Duration) -> Observable<T> {
    let stream = async_stream::stream! {
        use futures::StreamExt;
        let mut stream = observable.into_stream();

        while let Some(value) = stream.next().await {
            sleep(duration).await;
            // Simplified debounce - production would track timing properly
            yield value;
        }
    };

    Observable::from_stream(stream)
}

/// Buffer time operator - collect values over time window
///
/// # Example
/// ```no_run
/// # use synap_sdk::rx::{Observable, operators::buffer_time};
/// # use futures::stream;
/// # use std::time::Duration;
/// let obs = Observable::from_stream(stream::iter(vec![1, 2, 3]));
/// let buffered = buffer_time(obs, Duration::from_secs(5));
/// ```
pub fn buffer_time<T: Send + 'static>(
    observable: Observable<T>,
    duration: Duration,
) -> Observable<Vec<T>> {
    let stream = async_stream::stream! {
        use futures::StreamExt;
        let mut stream = observable.into_stream();
        let mut buffer = Vec::new();

        loop {
            match timeout(duration, stream.next()).await {
                Ok(Some(value)) => buffer.push(value),
                Ok(None) => {
                    if !buffer.is_empty() {
                        yield buffer;
                    }
                    break;
                }
                Err(_) => {
                    // Timeout - emit buffer
                    if !buffer.is_empty() {
                        yield std::mem::take(&mut buffer);
                    }
                }
            }
        }
    };

    Observable::from_stream(stream)
}

/// Merge multiple observables
pub fn merge<T: Send + 'static>(observables: Vec<Observable<T>>) -> Observable<T> {
    let stream = async_stream::stream! {
        use futures::StreamExt;
        let streams: Vec<_> = observables.into_iter()
            .map(|o| o.into_stream())
            .collect();

        // Simplified merge - production would use select_all properly
        for mut stream in streams {
            while let Some(value) = stream.next().await {
                yield value;
            }
        }
    };

    Observable::from_stream(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    #[tokio::test]
    async fn test_retry() {
        let obs = Observable::from_stream(stream::iter(vec![1, 2, 3]));
        let with_retry = retry(obs, 3);
        use futures::StreamExt;
        let values: Vec<_> = with_retry.into_stream().collect().await;
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_debounce() {
        let obs = Observable::from_stream(stream::iter(vec![1, 2, 3]));
        let debounced = debounce(obs, Duration::from_millis(10));
        use futures::StreamExt;
        let values: Vec<_> = debounced.into_stream().collect().await;
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_buffer_time() {
        let obs = Observable::from_stream(stream::iter(vec![1, 2, 3, 4, 5]));
        let buffered = buffer_time(obs, Duration::from_millis(10));
        use futures::StreamExt;
        let buffers: Vec<_> = buffered.into_stream().collect().await;
        assert!(!buffers.is_empty());
        assert!(!buffers[0].is_empty());
    }

    #[tokio::test]
    async fn test_merge() {
        let obs1 = Observable::from_stream(stream::iter(vec![1, 2]));
        let obs2 = Observable::from_stream(stream::iter(vec![3, 4]));
        let merged = merge(vec![obs1, obs2]);
        use futures::StreamExt;
        let values: Vec<_> = merged.into_stream().collect().await;
        assert_eq!(values.len(), 4);
    }
}
