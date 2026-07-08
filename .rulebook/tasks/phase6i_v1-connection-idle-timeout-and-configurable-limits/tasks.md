## 1. Implementation
- [ ] 1.1 Wrap RESP3 and SynapRPC per-connection reads with a configurable idle timeout; close idle connections
- [ ] 1.2 Thread the network-limit constants into config.rs with current values as defaults
- [ ] 1.3 Pass the configured limits into the parser/codec/pubsub/listener paths
- [ ] 1.4 Document the knobs in config.example.yml
- [ ] 1.5 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior (idle connection closed; configured limit honored)
- [ ] 2.3 Run tests and confirm they pass
