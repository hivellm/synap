package synap

import (
	"context"
	"encoding/json"
)

// StreamManager provides event stream operations against the Synap server.
type StreamManager struct {
	client *SynapClient
}

// Create creates a new stream room. maxEvents 0 means unlimited retention.
func (s *StreamManager) Create(ctx context.Context, room string, maxEvents int) error {
	type payload struct {
		Room      string `json:"room"`
		MaxEvents *int   `json:"max_events,omitempty"`
	}
	p := payload{Room: room}
	if maxEvents > 0 {
		p.MaxEvents = &maxEvents
	}
	_, err := s.client.sendCommand(ctx, "stream.create", p)
	return err
}

// Publish appends an event to the stream room and returns the assigned offset.
// data must be JSON-serialisable.
func (s *StreamManager) Publish(ctx context.Context, room, eventType string, data interface{}) (uint64, error) {
	type payload struct {
		Room  string      `json:"room"`
		Event string      `json:"event"`
		Data  interface{} `json:"data"`
	}
	raw, err := s.client.sendCommand(ctx, "stream.publish", payload{Room: room, Event: eventType, Data: data})
	if err != nil {
		return 0, err
	}
	var result struct {
		Offset uint64 `json:"offset"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return 0, newInvalidResponseError("stream.publish: " + err.Error())
	}
	return result.Offset, nil
}

// Consume reads up to limit events starting at offset from the stream room.
// offset 0 reads from the beginning.
func (s *StreamManager) Consume(ctx context.Context, room string, offset uint64, limit int) ([]Event, error) {
	type payload struct {
		Room         string `json:"room"`
		SubscriberID string `json:"subscriber_id"`
		FromOffset   uint64 `json:"from_offset"`
		Limit        int    `json:"limit"`
	}
	p := payload{
		Room:         room,
		SubscriberID: "sdk-default",
		FromOffset:   offset,
		Limit:        limit,
	}
	raw, err := s.client.sendCommand(ctx, "stream.consume", p)
	if err != nil {
		return nil, err
	}
	var result struct {
		Events []json.RawMessage `json:"events"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, newInvalidResponseError("stream.consume: " + err.Error())
	}
	events := make([]Event, 0, len(result.Events))
	for _, rawEvent := range result.Events {
		var e Event
		if err := json.Unmarshal(rawEvent, &e); err != nil {
			return nil, newInvalidResponseError("stream.consume: decode event: " + err.Error())
		}
		events = append(events, e)
	}
	return events, nil
}

// Stats returns statistics for the named stream room.
func (s *StreamManager) Stats(ctx context.Context, room string) (StreamStats, error) {
	type payload struct {
		Room string `json:"room"`
	}
	raw, err := s.client.sendCommand(ctx, "stream.stats", payload{Room: room})
	if err != nil {
		return StreamStats{}, err
	}
	var stats StreamStats
	if err := json.Unmarshal(raw, &stats); err != nil {
		return StreamStats{}, newInvalidResponseError("stream.stats: " + err.Error())
	}
	return stats, nil
}

// List returns the names of all stream rooms.
func (s *StreamManager) List(ctx context.Context) ([]string, error) {
	raw, err := s.client.sendCommand(ctx, "stream.list", struct{}{})
	if err != nil {
		return nil, err
	}
	var result struct {
		Rooms []string `json:"rooms"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, newInvalidResponseError("stream.list: " + err.Error())
	}
	return result.Rooms, nil
}

// Delete removes the named stream room and all its events.
func (s *StreamManager) Delete(ctx context.Context, room string) error {
	type payload struct {
		Room string `json:"room"`
	}
	_, err := s.client.sendCommand(ctx, "stream.delete", payload{Room: room})
	return err
}
