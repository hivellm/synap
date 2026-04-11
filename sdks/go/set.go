package synap

import (
	"context"
	"encoding/json"
)

// SetManager provides set data structure operations against the Synap server.
// A set is a collection of unique strings.
type SetManager struct {
	client *SynapClient
}

// Add inserts members into the set at key. Returns the number of members
// that were newly added (pre-existing members are ignored).
func (s *SetManager) Add(ctx context.Context, key string, members []string) (int, error) {
	type payload struct {
		Key     string   `json:"key"`
		Members []string `json:"members"`
	}
	raw, err := s.client.sendCommand(ctx, "set.add", payload{Key: key, Members: members})
	if err != nil {
		return 0, err
	}
	var result struct {
		Added int `json:"added"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return 0, newInvalidResponseError("set.add: " + err.Error())
	}
	return result.Added, nil
}

// Members returns all members of the set at key.
func (s *SetManager) Members(ctx context.Context, key string) ([]string, error) {
	type payload struct {
		Key string `json:"key"`
	}
	raw, err := s.client.sendCommand(ctx, "set.members", payload{Key: key})
	if err != nil {
		return nil, err
	}
	var result struct {
		Members []string `json:"members"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, newInvalidResponseError("set.members: " + err.Error())
	}
	return result.Members, nil
}

// IsMember reports whether member exists in the set at key.
func (s *SetManager) IsMember(ctx context.Context, key, member string) (bool, error) {
	type payload struct {
		Key    string `json:"key"`
		Member string `json:"member"`
	}
	raw, err := s.client.sendCommand(ctx, "set.ismember", payload{Key: key, Member: member})
	if err != nil {
		return false, err
	}
	var result struct {
		IsMember bool `json:"is_member"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return false, newInvalidResponseError("set.ismember: " + err.Error())
	}
	return result.IsMember, nil
}

// Remove deletes members from the set at key. Returns the number of members
// that were actually removed.
func (s *SetManager) Remove(ctx context.Context, key string, members []string) (int, error) {
	type payload struct {
		Key     string   `json:"key"`
		Members []string `json:"members"`
	}
	raw, err := s.client.sendCommand(ctx, "set.rem", payload{Key: key, Members: members})
	if err != nil {
		return 0, err
	}
	var result struct {
		Removed int `json:"removed"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return 0, newInvalidResponseError("set.rem: " + err.Error())
	}
	return result.Removed, nil
}

// Card returns the cardinality (number of elements) of the set at key.
func (s *SetManager) Card(ctx context.Context, key string) (int, error) {
	type payload struct {
		Key string `json:"key"`
	}
	raw, err := s.client.sendCommand(ctx, "set.card", payload{Key: key})
	if err != nil {
		return 0, err
	}
	var result struct {
		Cardinality int `json:"size"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return 0, newInvalidResponseError("set.card: " + err.Error())
	}
	return result.Cardinality, nil
}
