---
title: Rate Limiting Configuration
module: configuration
id: rate-limiting
order: 6
description: Configure rate limiting to protect against DoS attacks
tags: [configuration, rate-limiting, security, dos-protection]
---

# Rate Limiting Configuration

Configure per-client rate limiting for the HTTP server (port 15500): REST,
`/api/v1/command`, MCP (`/mcp`), UMICP (`/umicp`) and the WebSocket upgrades.
The RESP3 and SynapRPC listeners are not covered; they have their own
connection limits under `network`.

## Overview

When `rate_limit.enabled` is `true`, Synap enforces a **token bucket per
client**:

- Each bucket holds up to `burst_size` tokens and refills at
  `requests_per_second`. Every request spends one token.
- A request that finds its bucket empty is answered **429 Too Many Requests**
  with a `Retry-After` header.
- The limit applies **before authentication**, so password checks (bcrypt) and
  API key checks are bounded too, whether they succeed or fail.
- `/health` and `/metrics` are **exempt**, so liveness probes and Prometheus
  scrapers are never throttled into reporting an outage.

With `enabled: false` (the default) no request is rate limited.

## Configuration

```yaml
rate_limit:
  enabled: true
  # Sustained requests per second per client (refill rate)
  requests_per_second: 1000
  # Requests a client may send at once (bucket capacity)
  burst_size: 100
```

| Option | Default | Description |
|--------|---------|-------------|
| `enabled` | `false` | Install the limiter |
| `requests_per_second` | `1000` | Tokens added to each bucket per second |
| `burst_size` | `100` | Bucket capacity: the most requests a client can send at once |

With `enabled: true`, both `requests_per_second` and `burst_size` must be at
least 1; the server refuses to start otherwise, since such a limiter would
reject every request or never let a limited client back in.

### Recommended Settings

**Public API:**
```yaml
rate_limit:
  enabled: true
  requests_per_second: 100
  burst_size: 200
```

**Internal API:**
```yaml
rate_limit:
  enabled: true
  requests_per_second: 10000
  burst_size: 20000
```

## How Clients Are Identified

Every request is checked against up to two buckets, and is served only if
each has a token:

1. **Peer address, before authentication.** The bucket of the TCP peer IP
   address. Forwarding headers such as `X-Forwarded-For` are ignored, because
   the client writes them. This is the check that bounds authentication
   attempts.
2. **Principal, after authentication.** A request that authenticated also
   spends a token from its principal's bucket: the user for Basic auth or a
   user-owned API key, the key itself for a key without a user, and the tenant
   (`hub:<uuid>`) in Hub mode. One principal is therefore bounded across every
   address it connects from. Anonymous requests have no principal bucket.

Both buckets use the same `requests_per_second` and `burst_size`.

**Behind a reverse proxy or load balancer**, every client reaches Synap from
the proxy's address and shares one peer bucket. Either rate limit at the proxy
and leave `rate_limit.enabled: false`, or size `burst_size` and
`requests_per_second` for the combined traffic of the proxy.

Buckets that have refilled to capacity carry no state and are pruned once
there are more than 10,000, so memory stays bounded as client addresses churn.

## Example

```yaml
rate_limit:
  enabled: true
  requests_per_second: 10
  burst_size: 5
```

- A client sending 20 requests at once gets 5 answered and 15 refused with 429.
- A new token arrives every 100 ms, so after the burst the client can sustain
  10 requests per second.
- Another client is unaffected: its bucket is its own.

## Rate Limit Response

**HTTP Status:** `429 Too Many Requests`

**Headers:** `Retry-After: <seconds>`, the whole number of seconds (at least 1)
until the bucket holds a token again.

**Body:**
```json
{
  "error": "Rate limit exceeded; retry after 1 s",
  "code": 429
}
```

## Startup Log

The server logs what it actually installed:

```text
Rate limiting enforced: 1000 req/s, burst 100, per client address before authentication and per authenticated principal (/health and /metrics exempt)
```

or, with `enabled: false`:

```text
Rate limiting disabled: requests are not rate limited
```

Refused requests are logged at `debug` level only, so a client hammering the
limit cannot flood the log.

## Best Practices

### Size the Burst for Real Clients

`burst_size` is what a client can send at once (page loads, batch jobs,
reconnect storms); `requests_per_second` is what it can sustain. A burst of
about twice the per-second rate absorbs normal spikes.

### Honor Retry-After

Clients should wait the `Retry-After` seconds before retrying a 429 rather
than retrying immediately, which only spends the next token as soon as it
arrives:

```python
import time
import requests

def get_with_retry(url, attempts=3):
    for _ in range(attempts):
        response = requests.get(url)
        if response.status_code != 429:
            return response
        time.sleep(int(response.headers.get("Retry-After", "1")))
    return response
```

### Per-Endpoint Limits

The limit is the same for every route. For per-endpoint limits, use a reverse
proxy:

```nginx
location /kv/ {
    limit_req zone=kv_limit burst=50;
    proxy_pass http://synap:15500;
}
```

## Troubleshooting

### Legitimate Clients Get 429

1. Increase `requests_per_second` and/or `burst_size`.
2. If Synap sits behind a proxy, all clients share the proxy's address; see
   [How Clients Are Identified](#how-clients-are-identified).

### Rate Limiting Not Applied

1. Check the startup log: it says `Rate limiting enforced` only when the
   limiter is installed.
2. Verify `rate_limit.enabled: true` and restart the server; the setting is
   read at startup.
3. `/health` and `/metrics` are never limited, by design.

## Related Topics

- [Security Guide](../guides/SECURITY.md) - Security best practices
- [Configuration Overview](./CONFIGURATION.md) - Complete configuration reference
- [Monitoring](../operations/MONITORING.md) - Monitor rate limit metrics
- [Troubleshooting](../operations/TROUBLESHOOTING.md) - Common problems

