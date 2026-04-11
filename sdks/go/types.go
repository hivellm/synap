package synap

// Message represents a queue message returned by the server.
type Message struct {
	ID         string  `json:"id"`
	Payload    []byte  `json:"payload"`
	Priority   uint8   `json:"priority"`
	RetryCount uint32  `json:"retry_count"`
	MaxRetries uint32  `json:"max_retries"`
	Deadline   *uint64 `json:"deadline,omitempty"`
}

// QueueStats holds statistics for a single queue.
type QueueStats struct {
	Depth        int    `json:"depth"`
	Consumers    int    `json:"consumers"`
	Published    uint64 `json:"published"`
	Consumed     uint64 `json:"consumed"`
	Acked        uint64 `json:"acked"`
	Nacked       uint64 `json:"nacked"`
	DeadLettered int    `json:"dead_lettered"`
}

// Event represents a single event in a stream room.
type Event struct {
	Offset    uint64      `json:"offset"`
	Event     string      `json:"event"`
	Data      interface{} `json:"data"`
	Timestamp *uint64     `json:"timestamp,omitempty"`
}

// StreamStats holds statistics for a stream room.
type StreamStats struct {
	Name           string `json:"name"`
	MessageCount   int    `json:"message_count"`
	MaxOffset      uint64 `json:"max_offset"`
	TotalPublished uint64 `json:"total_published"`
	TotalConsumed  uint64 `json:"total_consumed"`
	SubscriberCount int   `json:"subscriber_count"`
}

// KVStats holds statistics for the KV store.
type KVStats struct {
	TotalKeys        int     `json:"total_keys"`
	TotalMemoryBytes int     `json:"total_memory_bytes"`
	HitRate          float64 `json:"hit_rate"`
}
