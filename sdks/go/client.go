package synap

import (
	"bytes"
	"context"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"
)

// TransportMode selects the wire protocol.
type TransportMode int

const (
	TransportHTTP     TransportMode = iota // JSON over HTTP REST
	TransportSynapRPC                      // MessagePack over TCP (default)
	TransportRESP3                         // Redis RESP3 over TCP
)

// Config holds the configuration for a SynapClient.
type Config struct {
	baseURL   string
	host      string
	port      int
	transport TransportMode
	authToken string
	username  string
	password  string
	timeout   time.Duration
}

// NewConfig creates a new Config targeting the given URL.
// Transport is auto-detected from the scheme:
//   - synap://host:port → SynapRPC (default)
//   - resp3://host:port → RESP3
//   - http://host:port  → HTTP
func NewConfig(url string) *Config {
	cfg := &Config{
		baseURL: url,
		timeout: 30 * time.Second,
	}
	if strings.HasPrefix(url, "synap://") {
		cfg.transport = TransportSynapRPC
		rest := strings.TrimPrefix(url, "synap://")
		cfg.host, cfg.port = parseHostPort(rest, 15501)
	} else if strings.HasPrefix(url, "resp3://") {
		cfg.transport = TransportRESP3
		rest := strings.TrimPrefix(url, "resp3://")
		cfg.host, cfg.port = parseHostPort(rest, 6379)
	} else {
		cfg.transport = TransportHTTP
	}
	return cfg
}

func parseHostPort(authority string, defaultPort int) (string, int) {
	// Strip path
	if idx := strings.Index(authority, "/"); idx >= 0 {
		authority = authority[:idx]
	}
	if idx := strings.LastIndex(authority, ":"); idx >= 0 {
		host := authority[:idx]
		port := 0
		fmt.Sscanf(authority[idx+1:], "%d", &port)
		if port == 0 {
			port = defaultPort
		}
		return host, port
	}
	return authority, defaultPort
}

// WithAuth sets a Bearer token for authentication.
// Calling this clears any previously set basic-auth credentials.
func (c *Config) WithAuth(token string) *Config {
	c.authToken = token
	c.username = ""
	c.password = ""
	return c
}

// WithBasicAuth sets HTTP Basic Auth credentials.
// Calling this clears any previously set Bearer token.
func (c *Config) WithBasicAuth(username, password string) *Config {
	c.username = username
	c.password = password
	c.authToken = ""
	return c
}

// WithTimeout sets the HTTP request timeout. Defaults to 30 seconds.
func (c *Config) WithTimeout(d time.Duration) *Config {
	c.timeout = d
	return c
}

// commandEnvelope is the JSON envelope sent to POST /api/v1/command.
type commandEnvelope struct {
	Command   string          `json:"command"`
	RequestID string          `json:"request_id"`
	Payload   json.RawMessage `json:"payload"`
}

// responseEnvelope is the JSON envelope received from the server.
type responseEnvelope struct {
	Success   bool            `json:"success"`
	RequestID string          `json:"request_id"`
	Payload   json.RawMessage `json:"payload"`
	Error     *string         `json:"error"`
}

// SynapClient is the main entry point for communicating with a Synap server.
// It is safe to use concurrently from multiple goroutines.
type SynapClient struct {
	config     *Config
	httpClient *http.Client
	endpoint   string
	rpc        *SynapRpcTransport
}

// NewClient creates a new SynapClient using the provided Config.
func NewClient(cfg *Config) *SynapClient {
	c := &SynapClient{
		config: cfg,
		httpClient: &http.Client{
			Timeout: cfg.timeout,
		},
	}
	switch cfg.transport {
	case TransportSynapRPC:
		c.rpc = newSynapRpcTransport(cfg.host, cfg.port, cfg.timeout)
		c.endpoint = fmt.Sprintf("http://%s:15500/api/v1/command", cfg.host)
	case TransportRESP3:
		// RESP3 falls back to HTTP for now; the resp3 transport file
		// will be integrated in a follow-up when the full RESP3 parser
		// is battle-tested against the server.
		c.endpoint = fmt.Sprintf("http://%s:15500/api/v1/command", cfg.host)
	default:
		c.endpoint = cfg.baseURL + "/api/v1/command"
	}
	return c
}

