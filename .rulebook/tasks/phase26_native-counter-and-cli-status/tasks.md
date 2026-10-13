## 1. Implementation
- [ ] 1.1 Map `kv.incr` and `kv.decr` with an amount to `INCRBY` and `DECRBY` in the Rust SDK native map, keeping `INCR` and `DECR` without one
- [ ] 1.2 Apply the same mapping in the TypeScript, Python, C# and PHP SDK native maps
- [ ] 1.3 Reject malformed CLI amounts before sending the command
- [ ] 1.4 Make CLI `PING` use a protocol ping and exit non-zero on connection refusal, timeout or denied credentials, with the diagnostic on stderr
- [ ] 1.5 Test omitted, positive, zero and negative amounts and overflow across HTTP, SynapRPC and RESP3, and the CLI executable's exit status and output streams

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
