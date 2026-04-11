package synap

import (
	"context"
	"encoding/json"
	"time"
)

// KVStore provides key-value store operations against the Synap server.
type KVStore struct {
	client *SynapClient
}

// Set stores a key-value pair. ttl is the optional expiry duration; pass 0 for
// no expiry. The server expects a TTL expressed in seconds (uint64).
func (k *KVStore) Set(ctx context.Context, key, value string, ttl time.Duration) error {
	type payload struct {
		Key   string  `json:"key"`
		Value string  `json:"value"`
		TTL   *uint64 `json:"ttl"`
	}
	p := payload{Key: key, Value: value}
	if ttl > 0 {
		secs := uint64(ttl.Seconds())
		p.TTL = &secs
	}
	_, err := k.client.sendCommand(ctx, "kv.set", p)
	return err
}

// Get retrieves the value for key. Returns ("", nil) when the key does not
// exist (server returns a null payload).
func (k *KVStore) Get(ctx context.Context, key string) (string, error) {
	type payload struct {
		Key string `json:"key"`
	}
	raw, err := k.client.sendCommand(ctx, "kv.get", payload{Key: key})
	if err != nil {
		return "", err
	}
	// Server returns null when key is absent.
	if string(raw) == "null" {
		return "", nil
	}
	var value string
	if err := json.Unmarshal(raw, &value); err != nil {
		return "", newInvalidResponseError("kv.get: " + err.Error())
	}
	return value, nil
}

// Delete removes a key. Returns true if the key existed and was deleted.
func (k *KVStore) Delete(ctx context.Context, key string) (bool, error) {
	type payload struct {
		Key string `json:"key"`
	}
	raw, err := k.client.sendCommand(ctx, "kv.del", payload{Key: key})
	if err != nil {
		return false, err
	}
	var result struct {
		Deleted bool `json:"deleted"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return false, newInvalidResponseError("kv.del: " + err.Error())
	}
	return result.Deleted, nil
}

// Exists reports whether key is present in the store.
func (k *KVStore) Exists(ctx context.Context, key string) (bool, error) {
	type payload struct {
		Key string `json:"key"`
	}
	raw, err := k.client.sendCommand(ctx, "kv.exists", payload{Key: key})
	if err != nil {
		return false, err
	}
	var result struct {
		Exists bool `json:"exists"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return false, newInvalidResponseError("kv.exists: " + err.Error())
	}
	return result.Exists, nil
}

// Incr atomically increments the integer value stored at key by one and
// returns the new value.
func (k *KVStore) Incr(ctx context.Context, key string) (int64, error) {
	type payload struct {
		Key string `json:"key"`
	}
	raw, err := k.client.sendCommand(ctx, "kv.incr", payload{Key: key})
	if err != nil {
		return 0, err
	}
	var result struct {
		Value int64 `json:"value"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return 0, newInvalidResponseError("kv.incr: " + err.Error())
	}
	return result.Value, nil
}

// Decr atomically decrements the integer value stored at key by one and
// returns the new value.
func (k *KVStore) Decr(ctx context.Context, key string) (int64, error) {
	type payload struct {
		Key string `json:"key"`
	}
	raw, err := k.client.sendCommand(ctx, "kv.decr", payload{Key: key})
	if err != nil {
		return 0, err
	}
	var result struct {
		Value int64 `json:"value"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return 0, newInvalidResponseError("kv.decr: " + err.Error())
	}
	return result.Value, nil
}

// Stats returns aggregate statistics for the KV store.
func (k *KVStore) Stats(ctx context.Context) (KVStats, error) {
	raw, err := k.client.sendCommand(ctx, "kv.stats", struct{}{})
	if err != nil {
		return KVStats{}, err
	}
	var stats KVStats
	if err := json.Unmarshal(raw, &stats); err != nil {
		return KVStats{}, newInvalidResponseError("kv.stats: " + err.Error())
	}
	return stats, nil
}
