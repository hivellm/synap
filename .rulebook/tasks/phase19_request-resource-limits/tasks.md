## 1. Implementation
- [ ] 1.1 Enforce configured rate limits as router middleware keyed by principal or peer address, placed before authentication
- [ ] 1.2 Answer 429 with `Retry-After` and exempt health probes explicitly
- [ ] 1.3 Make startup logs report whether rate limiting is actually enforced
- [ ] 1.4 Bound UMICP request bodies with a configurable limit and answer 413 above it
- [ ] 1.5 Test burst, refill, per-client independence, limited authentication attempts, and oversized UMICP bodies with Content-Length and chunked encoding

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
