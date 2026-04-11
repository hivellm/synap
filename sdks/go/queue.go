package synap

import (
	"context"
	"encoding/json"
)

// QueueManager provides queue operations against the Synap server.
type QueueManager struct {
	client *SynapClient
}

// Create creates a new queue.
// maxDepth 0 means unlimited; ackDeadline 0 means use the server default.
func (q *QueueManager) Create(ctx context.Context, name string, maxDepth int, ackDeadline int) error {
	type config struct {
		MaxDepth        *int `json:"max_depth,omitempty"`
		AckDeadlineSecs *int `json:"ack_deadline_secs,omitempty"`
	}
	type payload struct {
		Name   string  `json:"name"`
		Config *config `json:"config,omitempty"`
	}
	p := payload{Name: name}
	var cfg config
	hasConfig := false
	if maxDepth > 0 {
		cfg.MaxDepth = &maxDepth
		hasConfig = true
	}
	if ackDeadline > 0 {
		cfg.AckDeadlineSecs = &ackDeadline
		hasConfig = true
	}
	if hasConfig {
		p.Config = &cfg
	}
	_, err := q.client.sendCommand(ctx, "queue.create", p)
	return err
}

// Publish sends a message to the named queue. payload is the raw message bytes.
// priority is 0–9 (0 = default). maxRetries 0 means use the server default.
// Returns the assigned message ID.
func (q *QueueManager) Publish(ctx context.Context, name string, payload []byte, priority uint8, maxRetries uint32) (string, error) {
	// Server expects payload as a JSON array of byte values [104,101,...],
	// not as base64 (which is Go's default for []byte in JSON).
	intPayload := make([]int, len(payload))
	for i, b := range payload {
		intPayload[i] = int(b)
	}
	type body struct {
		Queue      string  `json:"queue"`
		Payload    []int   `json:"payload"`
		Priority   uint8   `json:"priority"`
		MaxRetries *uint32 `json:"max_retries,omitempty"`
	}
	b := body{Queue: name, Payload: intPayload, Priority: priority}
	if maxRetries > 0 {
		b.MaxRetries = &maxRetries
	}
	raw, err := q.client.sendCommand(ctx, "queue.publish", b)
	if err != nil {
		return "", err
	}
	var result struct {
		MessageID string `json:"message_id"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return "", newInvalidResponseError("queue.publish: " + err.Error())
	}
	return result.MessageID, nil
}

// Consume retrieves the next available message from the queue.
// Returns nil, nil when the queue is empty.
func (q *QueueManager) Consume(ctx context.Context, name, consumerID string) (*Message, error) {
	type payload struct {
		Queue      string `json:"queue"`
		ConsumerID string `json:"consumer_id"`
	}
	raw, err := q.client.sendCommand(ctx, "queue.consume", payload{Queue: name, ConsumerID: consumerID})
	if err != nil {
		return nil, err
	}
	// Server returns {"message": {...}} or {"message": null}
	var wrapper struct {
		Message *Message `json:"message"`
	}
	if err := json.Unmarshal(raw, &wrapper); err != nil {
		// Fallback: try to unmarshal directly as a Message.
		var msg Message
		if err2 := json.Unmarshal(raw, &msg); err2 != nil {
			return nil, newInvalidResponseError("queue.consume: " + err.Error())
		}
		if msg.ID == "" {
			return nil, nil
		}
		return &msg, nil
	}
	return wrapper.Message, nil
}

// Ack acknowledges successful processing of a message.
func (q *QueueManager) Ack(ctx context.Context, name, messageID string) error {
	type payload struct {
		Queue     string `json:"queue"`
		MessageID string `json:"message_id"`
	}
	_, err := q.client.sendCommand(ctx, "queue.ack", payload{Queue: name, MessageID: messageID})
	return err
}

// Nack negatively acknowledges a message, causing it to be requeued.
func (q *QueueManager) Nack(ctx context.Context, name, messageID string) error {
	type payload struct {
		Queue     string `json:"queue"`
		MessageID string `json:"message_id"`
	}
	_, err := q.client.sendCommand(ctx, "queue.nack", payload{Queue: name, MessageID: messageID})
	return err
}

// Stats returns statistics for the named queue.
func (q *QueueManager) Stats(ctx context.Context, name string) (QueueStats, error) {
	type payload struct {
		Queue string `json:"queue"`
	}
	raw, err := q.client.sendCommand(ctx, "queue.stats", payload{Queue: name})
	if err != nil {
		return QueueStats{}, err
	}
	var stats QueueStats
	if err := json.Unmarshal(raw, &stats); err != nil {
		return QueueStats{}, newInvalidResponseError("queue.stats: " + err.Error())
	}
	return stats, nil
}

// List returns the names of all queues.
func (q *QueueManager) List(ctx context.Context) ([]string, error) {
	raw, err := q.client.sendCommand(ctx, "queue.list", struct{}{})
	if err != nil {
		return nil, err
	}
	var result struct {
		Queues []string `json:"queues"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, newInvalidResponseError("queue.list: " + err.Error())
	}
	return result.Queues, nil
}

// Delete removes the named queue and all its messages.
func (q *QueueManager) Delete(ctx context.Context, name string) error {
	type payload struct {
		Queue string `json:"queue"`
	}
	_, err := q.client.sendCommand(ctx, "queue.delete", payload{Queue: name})
	return err
}