// KV returns a KVStore interface for key-value operations.
func (c *SynapClient) KV() *KVStore { return &KVStore{client: c} }

// Queue returns a QueueManager interface for queue operations.
func (c *SynapClient) Queue() *QueueManager { return &QueueManager{client: c} }

// Stream returns a StreamManager interface for stream operations.
func (c *SynapClient) Stream() *StreamManager { return &StreamManager{client: c} }

// PubSub returns a PubSubManager interface for pub/sub operations.
func (c *SynapClient) PubSub() *PubSubManager { return &PubSubManager{client: c} }

// Hash returns a HashManager interface for hash operations.
func (c *SynapClient) Hash() *HashManager { return &HashManager{client: c} }

// List returns a ListManager interface for list operations.
func (c *SynapClient) List() *ListManager { return &ListManager{client: c} }

// Set returns a SetManager interface for set operations.
func (c *SynapClient) Set() *SetManager { return &SetManager{client: c} }

// sendCommand dispatches to the active transport (SynapRPC, RESP3, or HTTP)
// and returns the raw payload bytes. The caller unmarshals into the expected type.
func (c *SynapClient) sendCommand(ctx context.Context, command string, payload interface{}) (json.RawMessage, error) {
	// For SynapRPC transport, use the binary protocol
	if c.rpc != nil {
		return c.sendRPC(ctx, command, payload)
	}
	return c.sendHTTP(ctx, command, payload)
}

// sendRPC dispatches a command via the SynapRPC binary transport.
func (c *SynapClient) sendRPC(ctx context.Context, command string, payload interface{}) (json.RawMessage, error) {
	// Marshal payload to JSON first, then extract fields for wire args
	payloadBytes, err := json.Marshal(payload)
	if err != nil {
		return nil, fmt.Errorf("synap: marshal payload: %w", err)
	}
	var payloadMap map[string]interface{}
	_ = json.Unmarshal(payloadBytes, &payloadMap)

	// Map SDK command to native wire command + args
	wireCmd, wireArgs, err := mapCommandToWire(command, payloadMap)
	if err != nil {
		return nil, err
	}

	// Execute via RPC transport
	result, err := c.rpc.Execute(ctx, wireCmd, wireArgs)
	if err != nil {
		return nil, err
	}

	// Map the native response back to the JSON shape the module methods expect
	mapped := mapResponseFromWire(command, result)
	respBytes, err := json.Marshal(mapped)
	if err != nil {
		return nil, fmt.Errorf("synap: marshal rpc response: %w", err)
	}
	return json.RawMessage(respBytes), nil
}

// sendHTTP dispatches a command via the HTTP REST transport.
func (c *SynapClient) sendHTTP(ctx context.Context, command string, payload interface{}) (json.RawMessage, error) {
	payloadBytes, err := json.Marshal(payload)
	if err != nil {
		return nil, fmt.Errorf("synap: marshal payload: %w", err)
	}

	reqID, err := newRequestID()
	if err != nil {
		return nil, fmt.Errorf("synap: generate request_id: %w", err)
	}

	env := commandEnvelope{
		Command:   command,
		RequestID: reqID,
		Payload:   json.RawMessage(payloadBytes),
	}

	body, err := json.Marshal(env)
	if err != nil {
		return nil, fmt.Errorf("synap: marshal envelope: %w", err)
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, c.endpoint, bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("synap: build request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "application/json")

	c.applyAuth(req)

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("synap: http: %w", err)
	}
	defer resp.Body.Close()

	respBytes, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("synap: read response body: %w", err)
	}

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, newServerError(fmt.Sprintf("HTTP %d: %s", resp.StatusCode, string(respBytes)))
	}

	var envelope responseEnvelope
	if err := json.Unmarshal(respBytes, &envelope); err != nil {
		return nil, fmt.Errorf("synap: unmarshal response: %w", err)
	}

	if !envelope.Success {
		msg := "unknown error"
		if envelope.Error != nil {
			msg = *envelope.Error
		}
		return nil, newServerError(msg)
	}

	return envelope.Payload, nil
}

