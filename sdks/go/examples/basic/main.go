// Package main demonstrates basic usage of the Synap Go SDK.
package main

import (
	"context"
	"fmt"
	"log"
	"time"

	synap "github.com/hivellm/synap/sdks/go"
)

func main() {
	// ── Client setup ─────────────────────────────────────────────────────────

	cfg := synap.NewConfig("http://localhost:15500").
		WithTimeout(10 * time.Second)
		// Optionally add auth:
		// WithAuth("your-bearer-token")
		// WithBasicAuth("user", "pass")

	client := synap.NewClient(cfg)
	ctx := context.Background()

	// ── KV store ─────────────────────────────────────────────────────────────

	fmt.Println("--- KV store ---")
	if err := client.KV().Set(ctx, "greeting", "hello world", 0); err != nil {
		log.Fatalf("KV Set: %v", err)
	}
	val, err := client.KV().Get(ctx, "greeting")
	if err != nil {
		log.Fatalf("KV Get: %v", err)
	}
	fmt.Printf("greeting = %q\n", val)

	// Set with TTL
	if err := client.KV().Set(ctx, "session:abc", "token123", time.Hour); err != nil {
		log.Fatalf("KV Set TTL: %v", err)
	}

	// Increment / decrement
	client.KV().Set(ctx, "counter", "0", 0) //nolint:errcheck
	count, _ := client.KV().Incr(ctx, "counter")
	fmt.Printf("counter after incr = %d\n", count)

	deleted, _ := client.KV().Delete(ctx, "greeting")
	fmt.Printf("deleted greeting = %v\n", deleted)

	stats, _ := client.KV().Stats(ctx)
	fmt.Printf("KV stats: total_keys=%d hit_rate=%.2f\n", stats.TotalKeys, stats.HitRate)

	// ── Queue ─────────────────────────────────────────────────────────────────

	fmt.Println("\n--- Queue ---")
	if err := client.Queue().Create(ctx, "tasks", 1000, 30); err != nil {
		log.Fatalf("Queue Create: %v", err)
	}
	msgID, err := client.Queue().Publish(ctx, "tasks", []byte(`{"type":"send-email","to":"alice@example.com"}`), 5, 3)
	if err != nil {
		log.Fatalf("Queue Publish: %v", err)
	}
	fmt.Printf("published message: %s\n", msgID)

	msg, err := client.Queue().Consume(ctx, "tasks", "worker-1")
	if err != nil {
		log.Fatalf("Queue Consume: %v", err)
	}
	if msg != nil {
		fmt.Printf("consumed: id=%s payload=%s\n", msg.ID, string(msg.Payload))
		if err := client.Queue().Ack(ctx, "tasks", msg.ID); err != nil {
			log.Fatalf("Queue Ack: %v", err)
		}
		fmt.Println("acked message")
	}

	queues, _ := client.Queue().List(ctx)
	fmt.Printf("queues: %v\n", queues)

	// ── Stream ────────────────────────────────────────────────────────────────

	fmt.Println("\n--- Stream ---")
	if err := client.Stream().Create(ctx, "events", 10000); err != nil {
		log.Fatalf("Stream Create: %v", err)
	}
	offset, err := client.Stream().Publish(ctx, "events", "user.login", map[string]interface{}{
		"user_id": 42,
		"ip":      "192.168.1.1",
	})
	if err != nil {
		log.Fatalf("Stream Publish: %v", err)
	}
	fmt.Printf("published event at offset %d\n", offset)

	events, err := client.Stream().Consume(ctx, "events", 0, 10)
	if err != nil {
		log.Fatalf("Stream Consume: %v", err)
	}
	for _, e := range events {
		fmt.Printf("event[%d] %s: %v\n", e.Offset, e.Event, e.Data)
	}

	// ── PubSub ────────────────────────────────────────────────────────────────

	fmt.Println("\n--- PubSub ---")
	subID, err := client.PubSub().Subscribe(ctx, "svc-notifications", []string{"user.*", "order.#"})
	if err != nil {
		log.Fatalf("PubSub Subscribe: %v", err)
	}
	fmt.Printf("subscribed as %s\n", subID)

	n, err := client.PubSub().Publish(ctx, "user.created", map[string]interface{}{"id": 1, "name": "Alice"}, 5)
	if err != nil {
		log.Fatalf("PubSub Publish: %v", err)
	}
	fmt.Printf("delivered to %d subscribers\n", n)

	if err := client.PubSub().Unsubscribe(ctx, "svc-notifications", []string{"user.*"}); err != nil {
		log.Fatalf("PubSub Unsubscribe: %v", err)
	}

	// ── Hash ──────────────────────────────────────────────────────────────────

	fmt.Println("\n--- Hash ---")
	client.Hash().Set(ctx, "user:1", "name", "Alice")   //nolint:errcheck
	client.Hash().Set(ctx, "user:1", "email", "alice@example.com") //nolint:errcheck
	client.Hash().Set(ctx, "user:1", "age", "30")        //nolint:errcheck

	name, _ := client.Hash().Get(ctx, "user:1", "name")
	fmt.Printf("user:1 name = %s\n", name)

	all, _ := client.Hash().GetAll(ctx, "user:1")
	fmt.Printf("user:1 fields = %v\n", all)

	// ── List ──────────────────────────────────────────────────────────────────

	fmt.Println("\n--- List ---")
	client.List().RPush(ctx, "myqueue", []string{"job-1", "job-2", "job-3"}) //nolint:errcheck
	jobs, _ := client.List().LPop(ctx, "myqueue", 1)
	fmt.Printf("dequeued: %v\n", jobs)
	l, _ := client.List().Len(ctx, "myqueue")
	fmt.Printf("remaining: %d\n", l)

	// ── Set ───────────────────────────────────────────────────────────────────

	fmt.Println("\n--- Set ---")
	client.Set().Add(ctx, "tags", []string{"go", "redis", "synap"}) //nolint:errcheck
	tags, _ := client.Set().Members(ctx, "tags")
	fmt.Printf("tags: %v\n", tags)
	has, _ := client.Set().IsMember(ctx, "tags", "go")
	fmt.Printf("has 'go': %v\n", has)
	card, _ := client.Set().Card(ctx, "tags")
	fmt.Printf("cardinality: %d\n", card)
}
