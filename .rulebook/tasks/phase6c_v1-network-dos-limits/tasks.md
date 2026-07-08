## 1. Implementation
- [ ] 1.1 Enforce configurable max bulk length + max array/element count in resp3 parser; reject before allocating; bound read_line
- [ ] 1.2 Enforce configurable max frame size in synap_rpc codec read_frame; drop connection when exceeded
- [ ] 1.3 Replace pubsub UnboundedSender with a bounded channel + per-subscriber output-buffer limit and drop-or-disconnect policy
- [ ] 1.4 Add a slow-consumer / dropped-message metric for pubsub
- [ ] 1.5 Add semaphore-bounded max-connections limit to both binary listeners
- [ ] 1.6 Add idle-connection timeout to both binary listeners
- [ ] 1.7 Surface all limits in config.rs with safe defaults and document them in config.example.yml
- [ ] 1.8 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (limits section in config docs)
- [ ] 2.2 Write tests covering the new behavior (oversized frame rejected on both protocols; bounded pubsub drops/disconnects slow consumer; max-connections enforced)
- [ ] 2.3 Run tests and confirm they pass
