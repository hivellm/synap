package synap

import (
	"context"
	"encoding/json"
)

// HashManager provides hash data structure operations against the Synap server.
// A hash is a field-value map, ideal for storing objects.
type HashManager struct {
	client *SynapClient
}

// Set stores field in the hash at key. Returns true if the field was newly
// created (false if it already existed and was updated).
func (h *HashManager) Set(ctx context.Context, key, field, value string) (bool, error) {
	type payload struct {
		Key   string `json:"key"`
		Field string `json:"field"`
		Value string `json:"value"`
	}
	raw, err := h.client.sendCommand(ctx, "hash.set", payload{Key: key, Field: field, Value: value})
	if err != nil {
		return false, err
	}
	var result struct {
		Success bool `json:"success"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return false, newInvalidResponseError("hash.set: " + err.Error())
	}
	return result.Success, nil
}

// Get retrieves the value of field in the hash at key.
// Returns ("", nil) when the field does not exist.
func (h *HashManager) Get(ctx context.Context, key, field string) (string, error) {
	type payload struct {
		Key   string `json:"key"`
		Field string `json:"field"`
	}
	raw, err := h.client.sendCommand(ctx, "hash.get", payload{Key: key, Field: field})
	if err != nil {
		return "", err
	}
	var result struct {
		Value *string `json:"value"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return "", newInvalidResponseError("hash.get: " + err.Error())
	}
	if result.Value == nil {
		return "", nil
	}
	return *result.Value, nil
}

// GetAll returns all field-value pairs in the hash at key.
func (h *HashManager) GetAll(ctx context.Context, key string) (map[string]string, error) {
	type payload struct {
		Key string `json:"key"`
	}
	raw, err := h.client.sendCommand(ctx, "hash.getall", payload{Key: key})
	if err != nil {
		return nil, err
	}
	var result struct {
		Fields map[string]string `json:"fields"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, newInvalidResponseError("hash.getall: " + err.Error())
	}
	if result.Fields == nil {
		return map[string]string{}, nil
	}
	return result.Fields, nil
}

// Del removes field from the hash at key. Returns the number of fields deleted.
func (h *HashManager) Del(ctx context.Context, key, field string) (int64, error) {
	type payload struct {
		Key    string   `json:"key"`
		Field  string   `json:"field"`
		Fields []string `json:"fields"`
	}
	raw, err := h.client.sendCommand(ctx, "hash.del", payload{Key: key, Field: field, Fields: []string{field}})
	if err != nil {
		return 0, err
	}
	var result struct {
		Deleted int64 `json:"deleted"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return 0, newInvalidResponseError("hash.del: " + err.Error())
	}
	return result.Deleted, nil
}

// Exists reports whether field exists in the hash at key.
func (h *HashManager) Exists(ctx context.Context, key, field string) (bool, error) {
	type payload struct {
		Key   string `json:"key"`
		Field string `json:"field"`
	}
	raw, err := h.client.sendCommand(ctx, "hash.exists", payload{Key: key, Field: field})
	if err != nil {
		return false, err
	}
	var result struct {
		Exists bool `json:"exists"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return false, newInvalidResponseError("hash.exists: " + err.Error())
	}
	return result.Exists, nil
}
