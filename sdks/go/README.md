# Synap Go SDK

Go client SDK for [Synap](https://github.com/hivellm/synap) — a high-performance in-memory data engine.

Communicates with the Synap server via its StreamableHTTP endpoint (`POST /api/v1/command`).

## Requirements

- Go 1.22+
- No external dependencies — stdlib only

## Installation

```bash
go get github.com/hivellm/synap-sdk-go
```

## Quick start

```go
package main

import (
    "context"
    "fmt"
    "time"

    synap "github.com/hivellm/synap-sdk-go"
)

func main() {
    cfg := synap.NewConfig("http://localhost:15500").
        WithTimeout(10 * time.Second)
    client := synap.NewClient(cfg)

    ctx := context.Background()
    client.KV().Set(ctx, "hello", "world", 0)
    val, _ := client.KV().Get(ctx, "hello")
    fmt.Println(val) // world
}
```

## Configuration

```go
cfg := synap.NewConfig("http://localhost:15500")

// Bearer token auth
cfg.WithAuth("your-api-token")

// HTTP Basic Auth
cfg.WithBasicAuth("username", "password")

// Custom timeout (default: 30s)
cfg.WithTimeout(5 * time.Second)
```

## KV Store

```go
kv := client.KV()

// Set with optional TTL
kv.Set(ctx, "key", "value", 0)            // no expiry
kv.Set(ctx, "session", "tok", time.Hour)  // expires in 1 hour

// Get — returns ("", nil) when key is absent
val, err := kv.Get(ctx, "key")

// Delete — returns true if the key existed
deleted, err := kv.Delete(ctx, "key")

// Check existence
exists, err := kv.Exists(ctx, "key")

// Atomic increment / decrement
newVal, err := kv.Incr(ctx, "counter")
newVal, err  = kv.Decr(ctx, "counter")

// Statistics
stats, err := kv.Stats(ctx)
fmt.Println(stats.TotalKeys, stats.HitRate)
```

## Queue

```go
q := client.Queue()

// Create (maxDepth 0 = unlimited, ackDeadline 0 = server default)
q.Create(ctx, "tasks", 1000, 30)

// Publish — returns message ID
msgID, err := q.Publish(ctx, "tasks", []byte("payload"), 5, 3)

// Consume — returns nil, nil when queue is empty
msg, err := q.Consume(ctx, "tasks", "worker-1")
if msg != nil {
    fmt.Println(msg.ID, string(msg.Payload))
    q.Ack(ctx, "tasks", msg.ID)   // success
    // or
    q.Nack(ctx, "tasks", msg.ID)  // requeue
}

// Stats
stats, err := q.Stats(ctx, "tasks")
fmt.Println(stats.Depth, stats.Published, stats.Acked)

// Enumerate
queues, err := q.List(ctx)

// Remove
q.Delete(ctx, "tasks")
```

## Stream

```go
s := client.Stream()

// Create (maxEvents 0 = unlimited retention)
s.Create(ctx, "events", 10000)

// Publish — returns the assigned offset
offset, err := s.Publish(ctx, "events", "user.login", map[string]interface{}{
    "user_id": 42,
})

// Consume starting at offset 0, up to 100 events
events, err := s.Consume(ctx, "events", 0, 100)
for _, e := range events {
    fmt.Println(e.Offset, e.Event, e.Data)
}

// Stats
stats, err := s.Stats(ctx, "events")

// Enumerate / remove
rooms, err := s.List(ctx)
s.Delete(ctx, "events")
```

## PubSub

```go
ps := client.PubSub()

// Subscribe (supports wildcards: "user.*", "order.#")
subID, err := ps.Subscribe(ctx, "my-service", []string{"user.*", "order.#"})

// Publish — returns number of subscribers reached
n, err := ps.Publish(ctx, "user.created", map[string]interface{}{"id": 1}, 5)

// Unsubscribe
ps.Unsubscribe(ctx, "my-service", []string{"user.*"})

// List active topics
topics, err := ps.ListTopics(ctx)
```

## Hash

```go
h := client.Hash()

h.Set(ctx, "user:1", "name", "Alice")
h.Set(ctx, "user:1", "age", "30")

name, err := h.Get(ctx, "user:1", "name")      // "Alice"
all,  err  := h.GetAll(ctx, "user:1")           // map[string]string
ok,   err  := h.Exists(ctx, "user:1", "name")   // true
n,    err  := h.Del(ctx, "user:1", "name")       // 1
```

## List

```go
l := client.List()

l.LPush(ctx, "queue", []string{"a", "b"})
l.RPush(ctx, "queue", []string{"c"})

vals, err := l.LPop(ctx, "queue", 1)   // ["a"]
vals, err  = l.RPop(ctx, "queue", 1)   // ["c"]
vals, err  = l.Range(ctx, "queue", 0, -1)
n,    err  := l.Len(ctx, "queue")
```

## Set

```go
s := client.Set()

n,  err := s.Add(ctx, "tags", []string{"go", "redis"})
members, err := s.Members(ctx, "tags")
ok,  err  := s.IsMember(ctx, "tags", "go")
n,   err  = s.Remove(ctx, "tags", []string{"redis"})
card, err := s.Card(ctx, "tags")
```

## Error handling

All operations return a standard `error`. Server errors wrap a `*SynapError` value
with `Code` and `Message` fields:

```go
val, err := client.KV().Get(ctx, "key")
if err != nil {
    var synapErr *synap.SynapError
    if errors.As(err, &synapErr) {
        fmt.Println("code:", synapErr.Code)
        fmt.Println("message:", synapErr.Message)
    }
}
```

## Wire format

All requests are `POST /api/v1/command` with JSON body:

```json
{
  "command": "kv.set",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "payload": {"key": "k", "value": "v", "ttl": null}
}
```

Every response follows the same envelope:

```json
{
  "success": true,
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "payload": {...},
  "error": null
}
```

## Running the example

```bash
cd examples/basic
go run main.go
```

## Running tests

```bash
go test ./...
```
