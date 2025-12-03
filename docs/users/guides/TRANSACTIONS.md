---
title: Transactions
module: guides
id: transactions-guide
order: 4
description: MULTI/EXEC/WATCH transactions with optimistic locking
tags: [guides, transactions, multi, exec, watch]
---

# Transactions

Complete guide to transactions in Synap.

## Overview

Synap supports Redis-compatible transactions:
- **MULTI/EXEC**: Batch operations atomically
- **WATCH**: Optimistic locking
- **DISCARD**: Cancel transaction
- **Atomic Execution**: All or nothing

## Basic Transactions

### MULTI/EXEC

```bash
# Start transaction
curl -X POST http://localhost:15500/transaction/multi

# Queue operations
curl -X POST http://localhost:15500/transaction/queue \
  -H "Content-Type: application/json" \
  -d '{"command":"kv.set","args":{"key":"user:1","value":"John"}}'

curl -X POST http://localhost:15500/transaction/queue \
  -H "Content-Type: application/json" \
  -d '{"command":"kv.set","args":{"key":"user:2","value":"Jane"}}'

# Execute transaction
curl -X POST http://localhost:15500/transaction/exec
```

### Using SDKs

**Python:**
```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")

# Start transaction
tx = client.transaction()

# Queue operations
tx.set("user:1", "John")
tx.set("user:2", "Jane")
tx.incr("counter")

# Execute
results = tx.exec()
```

**TypeScript:**
```typescript
const client = new SynapClient("http://localhost:15500");

const tx = client.transaction();
tx.set("user:1", "John");
tx.set("user:2", "Jane");
tx.incr("counter");

const results = await tx.exec();
```

**Rust:**
```rust
let client = SynapClient::new("http://localhost:15500")?;

let mut tx = client.transaction();
tx.set("user:1", "John")?;
tx.set("user:2", "Jane")?;
tx.incr("counter")?;

let results = tx.exec().await?;
```

## WATCH - Optimistic Locking

### Watch Keys

```bash
# Watch key before transaction
curl -X POST http://localhost:15500/transaction/watch \
  -H "Content-Type: application/json" \
  -d '{"keys":["user:1","user:2"]}'

# Start transaction
curl -X POST http://localhost:15500/transaction/multi

# Queue operations
curl -X POST http://localhost:15500/transaction/queue \
  -H "Content-Type: application/json" \
  -d '{"command":"kv.set","args":{"key":"user:1","value":"John"}}'

# Execute (will fail if watched keys changed)
curl -X POST http://localhost:15500/transaction/exec
```

### Using SDKs

**Python:**
```python
# Watch keys
client.watch("user:1", "user:2")

# Start transaction
tx = client.transaction()

# Queue operations
tx.set("user:1", "John")
tx.set("user:2", "Jane")

# Execute (fails if watched keys changed)
try:
    results = tx.exec()
except TransactionError as e:
    print("Transaction failed:", e)
```

## DISCARD - Cancel Transaction

### Discard Transaction

```bash
# Start transaction
curl -X POST http://localhost:15500/transaction/multi

# Queue operations
curl -X POST http://localhost:15500/transaction/queue \
  -H "Content-Type: application/json" \
  -d '{"command":"kv.set","args":{"key":"user:1","value":"John"}}'

# Discard (cancel transaction)
curl -X POST http://localhost:15500/transaction/discard
```

### Using SDKs

**Python:**
```python
tx = client.transaction()
tx.set("user:1", "John")

# Discard transaction
tx.discard()
```

## Transaction Errors

### Error Handling

If any command in transaction fails, entire transaction is rolled back:

```python
tx = client.transaction()
tx.set("user:1", "John")
tx.set("invalid:key", "value")  # This might fail
tx.set("user:2", "Jane")

try:
    results = tx.exec()
except TransactionError as e:
    print("Transaction failed:", e)
    # All operations rolled back
```

## Use Cases

### Atomic Updates

```python
# Update multiple keys atomically
tx = client.transaction()
tx.set("user:1:balance", "100")
tx.set("user:2:balance", "200")
tx.incr("total_balance")
results = tx.exec()
```

### Conditional Updates

```python
# Watch key before updating
client.watch("user:1:version")

# Get current version
version = client.kv.get("user:1:version")

# Start transaction
tx = client.transaction()
tx.set("user:1:data", new_data)
tx.incr("user:1:version")

# Execute (fails if version changed)
try:
    results = tx.exec()
except TransactionError:
    # Retry or handle conflict
    pass
```

## Best Practices

### Keep Transactions Short

- Minimize time between MULTI and EXEC
- Avoid long-running operations in transactions
- Use WATCH for optimistic locking

### Handle Failures

```python
def update_with_retry(key, value, max_retries=3):
    for attempt in range(max_retries):
        try:
            client.watch(key)
            tx = client.transaction()
            tx.set(key, value)
            tx.exec()
            return True
        except TransactionError:
            if attempt == max_retries - 1:
                raise
            time.sleep(0.1)
    return False
```

### Use WATCH for Critical Updates

```python
# Watch balance before transfer
client.watch("account:1:balance", "account:2:balance")

tx = client.transaction()
tx.decrby("account:1:balance", 100)
tx.incrby("account:2:balance", 100)
tx.exec()  # Fails if balances changed
```

## Related Topics

- [Basic KV Operations](../kv-store/BASIC.md) - Basic operations
- [Advanced KV Operations](../kv-store/ADVANCED.md) - Advanced features

