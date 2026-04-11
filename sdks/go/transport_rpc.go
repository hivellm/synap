package synap

import (
	"context"
	"encoding/binary"
	"fmt"
	"io"
	"net"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/vmihailenco/msgpack/v5"
)

// toSynapWireMap converts a Go value to serde's externally-tagged format:
//
//	nil     → "Null"
//	string  → {"Str": "value"}
//	int     → {"Int": 42}
//	bool    → {"Bool": true}
//	float64 → {"Float": 1.5}
//	[]byte  → {"Bytes": [1,2,3]}
func toSynapWireMap(v interface{}) interface{} {
	if v == nil {
		return "Null"
	}
	switch val := v.(type) {
	case string:
		return map[string]interface{}{"Str": val}
	case bool:
		return map[string]interface{}{"Bool": val}
	case int:
		return map[string]interface{}{"Int": int64(val)}
	case int64:
		return map[string]interface{}{"Int": val}
	case float64:
		return map[string]interface{}{"Float": val}
	case []byte:
		return map[string]interface{}{"Bytes": val}
	default:
		return map[string]interface{}{"Str": fmt.Sprintf("%v", val)}
	}
}

// unwrapSynapValue converts a serde externally-tagged SynapValue to a plain Go value.
func unwrapSynapValue(v interface{}) interface{} {
	if v == nil {
		return nil
	}
	if s, ok := v.(string); ok {
		if s == "Null" {
			return nil
		}
		return s
	}
	m, ok := v.(map[string]interface{})
	if !ok {
		return v
	}
	if val, has := m["Str"]; has {
		return val
	}
	if val, has := m["Int"]; has {
		return val
	}
	if val, has := m["Float"]; has {
		return val
	}
	if val, has := m["Bool"]; has {
		return val
	}
	if val, has := m["Bytes"]; has {
		if arr, ok := val.([]interface{}); ok {
			b := make([]byte, len(arr))
			for i, x := range arr {
				switch n := x.(type) {
				case int8:
					b[i] = byte(n)
				case uint8:
					b[i] = n
				case int64:
					b[i] = byte(n)
				case uint64:
					b[i] = byte(n)
				}
			}
			return string(b)
		}
		return val
	}
	if val, has := m["Array"]; has {
		if arr, ok := val.([]interface{}); ok {
			out := make([]interface{}, len(arr))
			for i, x := range arr {
				out[i] = unwrapSynapValue(x)
			}
			return out
		}
		return val
	}
	if val, has := m["Map"]; has {
		if pairs, ok := val.([]interface{}); ok {
			out := make(map[string]interface{})
			for _, p := range pairs {
				if pair, ok := p.([]interface{}); ok && len(pair) == 2 {
					k := unwrapSynapValue(pair[0])
					v := unwrapSynapValue(pair[1])
					out[fmt.Sprintf("%v", k)] = v
				}
			}
			return out
		}
		return val
	}
	return v
}

// ── SynapRPC transport ────────────────────────────────────────────────────────

// SynapRpcTransport is a persistent TCP connection to the SynapRPC listener.
// Synchronous request-response protected by a mutex. Auto-reconnects on failure.
type SynapRpcTransport struct {
	host    string
	port    int
	timeout time.Duration

	mu     sync.Mutex
	conn   net.Conn
	nextID uint32
}

func newSynapRpcTransport(host string, port int, timeout time.Duration) *SynapRpcTransport {
	return &SynapRpcTransport{host: host, port: port, timeout: timeout}
}

func (t *SynapRpcTransport) doConnect() error {
	addr := net.JoinHostPort(t.host, fmt.Sprintf("%d", t.port))
	conn, err := net.DialTimeout("tcp", addr, t.timeout)
	if err != nil {
		return fmt.Errorf("SynapRPC connect %s: %w", addr, err)
	}
	t.conn = conn
	return nil
}

// Execute sends a command and waits for the response. Thread-safe via mutex.
// Auto-reconnects once on failure.
func (t *SynapRpcTransport) Execute(ctx context.Context, cmd string, args []interface{}) (interface{}, error) {
	t.mu.Lock()
	defer t.mu.Unlock()

	for attempt := 0; attempt < 2; attempt++ {
		if t.conn == nil || attempt == 1 {
			if t.conn != nil {
				t.conn.Close()
			}
			t.conn = nil
			if err := t.doConnect(); err != nil {
				if attempt == 0 {
					continue
				}
				return nil, err
			}
		}

		id := atomic.AddUint32(&t.nextID, 1)

		wireArgs := make([]interface{}, len(args))
		for i, a := range args {
			wireArgs[i] = toSynapWireMap(a)
		}
		reqMap := map[string]interface{}{
			"id":      id,
			"command": strings.ToUpper(cmd),
			"args":    wireArgs,
		}
		body, err := msgpack.Marshal(reqMap)
		if err != nil {
			return nil, fmt.Errorf("SynapRPC marshal: %w", err)
		}

		deadline := time.Now().Add(t.timeout)
		if dl, ok := ctx.Deadline(); ok && dl.Before(deadline) {
			deadline = dl
		}
		_ = t.conn.SetDeadline(deadline)

		// Write length-prefixed frame
		frame := make([]byte, 4+len(body))
		binary.LittleEndian.PutUint32(frame[:4], uint32(len(body)))
		copy(frame[4:], body)
		if _, err := t.conn.Write(frame); err != nil {
			t.conn.Close()
			t.conn = nil
			if attempt == 0 {
				continue
			}
			return nil, fmt.Errorf("SynapRPC write: %w", err)
		}

		// Read response: 4-byte LE length header + body
		header := make([]byte, 4)
		if _, err := io.ReadFull(t.conn, header); err != nil {
			t.conn.Close()
			t.conn = nil
			if attempt == 0 {
				continue
			}
			return nil, fmt.Errorf("SynapRPC read header: %w", err)
		}
		respLen := binary.LittleEndian.Uint32(header)
		if respLen > 64*1024*1024 {
			t.conn.Close()
			t.conn = nil
			return nil, fmt.Errorf("SynapRPC frame too large: %d", respLen)
		}
		respBody := make([]byte, respLen)
		if _, err := io.ReadFull(t.conn, respBody); err != nil {
			t.conn.Close()
			t.conn = nil
			if attempt == 0 {
				continue
			}
			return nil, fmt.Errorf("SynapRPC read body: %w", err)
		}

		// Decode: response is array [id, {"Ok": value} | {"Err": string}]
		var raw interface{}
		if err := msgpack.Unmarshal(respBody, &raw); err != nil {
			return nil, fmt.Errorf("SynapRPC unmarshal: %w", err)
		}

		arr, ok := raw.([]interface{})
		if !ok || len(arr) != 2 {
			return nil, fmt.Errorf("SynapRPC: unexpected response format: %T", raw)
		}

		resultMap, ok := arr[1].(map[string]interface{})
		if !ok {
			return nil, fmt.Errorf("SynapRPC: result is not a map: %T", arr[1])
		}
		if okVal, has := resultMap["Ok"]; has {
			return unwrapSynapValue(okVal), nil
		}
		if errVal, has := resultMap["Err"]; has {
			return nil, newServerError(fmt.Sprintf("%v", errVal))
		}
		return nil, fmt.Errorf("SynapRPC: result has neither Ok nor Err")
	}
	return nil, fmt.Errorf("SynapRPC: exhausted reconnect attempts")
}

// Close tears down the underlying TCP connection.
func (t *SynapRpcTransport) Close() {
	t.mu.Lock()
	defer t.mu.Unlock()
	if t.conn != nil {
		t.conn.Close()
		t.conn = nil
	}
}
