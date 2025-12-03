---
title: Rate Limiting Configuration
module: configuration
id: rate-limiting
order: 6
description: Configure rate limiting to protect against DoS attacks
tags: [configuration, rate-limiting, security, dos-protection]
---

# Rate Limiting Configuration

Configure rate limiting to protect Synap against DoS attacks and excessive API usage.

## Overview

Synap implements **token bucket rate limiting** with:

- **Per-IP rate limiting** - Each IP address has its own rate limit
- **Configurable requests per second** - Set maximum request rate
- **Burst capacity** - Allow temporary spikes above rate limit
- **Automatic cleanup** - Old entries are cleaned up periodically

## Configuration

### Enable Rate Limiting

```yaml
rate_limit:
  # Enable rate limiting (token bucket algorithm)
  enabled: true
  
  # Maximum requests per second per IP
  requests_per_second: 1000
  
  # Burst size (maximum tokens in bucket)
  burst_size: 100
```

### Configuration Options

| Option | Default | Description |
|--------|---------|-------------|
| `enabled` | `false` | Enable/disable rate limiting |
| `requests_per_second` | `1000` | Maximum requests per second per IP |
| `burst_size` | `100` | Maximum tokens in bucket (allows spikes) |

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

**High-Traffic Production:**
```yaml
rate_limit:
  enabled: true
  requests_per_second: 5000
  burst_size: 10000
```

## How It Works

### Token Bucket Algorithm

1. **Token Refill**: Tokens are refilled at `requests_per_second` rate
2. **Token Consumption**: Each request consumes 1 token
3. **Burst Handling**: Up to `burst_size` tokens can accumulate
4. **Rate Limit Exceeded**: Returns `429 Too Many Requests` when no tokens available

### Example

```yaml
rate_limit:
  requests_per_second: 100
  burst_size: 200
```

**Behavior:**
- Normal rate: 100 requests/second
- Burst: Can handle up to 200 requests in quick succession
- After burst: Must wait for tokens to refill

## Response Codes

### Rate Limit Exceeded

When rate limit is exceeded:

**HTTP Status:** `429 Too Many Requests`

**Response:**
```json
{
  "success": false,
  "error": {
    "type": "RateLimitExceeded",
    "message": "Rate limit exceeded. Please try again later.",
    "status_code": 429
  }
}
```

## Monitoring

### Check Rate Limit Status

Rate limiting is tracked per IP. Monitor with:

```bash
# Check server logs
tail -f /var/log/synap/synap.log | grep "Rate limit"

# Check metrics (if Prometheus enabled)
curl http://localhost:15500/metrics | grep rate_limit
```

## Best Practices

### 1. Set Appropriate Limits

**Too Low:**
- Legitimate users get rate limited
- Poor user experience

**Too High:**
- Doesn't protect against attacks
- Wastes resources

**Recommended:**
- Start with conservative limits
- Monitor and adjust based on traffic patterns
- Use different limits for different endpoints if needed

### 2. Handle Rate Limit Errors

**Client-Side:**
```python
import time
from synap import SynapClient

client = SynapClient("http://localhost:15500")

def make_request_with_retry():
    max_retries = 3
    retry_delay = 1
    
    for attempt in range(max_retries):
        try:
            return client.kv.get("key")
        except RateLimitError:
            if attempt < max_retries - 1:
                time.sleep(retry_delay * (attempt + 1))
            else:
                raise
```

**Server-Side:**
```python
from fastapi import HTTPException

try:
    result = synap_client.kv.get(key)
except RateLimitError:
    raise HTTPException(
        status_code=429,
        detail="Rate limit exceeded. Please try again later."
    )
```

### 3. Use Burst Size Wisely

**Small Burst:**
- Stricter rate limiting
- Better protection
- May reject legitimate bursts

**Large Burst:**
- More lenient
- Better user experience
- Less protection

**Recommended:** `burst_size = 2 * requests_per_second`

### 4. Monitor and Adjust

**Monitor:**
- Rate limit hit frequency
- IP addresses hitting limits
- Traffic patterns

**Adjust:**
- Increase limits for legitimate traffic
- Decrease limits for abusive traffic
- Consider per-endpoint limits

## Advanced Configuration

### Per-Endpoint Rate Limiting

Currently, rate limiting is global. For per-endpoint limits, use a reverse proxy:

**Nginx Example:**
```nginx
# Different limits for different endpoints
location /kv/ {
    limit_req zone=kv_limit burst=50;
    proxy_pass http://synap:15500;
}

location /queue/ {
    limit_req zone=queue_limit burst=20;
    proxy_pass http://synap:15500;
}
```

### Rate Limiting with Authentication

Rate limiting works with authentication:

- **Authenticated users**: Rate limited by IP
- **API keys**: Can have different limits (future feature)
- **Admin users**: Can bypass rate limits (future feature)

## Troubleshooting

### Rate Limits Too Strict

**Problem:** Legitimate users getting rate limited.

**Solution:**
1. Increase `requests_per_second`
2. Increase `burst_size`
3. Check if traffic is legitimate or abusive

### Rate Limits Not Working

**Problem:** Rate limiting enabled but not blocking requests.

**Solution:**
1. Verify `enabled: true` in config
2. Check server logs for rate limit messages
3. Verify middleware is properly configured
4. Restart server after config changes

### High Memory Usage

**Problem:** Rate limiter using too much memory.

**Solution:**
1. Automatic cleanup runs every 5 minutes
2. Old entries (>5 minutes inactive) are removed
3. For high-traffic, consider reducing cleanup interval

## Related Topics

- [Security Guide](../guides/SECURITY.md) - Security best practices
- [Configuration Overview](./CONFIGURATION.md) - Complete configuration reference
- [Monitoring](../operations/MONITORING.md) - Monitor rate limit metrics
- [Troubleshooting](../operations/TROUBLESHOOTING.md) - Common problems