// mapCommandToWire converts an SDK command + JSON payload to native wire command + args.
func mapCommandToWire(command string, payload map[string]interface{}) (string, []interface{}, error) {
	getStr := func(key string) string {
		if v, ok := payload[key]; ok {
			if s, ok := v.(string); ok {
				return s
			}
			return fmt.Sprintf("%v", v)
		}
		return ""
	}
	getFloat := func(key string) float64 {
		if v, ok := payload[key]; ok {
			switch n := v.(type) {
			case float64:
				return n
			case int:
				return float64(n)
			}
		}
		return 0
	}
	getStrSlice := func(key string) []string {
		if v, ok := payload[key]; ok {
			if arr, ok := v.([]interface{}); ok {
				result := make([]string, len(arr))
				for i, item := range arr {
					result[i] = fmt.Sprintf("%v", item)
				}
				return result
			}
		}
		return nil
	}

	switch command {
	// KV
	case "kv.set":
		args := []interface{}{getStr("key"), getStr("value")}
		if ttl := getFloat("ttl"); ttl > 0 {
			args = append(args, "EX", int(ttl))
		}
		return "SET", args, nil
	case "kv.get":
		return "GET", []interface{}{getStr("key")}, nil
	case "kv.del":
		return "DEL", []interface{}{getStr("key")}, nil
	case "kv.exists":
		return "EXISTS", []interface{}{getStr("key")}, nil
	case "kv.incr":
		amount := getFloat("amount")
		if amount > 1 {
			return "INCRBY", []interface{}{getStr("key"), int(amount)}, nil
		}
		return "INCR", []interface{}{getStr("key")}, nil
	case "kv.decr":
		amount := getFloat("amount")
		if amount > 1 {
			return "DECRBY", []interface{}{getStr("key"), int(amount)}, nil
		}
		return "DECR", []interface{}{getStr("key")}, nil

	// Hash
	case "hash.set":
		return "HSET", []interface{}{getStr("key"), getStr("field"), getStr("value")}, nil
	case "hash.get":
		return "HGET", []interface{}{getStr("key"), getStr("field")}, nil
	case "hash.getall":
		return "HGETALL", []interface{}{getStr("key")}, nil
	case "hash.del":
		args := []interface{}{getStr("key")}
		for _, f := range getStrSlice("fields") {
			args = append(args, f)
		}
		if len(args) == 1 {
			args = append(args, getStr("field"))
		}
		return "HDEL", args, nil
	case "hash.exists":
		return "HEXISTS", []interface{}{getStr("key"), getStr("field")}, nil

	// List
	case "list.lpush":
		args := []interface{}{getStr("key")}
		for _, v := range getStrSlice("values") {
			args = append(args, v)
		}
		return "LPUSH", args, nil
	case "list.rpush":
		args := []interface{}{getStr("key")}
		for _, v := range getStrSlice("values") {
			args = append(args, v)
		}
		return "RPUSH", args, nil
	case "list.lpop":
		return "LPOP", []interface{}{getStr("key"), int(getFloat("count"))}, nil
	case "list.rpop":
		return "RPOP", []interface{}{getStr("key"), int(getFloat("count"))}, nil
	case "list.range":
		return "LRANGE", []interface{}{getStr("key"), int(getFloat("start")), int(getFloat("stop"))}, nil
	case "list.len":
		return "LLEN", []interface{}{getStr("key")}, nil

	// Set
	case "set.add":
		args := []interface{}{getStr("key")}
		for _, m := range getStrSlice("members") {
			args = append(args, m)
		}
		return "SADD", args, nil
	case "set.members":
		return "SMEMBERS", []interface{}{getStr("key")}, nil
	case "set.ismember":
		return "SISMEMBER", []interface{}{getStr("key"), getStr("member")}, nil
	case "set.rem":
		args := []interface{}{getStr("key")}
		for _, m := range getStrSlice("members") {
			args = append(args, m)
		}
		return "SREM", args, nil
	case "set.card":
		return "SCARD", []interface{}{getStr("key")}, nil

	// Queue
	case "queue.create":
		return "QCREATE", []interface{}{getStr("name")}, nil
	case "queue.delete":
		return "QDELETE", []interface{}{getStr("queue")}, nil
	case "queue.publish":
		args := []interface{}{getStr("queue")}
		if p, ok := payload["payload"]; ok {
			if barr, ok := p.([]interface{}); ok {
				b := make([]byte, len(barr))
				for i, v := range barr {
					if f, ok := v.(float64); ok {
						b[i] = byte(f)
					}
				}
				args = append(args, string(b))
			} else {
				args = append(args, fmt.Sprintf("%v", p))
			}
		}
		return "QPUBLISH", args, nil
	case "queue.consume":
		return "QCONSUME", []interface{}{getStr("queue"), getStr("consumer_id")}, nil
	case "queue.ack":
		return "QACK", []interface{}{getStr("queue"), getStr("message_id")}, nil
	case "queue.nack":
		return "QNACK", []interface{}{getStr("queue"), getStr("message_id")}, nil
	case "queue.list":
		return "QLIST", []interface{}{}, nil
	case "queue.stats":
		return "QSTATS", []interface{}{getStr("queue")}, nil

	// Stream
	case "stream.create":
		return "SCREATE", []interface{}{getStr("room")}, nil
	case "stream.publish":
		return "SPUBLISH", []interface{}{getStr("room"), getStr("event"), getStr("data")}, nil
	case "stream.consume":
		return "SCONSUME", []interface{}{getStr("room"), getStr("subscriber_id"), int(getFloat("from_offset")), int(getFloat("limit"))}, nil
	case "stream.list":
		return "SLIST", []interface{}{}, nil
	case "stream.stats":
		return "SSTATS", []interface{}{getStr("room")}, nil
	case "stream.delete":
		return "SDELETE", []interface{}{getStr("room")}, nil

	// PubSub
	case "pubsub.publish":
		return "PUBLISH", []interface{}{getStr("topic"), getStr("payload")}, nil
	case "pubsub.subscribe":
		args := []interface{}{}
		for _, t := range getStrSlice("topics") {
			args = append(args, t)
		}
		return "SUBSCRIBE", args, nil
	case "pubsub.unsubscribe":
		args := []interface{}{getStr("subscriber_id")}
		for _, t := range getStrSlice("topics") {
			args = append(args, t)
		}
		return "UNSUBSCRIBE", args, nil
	case "pubsub.topics":
		return "TOPICS", []interface{}{}, nil

	// Utility
	case "kv.dbsize":
		return "DBSIZE", []interface{}{}, nil
	case "kv.flushdb":
		return "FLUSHDB", []interface{}{}, nil
	case "kv.flushall":
		return "FLUSHALL", []interface{}{}, nil
	case "kv.persist":
		return "PERSIST", []interface{}{getStr("key")}, nil
	case "kv.scan":
		prefix := getStr("prefix")
		if prefix == "" {
			return "KEYS", []interface{}{"*"}, nil
		}
		return "KEYS", []interface{}{prefix + "*"}, nil
	case "kv.mdel":
		args := []interface{}{}
		for _, k := range getStrSlice("keys") {
			args = append(args, k)
		}
		return "DEL", args, nil
	case "kv.mset":
		args := []interface{}{}
		if pairs, ok := payload["pairs"].([]interface{}); ok {
			for _, p := range pairs {
				if pm, ok := p.(map[string]interface{}); ok {
					args = append(args, fmt.Sprintf("%v", pm["key"]), fmt.Sprintf("%v", pm["value"]))
				}
			}
		}
		return "MSET", args, nil
	case "kv.mget":
		args := []interface{}{}
		for _, k := range getStrSlice("keys") {
			args = append(args, k)
		}
		return "MGET", args, nil

	default:
		return "", nil, &SynapError{Message: fmt.Sprintf("command '%s' is not supported on transport 'SynapRpc'", command), Code: "UnsupportedCommand"}
	}
}

