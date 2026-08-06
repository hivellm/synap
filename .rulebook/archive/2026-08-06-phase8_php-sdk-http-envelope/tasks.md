## 1. Implementation (hivellm/synap-sdk-php)

- [x] 1.1 Normalise HTTP replies per command so both paths hand modules the same shape
- [x] 1.2 Recheck the four `$response['success']` call sites against the real payloads
- [x] 1.3 Collapse the 24 `$response['payload'] ?? $response` workarounds

## 2. Verification

- [x] 2.1 Make the S2S suite take its base URL from the environment instead of pinning it in phpunit.xml
- [x] 2.2 Run the read surface over HTTP, SynapRPC and RESP3 against a live server

## 3. Tail (docs + tests)

- [x] 3.1 Update or create documentation covering the implementation
- [x] 3.2 Write tests covering the new behavior
- [x] 3.3 Run tests and confirm they pass
