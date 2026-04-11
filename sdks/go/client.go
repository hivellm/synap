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
	"time"
)

// Config holds the configuration for a SynapClient.
type Config struct {
	baseURL   string
	authToken string
	username  string
	password  string
	timeout   time.Duration
}

// NewConfig creates a new Config targeting the given base URL.
// Example: synap.NewConfig("http://localhost:15500")
func NewConfig(baseURL string) *Config {
	return &Config{
		baseURL: baseURL,
		timeout: 30 * time.Second,
	}
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
}

// NewClient creates a new SynapClient using the provided Config.
func NewClient(cfg *Config) *SynapClient {
	return &SynapClient{
		config: cfg,
		httpClient: &http.Client{
			Timeout: cfg.timeout,
		},
		endpoint: cfg.baseURL + "/api/v1/command",
	}
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

// sendCommand sends a command to the Synap server and returns the raw payload
// bytes from the response envelope. The caller is responsible for unmarshalling
// the returned bytes into the expected type.
func (c *SynapClient) sendCommand(ctx context.Context, command string, payload interface{}) (json.RawMessage, error) {
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