// mapResponseFromWire converts a native wire response to the JSON shape SDK modules expect.
func mapResponseFromWire(command string, result interface{}) interface{} {
	switch command {
	case "kv.set":
		return map[string]interface{}{"success": true}
	case "kv.get":
		if result == nil {
			return nil
		}
		return result
	case "kv.del":
		deleted := false
		if n, ok := result.(int64); ok {
			deleted = n > 0
		}
		return map[string]interface{}{"deleted": deleted}
	case "kv.exists":
		exists := false
		if n, ok := result.(int64); ok {
			exists = n > 0
		}
		return map[string]interface{}{"exists": exists}
	case "kv.incr", "kv.decr":
		return map[string]interface{}{"value": result}
	case "hash.set":
		created := false
		if n, ok := result.(int64); ok {
			created = n > 0
		}
		return map[string]interface{}{"created": created}
	case "hash.get":
		if result == nil {
			return map[string]interface{}{"found": false}
		}
		return map[string]interface{}{"found": true, "value": result}
	case "hash.exists":
		exists := false
		if n, ok := result.(int64); ok {
			exists = n > 0
		}
		return map[string]interface{}{"exists": exists}
	case "hash.del":
		return map[string]interface{}{"removed": result}
	case "hash.getall":
		// RPC returns a flat array [k, v, k, v, ...] or a map
		if arr, ok := result.([]interface{}); ok {
			fields := map[string]interface{}{}
			for i := 0; i+1 < len(arr); i += 2 {
				fields[fmt.Sprintf("%v", arr[i])] = arr[i+1]
			}
			return map[string]interface{}{"fields": fields}
		}
		return map[string]interface{}{"fields": result}
	case "list.lpush", "list.rpush":
		return map[string]interface{}{"len": result}
	case "list.lpop", "list.rpop":
		if arr, ok := result.([]interface{}); ok {
			return map[string]interface{}{"values": arr}
		}
		if result == nil {
			return map[string]interface{}{"values": []interface{}{}}
		}
		return map[string]interface{}{"values": []interface{}{result}}
	case "list.range":
		if arr, ok := result.([]interface{}); ok {
			return map[string]interface{}{"values": arr}
		}
		return map[string]interface{}{"values": []interface{}{}}
	case "list.len":
		return map[string]interface{}{"len": result}
	case "set.add":
		return map[string]interface{}{"added": result}
	case "set.members":
		if arr, ok := result.([]interface{}); ok {
			return map[string]interface{}{"members": arr}
		}
		return map[string]interface{}{"members": []interface{}{}}
	case "set.ismember":
		isMember := false
		if n, ok := result.(int64); ok {
			isMember = n > 0
		}
		return map[string]interface{}{"is_member": isMember}
	case "set.rem":
		return map[string]interface{}{"removed": result}
	case "set.card":
		return map[string]interface{}{"count": result}
	case "queue.create", "queue.delete", "queue.ack", "queue.nack":
		return map[string]interface{}{}
	case "queue.publish":
		return map[string]interface{}{"message_id": result}
	case "queue.consume":
		if result == nil {
			return map[string]interface{}{"message": nil}
		}
		return map[string]interface{}{"message": result}
	case "queue.list":
		if arr, ok := result.([]interface{}); ok {
			return map[string]interface{}{"queues": arr}
		}
		return map[string]interface{}{"queues": []interface{}{}}
	case "queue.stats":
		return result
	default:
		return result
	}
}

// applyAuth adds the configured authentication headers to r.
func (c *SynapClient) applyAuth(r *http.Request) {
	cfg := c.config
	switch {
	case cfg.authToken != "":
		r.Header.Set("Authorization", "Bearer "+cfg.authToken)
	case cfg.username != "":
		encoded := base64.StdEncoding.EncodeToString([]byte(cfg.username + ":" + cfg.password))
		r.Header.Set("Authorization", "Basic "+encoded)
	}
}

// newRequestID generates a random UUID v4 string using crypto/rand.
func newRequestID() (string, error) {
	var b [16]byte
	if _, err := rand.Read(b[:]); err != nil {
		return "", err
	}
	// Set version (4) and variant bits per RFC 4122.
	b[6] = (b[6] & 0x0f) | 0x40
	b[8] = (b[8] & 0x3f) | 0x80
	return fmt.Sprintf("%08x-%04x-%04x-%04x-%012x",
		b[0:4], b[4:6], b[6:8], b[8:10], b[10:16]), nil
}
