package synap

import (
	"bufio"
	"context"
	"fmt"
	"net"
	"strconv"
	"strings"
	"sync"
	"time"
)

// Resp3Transport is a persistent TCP connection to a RESP3-compatible listener.
// It speaks RESP2 multibulk on the wire (compatible with Redis and Synap's RESP3
// listener). Synchronous request-response is protected by a mutex. Auto-reconnects
// on failure (up to 2 attempts). Connection is established lazily on first use.
type Resp3Transport struct {
	host    string
	port    int
	timeout time.Duration
	mu      sync.Mutex
	conn    net.Conn
	reader  *bufio.Reader
}

// newResp3Transport creates a new Resp3Transport targeting host:port.
// The connection is established lazily on the first Execute call.
func newResp3Transport(host string, port int, timeout time.Duration) *Resp3Transport {
	return &Resp3Transport{host: host, port: port, timeout: timeout}
}

func (t *Resp3Transport) doConnect() error {
	addr := net.JoinHostPort(t.host, strconv.Itoa(t.port))
	conn, err := net.DialTimeout("tcp", addr, t.timeout)
	if err != nil {
		return fmt.Errorf("RESP3 connect %s: %w", addr, err)
	}
	t.conn = conn
	t.reader = bufio.NewReader(conn)
	return nil
}

// buildMultibulk encodes cmd and args as a RESP2 multibulk array frame.
// Each element is serialised to its string representation and framed as a
// bulk string ($len\r\ndata\r\n).
func buildMultibulk(cmd string, args []interface{}) []byte {
	parts := make([]string, 0, len(args)+1)
	parts = append(parts, cmd)
	for _, a := range args {
		parts = append(parts, fmt.Sprintf("%v", a))
	}

	var b strings.Builder
	b.WriteString(fmt.Sprintf("*%d\r\n", len(parts)))
	for _, p := range parts {
		b.WriteString(fmt.Sprintf("$%d\r\n%s\r\n", len(p), p))
	}
	return []byte(b.String())
}

// readResponse reads and parses one RESP value from the buffered reader.
// Supported types:
//
//	'+' → simple string
//	'-' → error (returned as a Go error)
//	':' → integer (int64)
//	'$' → bulk string (string or nil on $-1)
//	'*' → array ([]interface{} or nil on *-1)
//	'_' → null (RESP3 extension)
func readResponse(r *bufio.Reader) (interface{}, error) {
	line, err := readLine(r)
	if err != nil {
		return nil, err
	}
	if len(line) == 0 {
		return nil, fmt.Errorf("RESP3: empty line")
	}

	prefix := line[0]
	payload := line[1:]

	switch prefix {
	case '+':
		// Simple string
		return payload, nil

	case '-':
		// Server error
		return nil, newServerError(payload)

	case ':':
		// Integer
		n, err := strconv.ParseInt(payload, 10, 64)
		if err != nil {
			return nil, fmt.Errorf("RESP3: invalid integer %q: %w", payload, err)
		}
		return n, nil

	case '$':
		// Bulk string
		length, err := strconv.Atoi(payload)
		if err != nil {
			return nil, fmt.Errorf("RESP3: invalid bulk length %q: %w", payload, err)
		}
		if length == -1 {
			return nil, nil
		}
		// Read exactly length bytes + CRLF
		data := make([]byte, length+2)
		if _, err := readFull(r, data); err != nil {
			return nil, fmt.Errorf("RESP3: read bulk data: %w", err)
		}
		return string(data[:length]), nil

	case '*':
		// Array
		count, err := strconv.Atoi(payload)
		if err != nil {
			return nil, fmt.Errorf("RESP3: invalid array length %q: %w", payload, err)
		}
		if count == -1 {
			return nil, nil
		}
		arr := make([]interface{}, count)
		for i := 0; i < count; i++ {
			elem, err := readResponse(r)
			if err != nil {
				return nil, err
			}
			arr[i] = elem
		}
		return arr, nil

	case '_':
		// RESP3 null
		return nil, nil

	default:
		return nil, fmt.Errorf("RESP3: unknown type byte %q in line %q", string(prefix), line)
	}
}

// readLine reads bytes up to and including \r\n, returning the line without
// the trailing \r\n.
func readLine(r *bufio.Reader) (string, error) {
	line, err := r.ReadString('\n')
	if err != nil {
		return "", fmt.Errorf("RESP3: read line: %w", err)
	}
	// Strip \r\n
	line = strings.TrimRight(line, "\r\n")
	return line, nil
}

// readFull reads exactly len(buf) bytes from r.
func readFull(r *bufio.Reader, buf []byte) (int, error) {
	total := 0
	for total < len(buf) {
		n, err := r.Read(buf[total:])
		total += n
		if err != nil {
			return total, err
		}
	}
	return total, nil
}

// Execute sends a RESP2 multibulk command and returns the parsed response.
// It is thread-safe via an internal mutex. On network failure it reconnects
// once and retries automatically.
//
// Returned Go types:
//   - string for simple strings and bulk strings
//   - int64 for integer replies
//   - nil for null replies
//   - []interface{} for array replies
//   - error for server-side errors (type *SynapError)
func (t *Resp3Transport) Execute(ctx context.Context, cmd string, args []interface{}) (interface{}, error) {
	t.mu.Lock()
	defer t.mu.Unlock()

	for attempt := 0; attempt < 2; attempt++ {
		if t.conn == nil || attempt == 1 {
			if t.conn != nil {
				t.conn.Close()
				t.conn = nil
				t.reader = nil
			}
			if err := t.doConnect(); err != nil {
				if attempt == 0 {
					continue
				}
				return nil, err
			}
		}

		deadline := time.Now().Add(t.timeout)
		if dl, ok := ctx.Deadline(); ok && dl.Before(deadline) {
			deadline = dl
		}
		_ = t.conn.SetDeadline(deadline)

		frame := buildMultibulk(cmd, args)
		if _, err := t.conn.Write(frame); err != nil {
			t.conn.Close()
			t.conn = nil
			t.reader = nil
			if attempt == 0 {
				continue
			}
			return nil, fmt.Errorf("RESP3 write: %w", err)
		}

		result, err := readResponse(t.reader)
		if err != nil {
			// If this is a server-error (e.g. wrong type), do NOT reconnect —
			// the connection is still healthy.
			if isSynapError(err) {
				return nil, err
			}
			t.conn.Close()
			t.conn = nil
			t.reader = nil
			if attempt == 0 {
				continue
			}
			return nil, fmt.Errorf("RESP3 read: %w", err)
		}

		return result, nil
	}
	return nil, fmt.Errorf("RESP3: exhausted reconnect attempts")
}

// Close tears down the underlying TCP connection.
func (t *Resp3Transport) Close() {
	t.mu.Lock()
	defer t.mu.Unlock()
	if t.conn != nil {
		t.conn.Close()
		t.conn = nil
		t.reader = nil
	}
}

// isSynapError reports whether err is a *SynapError (server-side error, not a
// network/IO error). We inspect the error without importing any extra packages.
func isSynapError(err error) bool {
	_, ok := err.(*SynapError)
	return ok
}
