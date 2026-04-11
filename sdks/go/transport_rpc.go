package synap

import (
	"context"
	"encoding/binary"
	"fmt"
	"net"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/vmihailenco/msgpack/v5"
)

// SynapValue is the tagged-union wire value used by SynapRPC.
// It mirrors Rust's rmp_serde externally-tagged enum format.
//
// Encoding rules:
//   - Null      → bare msgpack string "Null"
//   - Str(v)    → single-key map {"Str": v}
//   - Int(v)    → single-key map {"Int": v}
//   - Float(v)  → single-key map {"Float": v}
//   - Bool(v)   → single-key map {"Bool": v}
//   - Bytes(v)  → single-key map {"Bytes": [byte...]}
//   - Array(v)  → single-key map {"Array": [...]}
//   - Map(v)    → single-key map {"Map": [[k,v],...]}
type SynapValue struct {
	tag   string
	inner interface{}
}

// synapNull is the singleton Null value.
var synapNull = SynapValue{tag: "Null"}

func synapStr(s string) SynapValue   { return SynapValue{tag: "Str", inner: s} }
func synapInt(i int64) SynapValue    { return SynapValue{tag: "Int", inner: i} }
func synapFloat(f float64) SynapValue { return SynapValue{tag: "Float", inner: f} }
func synapBool(b bool) SynapValue    { return SynapValue{tag: "Bool", inner: b} }
func synapBytes(b []byte) SynapValue { return SynapValue{tag: "Bytes", inner: b} }

// toWireValue converts a plain Go value into a SynapValue for wire encoding.
func toWireValue(v interface{}) SynapValue {
	if v == nil {
		return synapNull
	}
	switch val := v.(type) {
	case string:
		return synapStr(val)
	case bool:
		return synapBool(val)
	case int:
		return synapInt(int64(val))
	case int8:
		return synapInt(int64(val))
	case int16:
		return synapInt(int64(val))
	case int32:
		return synapInt(int64(val))
	case int64:
		return synapInt(val)
	case uint:
		return synapInt(int64(val))
	case uint8:
		return synapInt(int64(val))
	case uint16:
		return synapInt(int64(val))
	case uint32:
		return synapInt(int64(val))
	case uint64:
		return synapInt(int64(val))
	case float32:
		return synapFloat(float64(val))
	case float64:
		return synapFloat(val)
	case []byte:
		return synapBytes(val)
	default:
		return synapStr(fmt.Sprintf("%v", val))
	}
}

// fromWireValue converts a SynapValue back to a plain Go value.
func fromWireValue(v SynapValue) interface{} {
	if v.tag == "Null" || v.tag == "" {
		return nil
	}
	return v.inner
}

// encodeSynapValue encodes a SynapValue into its msgpack wire form.
// Null → string "Null"; others → single-key fixmap.
func encodeSynapValue(enc *msgpack.Encoder, sv SynapValue) error {
	if sv.tag == "Null" {
		return enc.EncodeString("Null")
	}
	// Encode as a single-key map: {tag: inner}
	if err := enc.EncodeMapLen(1); err != nil {
		return err
	}
	if err := enc.EncodeString(sv.tag); err != nil {
		return err
	}
	switch sv.tag {
	case "Str":
		return enc.EncodeString(sv.inner.(string))
	case "Int":
		return enc.EncodeInt(sv.inner.(int64))
	case "Float":
		return enc.EncodeFloat64(sv.inner.(float64))
	case "Bool":
		return enc.EncodeBool(sv.inner.(bool))
	case "Bytes":
		return enc.EncodeBytes(sv.inner.([]byte))
	default:
		return enc.Encode(sv.inner)
	}
}

// rpcRequest is the on-wire request: array [id, command, args].
// Serialized as msgpack array (not map) to match serde tuple encoding.
type rpcRequest struct {
	ID      uint32
	Command string
	Args    []SynapValue
}

// encodedRequest serializes an rpcRequest to msgpack bytes.
func encodedRequest(req rpcRequest) ([]byte, error) {
	var buf strings.Builder
	_ = buf // use bytes.Buffer instead
	return marshalRequest(req)
}

