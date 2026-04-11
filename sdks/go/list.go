package synap

import (
	"context"
	"encoding/json"
)

// ListManager provides list data structure operations against the Synap server.
// A list is a doubly-linked list with O(1) push/pop at both ends.
type ListManager struct {
	client *SynapClient
}

// LPush prepends values to the head of the list at key.
// Returns the new length of the list after all values are pushed.
func (l *ListManager) LPush(ctx context.Context, key string, values []string) (int, error) {
	type payload struct {
		Key    string   `json:"key"`
		Values []string `json:"values"`
	}
	raw, err := l.client.sendCommand(ctx, "list.lpush", payload{Key: key, Values: values})
	if err != nil {
		return 0, err
	}
	var result struct {
		Length int `json:"length"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return 0, newInvalidResponseError("list.lpush: " + err.Error())
	}
	return result.Length, nil
}

// RPush appends values to the tail of the list at key.
// Returns the new length of the list after all values are pushed.
func (l *ListManager) RPush(ctx context.Context, key string, values []string) (int, error) {
	type payload struct {
		Key    string   `json:"key"`
		Values []string `json:"values"`
	}
	raw, err := l.client.sendCommand(ctx, "list.rpush", payload{Key: key, Values: values})
	if err != nil {
		return 0, err
	}
	var result struct {
		Length int `json:"length"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return 0, newInvalidResponseError("list.rpush: " + err.Error())
	}
	return result.Length, nil
}

// LPop removes and returns count elements from the head of the list at key.
// count 0 is treated as 1 by the server.
func (l *ListManager) LPop(ctx context.Context, key string, count int) ([]string, error) {
	type payload struct {
		Key   string `json:"key"`
		Count *int   `json:"count,omitempty"`
	}
	p := payload{Key: key}
	if count > 0 {
		p.Count = &count
	}
	raw, err := l.client.sendCommand(ctx, "list.lpop", p)
	if err != nil {
		return nil, err
	}
	var result struct {
		Values []string `json:"values"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, newInvalidResponseError("list.lpop: " + err.Error())
	}
	return result.Values, nil
}

// RPop removes and returns count elements from the tail of the list at key.
// count 0 is treated as 1 by the server.
func (l *ListManager) RPop(ctx context.Context, key string, count int) ([]string, error) {
	type payload struct {
		Key   string `json:"key"`
		Count *int   `json:"count,omitempty"`
	}
	p := payload{Key: key}
	if count > 0 {
		p.Count = &count
	}
	raw, err := l.client.sendCommand(ctx, "list.rpop", p)
	if err != nil {
		return nil, err
	}
	var result struct {
		Values []string `json:"values"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, newInvalidResponseError("list.rpop: " + err.Error())
	}
	return result.Values, nil
}

// Range returns the elements of the list at key between indices start and stop
// (inclusive, zero-based). Negative indices count from the tail: -1 is the
// last element.
func (l *ListManager) Range(ctx context.Context, key string, start, stop int) ([]string, error) {
	type payload struct {
		Key   string `json:"key"`
		Start int    `json:"start"`
		Stop  int    `json:"stop"`
	}
	raw, err := l.client.sendCommand(ctx, "list.range", payload{Key: key, Start: start, Stop: stop})
	if err != nil {
		return nil, err
	}
	var result struct {
		Values []string `json:"values"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, newInvalidResponseError("list.range: " + err.Error())
	}
	return result.Values, nil
}

// Len returns the number of elements in the list at key.
func (l *ListManager) Len(ctx context.Context, key string) (int, error) {
	type payload struct {
		Key string `json:"key"`
	}
	raw, err := l.client.sendCommand(ctx, "list.len", payload{Key: key})
	if err != nil {
		return 0, err
	}
	var result struct {
		Length int `json:"length"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return 0, newInvalidResponseError("list.len: " + err.Error())
	}
	return result.Length, nil
}
