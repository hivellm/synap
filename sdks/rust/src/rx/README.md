# RxJS-style Reactive Programming for Rust

This module provides RxJS-like patterns for reactive programming in Rust, while maintaining Rust's performance and safety guarantees.

## Quick Comparison

| RxJS (TypeScript) | Rust (`synap_sdk::rx`) |
|-------------------|------------------------|
| `new Observable()` | `Observable::from_stream()` |
| `new Subject()` | `Subject::new()` |
| `.subscribe({ next, error, complete })` | `.subscribe(next, error, complete)` |
| `.pipe(map(...))` | `.map(...)` |
| `.pipe(filter(...))` | `.filter(...)` |
| `.unsubscribe()` | `.unsubscribe()` |

## Examples

### 1. Observable (RxJS-style)

```rust
use synap_sdk::rx::Observable;
use futures::stream;

// Create Observable from Stream
let obs = Observable::from_stream(stream::iter(vec![1, 2, 3, 4, 5]));

// Chain operators (like RxJS pipe)
let subscription = obs
    .filter(|x| *x > 2)          // Only > 2
    .map(|x| x * 2)               // Double them
    .take(2)                      // Take first 2
    .subscribe_next(|value| {     // Subscribe
        tracing::info!("Value: {}", value);
    });

// Cleanup
subscription.unsubscribe();
```

### 2. Subject (Multicasting)

```rust
use synap_sdk::rx::Subject;

// Create Subject
let subject = Subject::new();

// Multiple subscribers
let sub1 = subject.subscribe(|value| {
    tracing::info!("Subscriber 1: {}", value);
});

let sub2 = subject.subscribe(|value| {
    tracing::info!("Subscriber 2: {}", value);
});

// Emit values (multicast to all subscribers)
subject.next("Hello");
subject.next("World");

// Cleanup
sub1.unsubscribe();
sub2.unsubscribe();
```

### 3. Queue with Observable

```rust
use synap_sdk::rx::Observable;
use synap_sdk::{SynapClient, SynapConfig};

let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;

// Get reactive stream
let (stream, handle) = client.queue()
    .observe_messages("tasks", "worker-1", Duration::from_millis(100));

// Convert to Observable
let observable = Observable::from_stream(stream);

// Apply operators (RxJS-style)
let subscription = observable
    .filter(|msg| msg.priority >= 7)  // High priority only
    .map(|msg| String::from_utf8_lossy(&msg.payload).to_string())
    .take(10)  // First 10 messages
    .subscribe(
        |data| tracing::info!("✅ Processed: {}", data),  // next
        |err| tracing::error!("❌ Error: {}", err),        // error
        || tracing::info!("🏁 Complete!")                  // complete
    );

// Later: cleanup
subscription.unsubscribe();
handle.unsubscribe();
```

### 4. Operators

```rust
use synap_sdk::rx::{Observable, operators};
use std::time::Duration;

let obs = Observable::from_stream(stream);

// Retry on error
let with_retry = operators::retry(obs, 3);

// Debounce
let debounced = operators::debounce(obs, Duration::from_millis(500));

// Buffer time (collect values over time window)
let buffered = operators::buffer_time(obs, Duration::from_secs(5));
```

## Available Operators

### Filtering
- `filter(predicate)` - Filter values
- `take(n)` - Take first N values
- `skip(n)` - Skip first N values
- `take_while(predicate)` - Take while predicate is true

### Transformation
- `map(fn)` - Transform values

### Error Handling
- `retry(count)` - Retry on error

### Timing
- `debounce(duration)` - Emit after quiet period
- `buffer_time(duration)` - Collect values over time window

### Combination
- `merge(observables)` - Merge multiple observables

## Subject Types

### Subject
- **Hot Observable** - multicasts to multiple subscribers
- Values emitted after subscription only
- Perfect for event buses

```rust
let subject = Subject::new();
subject.subscribe(|x| tracing::info!("{}", x));
subject.next(1);  // All subscribers receive this
```

## Key Differences from RxJS

### 1. Error Handling
- **RxJS**: Errors propagate through the stream
- **Rust**: Use `Result<T>` and `filter_map` for error handling

### 2. Backpressure
- **RxJS**: Manual backpressure strategies
- **Rust**: Built-in via async/await (consumer controls pace)

### 3. Type Safety
- **RxJS**: Runtime type checking
- **Rust**: Compile-time type safety

### 4. Performance
- **RxJS**: Interpreted JavaScript
- **Rust**: Zero-cost abstractions, compiled to native code

## Best Practices

1. **Use `.subscribe_next()` for simple cases**
   ```rust
   obs.subscribe_next(|value| tracing::info!("{}", value))
   ```

2. **Chain operators for readability**
   ```rust
   obs.filter(|x| *x > 0)
      .map(|x| x * 2)
      .take(10)
      .subscribe_next(|x| tracing::info!("{}", x))
   ```

3. **Always unsubscribe when done**
   ```rust
   let sub = obs.subscribe_next(|x| tracing::info!("{}", x));
   // ... later
   sub.unsubscribe();
   ```

4. **Use Subject for event buses**
   ```rust
   let events = Subject::new();
   // Multiple subscribers can listen
   events.subscribe(handler1);
   events.subscribe(handler2);
   // Emit events
   events.next(event);
   ```

## Future Enhancements

- [ ] `BehaviorSubject` (with initial value)
- [ ] `ReplaySubject` (replay N values)
- [ ] `switchMap`, `mergeMap`, `concatMap`
- [ ] `combineLatest`, `zip`
- [ ] `throttle`, `sampleTime`
- [ ] `scan` (reduce over time)
- [ ] Better error propagation

## See Also

- [REACTIVE_COMPARISON.md](../../REACTIVE_COMPARISON.md) - Full comparison with RxJS
- [examples/rxjs_style.rs](../../examples/rxjs_style.rs) - Complete example