func marshalRequest(req rpcRequest) ([]byte, error) {
	var out []byte
	enc := msgpack.GetEncoder()
	defer msgpack.PutEncoder(enc)

	// We need a byte-level writer. Use msgpack.Marshal with a custom type.
	// The request is encoded as a 3-element msgpack array: [id, command, args_array].
	type wireReq struct {
		_msgpack struct{} `msgpack:",asArray"`
		ID       uint32
		Command  string
		Args     []interface{}
	}
	_ = enc
	_ = out

	// Build args as interface{} slices that encode as the wire envelope.
	// We can't use msgpack.Marshal directly for SynapValue since it needs custom
	// encoding. Instead, build the raw msgpack bytes manually.
	return marshalRequestManual(req)
}

func marshalRequestManual(req rpcRequest) ([]byte, error) {
	// Use the low-level msgpack writer to construct: fixarray(3) + id + command + array(N args)
	// Then for each arg, inline the SynapValue encoding.
	//
	// We'll use a buffer and a msgpack.Encoder with a custom writer.
	type bufWriter struct {
		data []byte
	}
	bw := &bufWriterImpl{}
	enc := msgpack.NewEncoder(bw)
	enc.SetCustomStructTag("msgpack")

	// Outer array of 3 elements: [id, command, args]
	if err := enc.EncodeArrayLen(3); err != nil {
		return nil, err
	}
	if err := enc.EncodeUint32(req.ID); err != nil {
		return nil, err
	}
	if err := enc.EncodeString(req.Command); err != nil {
		return nil, err
	}
	// Args sub-array
	if err := enc.EncodeArrayLen(len(req.Args)); err != nil {
		return nil, err
	}
	for _, arg := range req.Args {
		if err := encodeSynapValue(enc, arg); err != nil {
			return nil, err
		}
	}
	return bw.data, nil
}

// bufWriterImpl implements io.Writer into a growing []byte slice.
type bufWriterImpl struct {
	data []byte
}

func (b *bufWriterImpl) Write(p []byte) (n int, err error) {
	b.data = append(b.data, p...)
	return len(p), nil
}

// decodedResponse holds the decoded response from the server.
type decodedResponse struct {
	ID     uint32
	Result interface{} // the unwrapped Ok value or an error
	IsErr  bool
	ErrMsg string
}

// decodeResponse decodes a msgpack response frame.
// Wire format: array [id, {Ok: wire_value} | {Err: string}]
func decodeResponse(data []byte) (decodedResponse, error) {
	dec := msgpack.NewDecoder(strings.NewReader(""))
	_ = dec

	// Decode manually: array(2), uint32 id, map(1) with key Ok|Err
	d := msgpack.NewDecoder(bytesReader(data))

	// Decode array header — expect 2 elements
	arrLen, err := d.DecodeArrayLen()
	if err != nil {
		return decodedResponse{}, fmt.Errorf("response decode: array len: %w", err)
	}
	if arrLen != 2 {
		return decodedResponse{}, fmt.Errorf("response decode: expected array(2), got array(%d)", arrLen)
	}

	// Decode request ID
	id, err := d.DecodeUint32()
	if err != nil {
		return decodedResponse{}, fmt.Errorf("response decode: id: %w", err)
	}

	// Decode result envelope: map(1) with key "Ok" or "Err"
	mapLen, err := d.DecodeMapLen()
	if err != nil {
		return decodedResponse{}, fmt.Errorf("response decode: map len: %w", err)
	}
	if mapLen != 1 {
		return decodedResponse{}, fmt.Errorf("response decode: expected map(1), got map(%d)", mapLen)
	}

	key, err := d.DecodeString()
	if err != nil {
		return decodedResponse{}, fmt.Errorf("response decode: result key: %w", err)
	}

	switch key {
	case "Ok":
		val, err := decodeWireValue(d)
		if err != nil {
			return decodedResponse{}, fmt.Errorf("response decode: Ok value: %w", err)
		}
		return decodedResponse{ID: id, Result: val}, nil
	case "Err":
		msg, err := d.DecodeString()
		if err != nil {
			return decodedResponse{}, fmt.Errorf("response decode: Err message: %w", err)
		}
		return decodedResponse{ID: id, IsErr: true, ErrMsg: msg}, nil
	default:
		return decodedResponse{}, fmt.Errorf("response decode: unexpected result key: %q", key)
	}
}

