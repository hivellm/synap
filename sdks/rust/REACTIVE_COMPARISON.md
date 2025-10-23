# Reactive Programming: RxJS vs Rust

Comparação direta entre padrões RxJS (TypeScript) e implementação Rust do Synap SDK.

## TypeScript (RxJS)

```typescript
import { Observable, Subject } from 'rxjs';
import { map, filter, take, retry, catchError } from 'rxjs/operators';

// Consume messages reactively
synap.queue.observeMessages({
  queueName: 'tasks',
  consumerId: 'worker-1',
  concurrency: 5
}).pipe(
  filter(msg => msg.message.priority >= 7),  // Only high priority
  map(msg => msg.data),                      // Extract data
  retry(3),                                   // Retry on error
  catchError(err => {
    console.error(err);
    return of(null);
  })
).subscribe({
  next: async (data) => {
    await processTask(data);
    await msg.ack();
  },
  error: (err) => console.error(err),
  complete: () => console.log('Done')
});

// Process with automatic ACK/NACK
synap.queue.processMessages({
  queueName: 'emails',
  consumerId: 'worker',
  concurrency: 10
}, async (data) => {
  await sendEmail(data);
}).subscribe({
  next: (result) => console.log(result)
});
```

## Rust (Futures + StreamExt)

```rust
use futures::StreamExt;
use std::time::Duration;

// Consume messages reactively
let (mut stream, handle) = client.queue()
    .observe_messages("tasks", "worker-1", Duration::from_millis(100));

// Chain operators (similar to RxJS pipe)
let mut processed = stream
    .filter(|msg| async move { msg.priority >= 7 })  // Only high priority
    .map(|msg| msg.data)                              // Extract data
    .take(100);                                        // Take first 100

// Subscribe (consume the stream)
while let Some(data) = processed.next().await {
    match process_task(&data).await {
        Ok(_) => println!("✅ Processed"),
        Err(e) => eprintln!("❌ Error: {}", e),
    }
}

// Stop consuming
handle.unsubscribe();
```

## Side-by-Side Comparison

| Feature | RxJS (TypeScript) | Rust (Futures) |
|---------|-------------------|----------------|
| **Create Observable** | `Observable.create()` | `async_stream::stream!` |
| **Subscribe** | `.subscribe({ next, error, complete })` | `while let Some(x) = stream.next().await` |
| **Map** | `.pipe(map(x => y))` | `.map(\|x\| y)` |
| **Filter** | `.pipe(filter(x => bool))` | `.filter(\|x\| async move { bool })` |
| **Take** | `.pipe(take(n))` | `.take(n)` |
| **Skip** | `.pipe(skip(n))` | `.skip(n)` |
| **Retry** | `.pipe(retry(3))` | Custom retry logic |
| **CatchError** | `.pipe(catchError(fn))` | `.filter_map(\|result\| ...)` |
| **Merge** | `merge(obs1, obs2)` | `futures::stream::select(s1, s2)` |
| **CombineLatest** | `combineLatest([obs1, obs2])` | `futures::stream::select_all([s1, s2])` |
| **Debounce** | `.pipe(debounce(Duration))` | Custom with `tokio::time` |
| **BufferTime** | `.pipe(bufferTime(ms))` | `.chunks_timeout(n, duration)` |
| **Subject** | `new Subject<T>()` | `tokio::sync::broadcast::channel()` |

## Rust Observable Pattern

```rust
use async_stream::stream;
use futures::Stream;
use std::pin::Pin;

// Create Observable-like stream
fn observe_events() -> Pin<Box<dyn Stream<Item = Event> + Send>> {
    Box::pin(stream! {
        loop {
            let event = fetch_event().await;
            yield event;
        }
    })
}

// Use it
let mut events = observe_events();
while let Some(event) = events.next().await {
    println!("Event: {:?}", event);
}
```

## Key Differences

### 1. **Push vs Pull**
- **RxJS**: Push-based (Observable pushes values to subscribers)
- **Rust**: Pull-based (consumer pulls values from Stream)

### 2. **Error Handling**
- **RxJS**: Errors propagate through the stream, can use `catchError`
- **Rust**: Errors are `Result<T>` values, handle with `filter_map` or pattern matching

### 3. **Backpressure**
- **RxJS**: Manual backpressure strategies
- **Rust**: Built-in via async/await (consumer controls pace)

### 4. **Hot vs Cold**
- **RxJS**: Observables are cold by default, can be made hot with `share()`
- **Rust**: Streams are always cold (lazy evaluation)

## Recommendation

**Use Rust's native Stream + StreamExt** - it's more idiom
