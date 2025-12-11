---
title: Lua Scripting
module: guides
id: lua-scripting
order: 5
description: Server-side scripting with EVAL/EVALSHA
tags: [guides, lua, scripting, eval, evalsha]
---

# Lua Scripting

Complete guide to server-side Lua scripting in Synap.

## Overview

Synap supports Redis-compatible Lua scripting:
- **EVAL**: Execute Lua script
- **EVALSHA**: Execute script by SHA1 hash
- **redis.call()**: Call Synap operations from Lua
- **Script Caching**: Automatic caching of scripts

## Basic Usage

### EVAL - Execute Script

```bash
curl -X POST http://localhost:15500/script/eval \
  -H "Content-Type: application/json" \
  -d '{
    "script": "return redis.call(\"get\", KEYS[1])",
    "keys": ["user:1"],
    "args": []
  }'
```

### Using SDKs

**Python:**
```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")

# Execute script
result = client.eval(
    "return redis.call('get', KEYS[1])",
    keys=["user:1"]
)
```

**TypeScript:**
```typescript
const client = new SynapClient("http://localhost:15500");

const result = await client.eval(
    "return redis.call('get', KEYS[1])",
    ["user:1"]
);
```

**Rust:**
```rust
let client = SynapClient::new("http://localhost:15500")?;

let result = client.eval(
    "return redis.call('get', KEYS[1])",
    vec!["user:1".to_string()],
    vec![]
).await?;
```

## EVALSHA - Execute by Hash

### Get Script SHA

```bash
# Execute script and get SHA
curl -X POST http://localhost:15500/script/load \
  -H "Content-Type: application/json" \
  -d '{
    "script": "return redis.call(\"get\", KEYS[1])"
  }'
```

**Response:**
```json
{
  "sha": "abc123def456..."
}
```

### Execute by SHA

```bash
curl -X POST http://localhost:15500/script/evalsha \
  -H "Content-Type: application/json" \
  -d '{
    "sha": "abc123def456...",
    "keys": ["user:1"],
    "args": []
  }'
```

## redis.call()

### Available Commands

All Synap operations are available via `redis.call()`:

```lua
-- Key-Value
redis.call("set", "key", "value")
redis.call("get", "key")
redis.call("del", "key")

-- Hash
redis.call("hset", "hash", "field", "value")
redis.call("hget", "hash", "field")

-- List
redis.call("lpush", "list", "value")
redis.call("rpop", "list")

-- Set
redis.call("sadd", "set", "member")
redis.call("smembers", "set")

-- Sorted Set
redis.call("zadd", "zset", 100, "member")
redis.call("zrange", "zset", 0, -1)
```

## Example Scripts

### Atomic Increment with Limit

```lua
local key = KEYS[1]
local limit = tonumber(ARGV[1])
local current = tonumber(redis.call("get", key) or "0")

if current < limit then
    return redis.call("incr", key)
else
    return nil
end
```

**Usage:**
```python
result = client.eval(
    """
    local key = KEYS[1]
    local limit = tonumber(ARGV[1])
    local current = tonumber(redis.call("get", key) or "0")
    
    if current < limit then
        return redis.call("incr", key)
    else
        return nil
    end
    """,
    keys=["counter"],
    args=["100"]
)
```

### Conditional Update

```lua
local key = KEYS[1]
local new_value = ARGV[1]
local expected = ARGV[2]

local current = redis.call("get", key)

if current == expected then
    redis.call("set", key, new_value)
    return 1
else
    return 0
end
```

### Batch Operations

```lua
local results = {}
for i = 1, #KEYS do
    results[i] = redis.call("get", KEYS[i])
end
return results
```

## Script Caching

### Automatic Caching

Scripts are automatically cached by SHA1 hash:

```python
# First execution - script is cached
result1 = client.eval(script, keys=["key1"])

# Subsequent executions use cached script
result2 = client.eval(script, keys=["key2"])  # Uses cache
```

### Manual Caching

```python
# Load script (returns SHA)
sha = client.script_load(script)

# Execute using SHA (faster)
result = client.evalsha(sha, keys=["key1"])
```

## Best Practices

### Keep Scripts Simple

- Avoid complex logic
- Use for atomic operations
- Keep execution time short

### Use KEYS and ARGV

```lua
-- Good: Use KEYS and ARGV
local key = KEYS[1]
local value = ARGV[1]
redis.call("set", key, value)

-- Bad: Hardcoded values
redis.call("set", "hardcoded", "value")
```

### Handle Errors

```lua
local ok, result = pcall(function()
    return redis.call("get", KEYS[1])
end)

if not ok then
    return {err = result}
else
    return result
end
```

## Related Topics

- [Transactions](./TRANSACTIONS.md) - Transaction support
- [Basic KV Operations](../kv-store/BASIC.md) - Basic operations