// decodeWireValue decodes a SynapValue from the msgpack decoder and returns
// the unwrapped Go value (string, int64, float64, bool, []byte, []interface{},
// map[string]interface{}, or nil).
func decodeWireValue(d *msgpack.Decoder) (interface{}, error) {
	// Peek at the msgpack type to determine the tag format.
	// "Null" encodes as a msgpack string; others as a single-key map.
	code, err := d.PeekCode()
	if err != nil {
		return nil, err
	}

	// msgpack fixstr starts at 0xa0; str8/16/32 are 0xd9/0xda/0xdb
	isStr := (code >= 0xa0 && code <= 0xbf) || code == 0xd9 || code == 0xda || code == 0xdb

	if isStr {
		tag, err := d.DecodeString()
		if err != nil {
			return nil, err
		}
		if tag != "Null" {
			return nil, fmt.Errorf("unexpected string tag in wire value: %q", tag)
		}
		return nil, nil
	}

	// Must be a map of length 1
	mapLen, err := d.DecodeMapLen()
	if err != nil {
		return nil, fmt.Errorf("wire value: expected map: %w", err)
	}
	if mapLen != 1 {
		return nil, fmt.Errorf("wire value: expected map(1), got map(%d)", mapLen)
	}

	tag, err := d.DecodeString()
	if err != nil {
		return nil, fmt.Errorf("wire value: tag: %w", err)
	}

	switch tag {
	case "Str":
		return d.DecodeString()
	case "Int":
		return d.DecodeInt64()
	case "Float":
		return d.DecodeFloat64()
	case "Bool":
		return d.DecodeBool()
	case "Bytes":
		b, err := d.DecodeBytes()
		if err != nil {
			return nil, err
		}
		// Return as string if it looks like UTF-8 text (common for KV values).
		return string(b), nil
	case "Array":
		n, err := d.DecodeArrayLen()
		if err != nil {
			return nil, err
		}
		out := make([]interface{}, n)
		for i := 0; i < n; i++ {
			v, err := decodeWireValue(d)
			if err != nil {
				return nil, err
			}
			out[i] = v
		}
		return out, nil
	case "Map":
		// Encoded as array of [key, value] pairs.
		pairCount, err := d.DecodeArrayLen()
		if err != nil {
			return nil, err
		}
		m := make(map[string]interface{}, pairCount)
		for i := 0; i < pairCount; i++ {
			// Each pair is an array of 2 wire values.
			pairLen, err := d.DecodeArrayLen()
			if err != nil {
				return nil, err
			}
			if pairLen != 2 {
				return nil, fmt.Errorf("Map pair expected length 2, got %d", pairLen)
			}
			k, err := decodeWireValue(d)
			if err != nil {
				return nil, err
			}
			v, err := decodeWireValue(d)
			if err != nil {
				return nil, err
			}
			m[fmt.Sprintf("%v", k)] = v
		}
		return m, nil
	default:
		return nil, fmt.Errorf("wire value: unknown tag: %q", tag)
	}
}

// bytesReader wraps a []byte for use as an io.Reader.
type bytesReaderImpl struct {
	data []byte
	pos  int
}

func bytesReader(data []byte) *bytesReaderImpl {
	return &bytesReaderImpl{data: data}
}

func (r *bytesReaderImpl) Read(p []byte) (n int, err error) {
	if r.pos >= len(r.data) {
		return 0, fmt.Errorf("EOF")
	}
	n = copy(p, r.data[r.pos:])
	r.pos += n
	return n, nil
}

// ── SynapRPC transport ────────────────────────────────────────────────────────

// rpcPending tracks an in-flight request.
type rpcPending struct {
	resultCh chan interface{}
	errCh    chan error
}

// SynapRpcTransport is a persistent TCP connection to the SynapRPC listener.
// It multiplexes requests by ID; responses are matched and dispatched.
type SynapRpcTransport struct {
	host    string
	port    int
	timeout time.Duration

	mu       sync.Mutex
	conn     net.Conn
	nextID   uint32
	pending  map[uint32]*rpcPending
	readBuf  []byte
	connErr  error
	connOnce sync.Once
}

// newSynapRpcTransport creates a new SynapRPC transport. The TCP connection is
// established lazily on the first call to Execute.
func newSynapRpcTransport(host string, port int, timeout time.Duration) *SynapRpcTransport {
	return &SynapRpcTransport{
		host:    host,
		port:    port,
		timeout: timeout,
		pending: make(map[uint32]*rpcPending),
	}
}

// connect establishes the TCP connection and starts the read loop.
// Must be called with t.mu held.
func (t *SynapRpcTransport) connect() error {
	addr := net.JoinHostPort(t.host, fmt.Sprintf("%d", t.port))
	conn, err := net.DialTimeout("tcp", addr, t.timeout)
	if err != nil {
		return fmt.Errorf("SynapRPC connect %s: %w", addr, err)
	}
	t.conn = conn
	t.readBuf = nil
	go t.readLoop(conn)
	return nil
}

