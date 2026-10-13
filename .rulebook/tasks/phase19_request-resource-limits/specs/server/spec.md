# Request Limits

## ADDED Requirements

### Requirement: Enforced rate limiting
When rate limiting is enabled, the server MUST limit requests per client
according to the configured rate and burst, answering 429 with `Retry-After`
once a client exceeds them, and MUST apply the limit before authentication.

#### Scenario: Burst exceeded
Given rate limiting with 10 requests per second and burst 5
When one client sends 20 requests at once
Then at most the burst allowance succeeds
And the rest receive 429 with `Retry-After`

#### Scenario: Capacity recovers
Given a client that was limited
When the refill interval passes
Then the client's requests succeed again

#### Scenario: Clients are independent
Given client A is being limited
When client B sends a request
Then client B's request succeeds

#### Scenario: Health probe exempt
Given a client that is being limited
When it calls the health endpoint
Then the server answers 200

### Requirement: Truthful startup log
The server MUST log rate limiting as enabled only when the limiter is
installed.

#### Scenario: Disabled limiter
Given `rate_limit.enabled: false`
When the server starts
Then no log line states that rate limiting is enabled

### Requirement: Bounded UMICP body
The server MUST reject a UMICP request whose body exceeds the configured limit
with 413, without buffering beyond the limit.

#### Scenario: Oversized chunked body
Given a UMICP body limit of 1 MiB
When a client streams a 50 MiB chunked body
Then the server answers 413

#### Scenario: Body at the limit
Given a UMICP body limit of 1 MiB
When a client sends a valid envelope of exactly 1 MiB
Then the request is processed
