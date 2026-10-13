## 1. Implementation
- [ ] 1.1 Install Hub authentication in the production router when `hub.enabled` is true
- [ ] 1.2 Implement the Hub mode policy: Hub credentials mandatory on data routes, explicit health and metrics exemptions
- [ ] 1.3 Carry the tenant in `OperationContext` and make resource scoping fail closed when Hub mode has no tenant
- [ ] 1.4 Scope MCP, UMICP and WebSocket operations by tenant
- [ ] 1.5 Refuse to start in Hub mode with RESP3 or SynapRPC enabled, naming the listener in the error
- [ ] 1.6 Test the production router against a mock Hub API: missing and invalid keys, and two tenants sharing a logical key on every enabled surface

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
