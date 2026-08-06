## 1. Implementation (hivellm/synap-sdk-php)

- [ ] 1.1 Normalise HTTP replies per command so both paths hand modules the same shape
- [ ] 1.2 Recheck the four `$response['success']` call sites against the real payloads
- [ ] 1.3 Collapse the 29 `$response['payload'] ?? $response` workarounds

## 2. Verification

- [ ] 2.1 Make the S2S suite take its base URL from the environment instead of pinning it in phpunit.xml
- [ ] 2.2 Run the read surface over HTTP, SynapRPC and RESP3 against a live server

## 3. Tail (docs + tests)

- [ ] 3.1 Update or create documentation covering the implementation
- [ ] 3.2 Write tests covering the new behavior
- [ ] 3.3 Run tests and confirm they pass
