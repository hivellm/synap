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
pub fn retry<T: Send + 'static>(observable: Observable<T>, count: usize) -> Observable<T> {
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
        let mut streams: Vec<_> = observables.into_iter()
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
