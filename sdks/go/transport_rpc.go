package synap

import (
	"context"
	"encoding/binary"
	"errors"
	"fmt"
	"io"
	"net"
	"strings"
	"sync"
	"sync/atomic"
	"time"
	"unicode/utf8"

	"github.com/vmihailenco/msgpack/v5"
)

// maxFrameBytes is the frame-body cap, matching the server's
// `protocol::synap_rpc::config::MAX_FRAME_BYTES`. It is validated against the
// length prefix before the body buffer is allocated, so a hostile peer cannot
// drive an unbounded allocation from four bytes.
//
// It was 64 MiB here while the server accepted 512 MiB, which meant this client
// rejected large frames the server considered legitimate.
const maxFrameBytes = 512 * 1024 * 1024

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
		// A Go string is an arbitrary byte sequence, not necessarily UTF-8,
		// and the SDK's KV surface is typed `string` -- so this is the only
		// way a caller can hand over binary. Sending it as MessagePack `str`
		// made the server decode it lossily: every invalid sequence came back
		// as U+FFFD, so `deadbeef` read back as `deadefbfbdefbfbd`. Bytes that
		// are not valid UTF-8 travel as `bin`, which round-trips exactly.
		if !utf8.ValidString(val) {
			return map[string]interface{}{"Bytes": []byte(val)}
		}
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
		// Two encodings reach us, and both must decode to the same string.
		//
		// Since 1.1.0 the server emits `Bytes` as MessagePack `bin`, which
		// decodes straight to []byte — ~33% smaller on the wire. Before that it
		// emitted an array of integers (Rust's rmp_serde Vec<u8> form), which a
		// pre-1.1.0 server still sends, so that path is kept indefinitely.
		switch b := val.(type) {
		case []byte:
			return string(b)
		case string:
			return b
		case []interface{}:
			out := make([]byte, len(b))
			for i, x := range b {
				switch n := x.(type) {
				case int8:
					out[i] = byte(n)
				case uint8:
					out[i] = n
				case int64:
					out[i] = byte(n)
				case uint64:
					out[i] = byte(n)
				}
			}
			return string(out)
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

	// Handshake credentials. This transport never sent AUTH, so it could not
	// reach a require_auth deployment on 15501 at all: every command came back
	// NOAUTH. The credentials the HTTP transport puts in an Authorization
	// header now open the connection here too.
	authToken string
	username  string
	password  string

	mu     sync.Mutex
	conn   net.Conn
	nextID uint32
}

func newSynapRpcTransport(host string, port int, timeout time.Duration) *SynapRpcTransport {
	return &SynapRpcTransport{host: host, port: port, timeout: timeout}
}

// withCredentials attaches handshake credentials, sent as AUTH on every
// connect -- including the reconnects Execute performs, so an authenticated
// session is not silently downgraded when the socket is replaced.
func (t *SynapRpcTransport) withCredentials(token, username, password string) *SynapRpcTransport {
	t.authToken = token
	t.username = username
	t.password = password
	return t
}

func (t *SynapRpcTransport) doConnect() error {
	addr := net.JoinHostPort(t.host, fmt.Sprintf("%d", t.port))
	conn, err := net.DialTimeout("tcp", addr, t.timeout)
	if err != nil {
		return fmt.Errorf("SynapRPC connect %s: %w", addr, err)
	}
	t.conn = conn

	if err := t.authenticate(); err != nil {
		conn.Close()
		t.conn = nil
		return err
	}
	return nil
}

// authenticate sends AUTH on a freshly dialled connection.
//
// `AUTH <password>` authenticates the default user and `AUTH <user> <pass>`
// names one, matching the server's handshake. With no credentials configured
// the connection stays anonymous, which is what an open deployment expects.
func (t *SynapRpcTransport) authenticate() error {
	var args []interface{}
	switch {
	case t.username != "" && t.password != "":
		args = []interface{}{t.username, t.password}
	case t.authToken != "":
		args = []interface{}{t.authToken}
	default:
		return nil
	}

	if _, err := t.exchange(context.Background(), "AUTH", args); err != nil {
		return fmt.Errorf("SynapRPC authenticate: %w", err)
	}
	return nil
}

// connError marks a failure that killed the connection, so Execute knows a
// reconnect is worth attempting. A server-side `Err` reply is not one of
// these: the connection is fine and the command genuinely failed.
type connError struct{ err error }

func (e *connError) Error() string { return e.err.Error() }
func (e *connError) Unwrap() error { return e.err }

// Execute sends a command and waits for the response. Thread-safe via mutex.
// Auto-reconnects once on failure.
func (t *SynapRpcTransport) Execute(ctx context.Context, cmd string, args []interface{}) (interface{}, error) {
	t.mu.Lock()
	defer t.mu.Unlock()

	for attempt := 0; attempt < 2; attempt++ {
		if t.conn == nil {
			if err := t.doConnect(); err != nil {
				if attempt == 0 {
					continue
				}
				return nil, err
			}
		}

		result, err := t.exchange(ctx, cmd, args)
		if err == nil {
			return result, nil
		}

		var dead *connError
		if !errors.As(err, &dead) {
			return nil, err
		}

		// The socket is gone: drop it so the next attempt dials -- and
		// re-authenticates -- from scratch.
		if t.conn != nil {
			t.conn.Close()
			t.conn = nil
		}
		if attempt == 1 {
			return nil, err
		}
	}
	return nil, fmt.Errorf("SynapRPC: exhausted reconnect attempts")
}

// exchange performs exactly one request/response round trip on the live
// connection. Split out of Execute so the AUTH handshake can use the same
// framing without re-entering Execute's mutex or its reconnect loop.
func (t *SynapRpcTransport) exchange(ctx context.Context, cmd string, args []interface{}) (interface{}, error) {
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
		return nil, &connError{fmt.Errorf("SynapRPC write: %w", err)}
	}

	// Read response: 4-byte LE length header + body
	header := make([]byte, 4)
	if _, err := io.ReadFull(t.conn, header); err != nil {
		return nil, &connError{fmt.Errorf("SynapRPC read header: %w", err)}
	}
	respLen := binary.LittleEndian.Uint32(header)
	if respLen > maxFrameBytes {
		return nil, &connError{fmt.Errorf("SynapRPC frame too large: %d", respLen)}
	}
	respBody := make([]byte, respLen)
	if _, err := io.ReadFull(t.conn, respBody); err != nil {
		return nil, &connError{fmt.Errorf("SynapRPC read body: %w", err)}
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

// Close tears down the underlying TCP connection.
func (t *SynapRpcTransport) Close() {
	t.mu.Lock()
	defer t.mu.Unlock()
	if t.conn != nil {
		t.conn.Close()
		t.conn = nil
	}
}
