package synap

import (
	"context"
	"encoding/json"
)

// PubSubManager provides pub/sub operations against the Synap server.
type PubSubManager struct {
	client *SynapClient
}

// Publish sends data to a topic. priority is 0–9 (0 = default).
// Returns the number of subscribers that received the message.
func (p *PubSubManager) Publish(ctx context.Context, topic string, data interface{}, priority uint8) (int, error) {
	type payload struct {
		Topic    string      `json:"topic"`
		Payload  interface{} `json:"payload"`
		Priority uint8       `json:"priority"`
	}
	raw, err := p.client.sendCommand(ctx, "pubsub.publish", payload{Topic: topic, Payload: data, Priority: priority})
	if err != nil {
		return 0, err
	}
	var result struct {
		SubscribersMatched int `json:"subscribers_matched"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return 0, newInvalidResponseError("pubsub.publish: " + err.Error())
	}
	return result.SubscribersMatched, nil
}

// Subscribe registers subscriberID to receive messages on the given topics.
// Wildcard patterns are supported: "user.*" (single-level), "user.#" (multi-level).
// Returns the subscriber ID confirmed by the server.
func (p *PubSubManager) Subscribe(ctx context.Context, subscriberID string, topics []string) (string, error) {
	type payload struct {
		SubscriberID string   `json:"subscriber_id"`
		Topics       []string `json:"topics"`
	}
	raw, err := p.client.sendCommand(ctx, "pubsub.subscribe", payload{SubscriberID: subscriberID, Topics: topics})
	if err != nil {
		return "", err
	}
	var result struct {
		SubscriberID   string `json:"subscriber_id"`
		SubscriptionID string `json:"subscription_id"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return "", newInvalidResponseError("pubsub.subscribe: " + err.Error())
	}
	if result.SubscriberID != "" {
		return result.SubscriberID, nil
	}
	return result.SubscriptionID, nil
}

// Unsubscribe removes subscriberID from the given topics.
func (p *PubSubManager) Unsubscribe(ctx context.Context, subscriberID string, topics []string) error {
	type payload struct {
		SubscriberID string   `json:"subscriber_id"`
		Topics       []string `json:"topics"`
	}
	_, err := p.client.sendCommand(ctx, "pubsub.unsubscribe", payload{SubscriberID: subscriberID, Topics: topics})
	return err
}

// ListTopics returns all active topics on the server.
func (p *PubSubManager) ListTopics(ctx context.Context) ([]string, error) {
	raw, err := p.client.sendCommand(ctx, "pubsub.topics", struct{}{})
	if err != nil {
		return nil, err
	}
	var result struct {
		Topics []string `json:"topics"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, newInvalidResponseError("pubsub.topics: " + err.Error())
	}
	return result.Topics, nil
}