// ensureConnected makes sure the TCP connection is alive, reconnecting once if needed.
func (t *SynapRpcTransport) ensureConnected() error {
	t.mu.Lock()
	defer t.mu.Unlock()
	if t.conn != nil {
		return nil
	}
	return t.connect()
}

// readLoop reads frames from conn and dispatches them to waiting goroutines.
// Runs in its own goroutine; terminates when conn is closed.
func (t *SynapRpcTransport) readLoop(conn net.Conn) {
	buf := make([]byte, 0, 4096)
	tmp := make([]byte, 4096)

	for {
		n, err := conn.Read(tmp)
		if err != nil {
			// Connection closed — fail all pending requests.
			t.mu.Lock()
			if t.conn == conn {
				t.conn = nil
			}
			pend := t.pending
			t.pending = make(map[uint32]*rpcPending)
			t.mu.Unlock()

			for _, p := range pend {
				p.errCh <- fmt.Errorf("SynapRPC connection closed: %w", err)
			}
			return
		}
		buf = append(buf, tmp[:n]...)

		// Drain complete frames.
		for len(buf) >= 4 {
			frameLen := binary.LittleEndian.Uint32(buf[:4])
			total := 4 + int(frameLen)
			if len(buf) < total {
				break
			}
			frame := make([]byte, frameLen)
			copy(frame, buf[4:total])
			buf = buf[total:]

			resp, err := decodeResponse(frame)
			if err != nil {
				// Corrupt frame — skip silently.
				continue
			}

			t.mu.Lock()
			pend, ok := t.pending[resp.ID]
			if ok {
				delete(t.pending, resp.ID)
			}
			t.mu.Unlock()

			if !ok {
				continue
			}
			if resp.IsErr {
				pend.errCh <- newServerError(resp.ErrMsg)
			} else {
				pend.resultCh <- resp.Result
			}
		}
	}
}

// Execute sends a command with the given args over SynapRPC and returns the
// decoded response value (unwrapped from WireValue).
func (t *SynapRpcTransport) Execute(ctx context.Context, cmd string, args []interface{}) (interface{}, error) {
	if err := t.ensureConnected(); err != nil {
		// Retry once.
		if err2 := t.ensureConnected(); err2 != nil {
			return nil, err2
		}
	}

	id := atomic.AddUint32(&t.nextID, 1)

	wireArgs := make([]SynapValue, len(args))
	for i, a := range args {
		wireArgs[i] = toWireValue(a)
	}

	req := rpcRequest{
		ID:      id,
		Command: strings.ToUpper(cmd),
		Args:    wireArgs,
	}

	body, err := marshalRequestManual(req)
	if err != nil {
		return nil, fmt.Errorf("SynapRPC marshal: %w", err)
	}

	frame := make([]byte, 4+len(body))
	binary.LittleEndian.PutUint32(frame[:4], uint32(len(body)))
	copy(frame[4:], body)

	pend := &rpcPending{
		resultCh: make(chan interface{}, 1),
		errCh:    make(chan error, 1),
	}

	t.mu.Lock()
	if t.conn == nil {
		t.mu.Unlock()
		return nil, fmt.Errorf("SynapRPC: no connection")
	}
	t.pending[id] = pend
	conn := t.conn
	t.mu.Unlock()

	// Apply context deadline to the write.
	if dl, ok := ctx.Deadline(); ok {
		_ = conn.SetWriteDeadline(dl)
	}
	if _, err := conn.Write(frame); err != nil {
		t.mu.Lock()
		delete(t.pending, id)
		t.conn = nil
		t.mu.Unlock()
		// Reconnect and retry once.
		if err2 := t.ensureConnected(); err2 == nil {
			return t.Execute(ctx, cmd, args)
		}
		return nil, fmt.Errorf("SynapRPC write: %w", err)
	}

	select {
	case result := <-pend.resultCh:
		return result, nil
	case err := <-pend.errCh:
		return nil, err
	case <-ctx.Done():
		t.mu.Lock()
		delete(t.pending, id)
		t.mu.Unlock()
		return nil, ctx.Err()
	}
}

// Close tears down the underlying TCP connection.
func (t *SynapRpcTransport) Close() {
	t.mu.Lock()
	defer t.mu.Unlock()
	if t.conn != nil {
		_ = t.conn.Close()
		t.conn = nil
	}
}
