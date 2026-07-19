package synap

import (
	"bytes"
	"context"
	"encoding/binary"
	"io"
	"net"
	"testing"
	"time"

	"github.com/vmihailenco/msgpack/v5"
)

// The server's `Bytes` encoding changed in Synap 1.1.0: it now emits
// MessagePack `bin` (~33% smaller) where it used to emit an array of integers.
// A client that understands only one of the two silently breaks against a
// server of the other vintage, so both must decode to the same string.
func TestUnwrapSynapValueAcceptsBothBytesEncodings(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name string
		wire interface{}
	}{
		{
			// What a 1.1.0+ server emits: MessagePack bin, decoded as []byte.
			name: "canonical bin",
			wire: map[string]interface{}{"Bytes": []byte("hello")},
		},
		{
			// What a pre-1.1.0 server emits: rmp_serde's Vec<u8> as an array
			// of integers. Kept working indefinitely.
			name: "legacy int array",
			wire: map[string]interface{}{"Bytes": []interface{}{
				int64('h'), int64('e'), int64('l'), int64('l'), int64('o'),
			}},
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			got := unwrapSynapValue(tc.wire)
			if got != "hello" {
				t.Fatalf("expected %q, got %#v", "hello", got)
			}
		})
	}
}

// The cap must match the server's, or this client rejects frames the server
// considers legitimate. It was 64 MiB against a 512 MiB server.
func TestMaxFrameBytesMatchesTheServerCap(t *testing.T) {
	t.Parallel()

	const serverCap = 512 * 1024 * 1024
	if maxFrameBytes != serverCap {
		t.Fatalf("frame cap %d does not match the server's %d", maxFrameBytes, serverCap)
	}
}

// A length prefix above the cap must be refused on the header alone — before
// the body buffer is allocated, and without waiting for a body that a hostile
// peer need never send.
func TestExecuteRefusesOverCapLengthPrefix(t *testing.T) {
	t.Parallel()

	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("listen: %v", err)
	}
	defer listener.Close()

	go func() {
		conn, acceptErr := listener.Accept()
		if acceptErr != nil {
			return
		}
		defer conn.Close()

		// Read the request frame the client sends.
		header := make([]byte, 4)
		if _, readErr := io.ReadFull(conn, header); readErr != nil {
			return
		}
		bodyLen := binary.LittleEndian.Uint32(header)
		if _, readErr := io.ReadFull(conn, make([]byte, bodyLen)); readErr != nil {
			return
		}

		// Answer with a header claiming more than the cap, and no body.
		reply := make([]byte, 4)
		binary.LittleEndian.PutUint32(reply, uint32(maxFrameBytes)+1)
		_, _ = conn.Write(reply)

		// Hold the connection open so a client that allocated first would
		// block here rather than failing.
		time.Sleep(2 * time.Second)
	}()

	addr, ok := listener.Addr().(*net.TCPAddr)
	if !ok {
		t.Fatalf("unexpected listener address type %T", listener.Addr())
	}

	transport := newSynapRpcTransport("127.0.0.1", addr.Port, 5*time.Second)
	defer transport.Close()

	if _, execErr := transport.Execute(context.Background(), "GET", []interface{}{"k"}); execErr == nil {
		t.Fatal("expected an over-cap frame to be refused")
	}
}

// The request encoding is unchanged by the cap and decoding work: decode it
// here with msgpack directly, so this asserts the wire rather than the SDK's
// own round-trip.
func TestExecuteSendsExternallyTaggedArgs(t *testing.T) {
	t.Parallel()

	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("listen: %v", err)
	}
	defer listener.Close()

	received := make(chan []byte, 1)

	go func() {
		conn, acceptErr := listener.Accept()
		if acceptErr != nil {
			return
		}
		defer conn.Close()

		header := make([]byte, 4)
		if _, readErr := io.ReadFull(conn, header); readErr != nil {
			return
		}
		body := make([]byte, binary.LittleEndian.Uint32(header))
		if _, readErr := io.ReadFull(conn, body); readErr != nil {
			return
		}
		received <- body

		// Reply Ok("PONG") so Execute returns cleanly.
		reply, marshalErr := msgpack.Marshal([]interface{}{
			uint32(1),
			map[string]interface{}{"Ok": map[string]interface{}{"Str": "PONG"}},
		})
		if marshalErr != nil {
			return
		}
		out := make([]byte, 4+len(reply))
		binary.LittleEndian.PutUint32(out[:4], uint32(len(reply)))
		copy(out[4:], reply)
		_, _ = conn.Write(out)
	}()

	addr, ok := listener.Addr().(*net.TCPAddr)
	if !ok {
		t.Fatalf("unexpected listener address type %T", listener.Addr())
	}

	transport := newSynapRpcTransport("127.0.0.1", addr.Port, 5*time.Second)
	defer transport.Close()

	got, err := transport.Execute(context.Background(), "get", []interface{}{"mykey"})
	if err != nil {
		t.Fatalf("execute: %v", err)
	}
	if got != "PONG" {
		t.Fatalf("expected PONG, got %#v", got)
	}

	body := <-received
	if !bytes.Contains(body, []byte("GET")) {
		t.Errorf("command should be upper-cased on the wire, got %q", body)
	}
	if !bytes.Contains(body, []byte("Str")) {
		t.Errorf("args should be externally tagged, got %q", body)
	}
	if !bytes.Contains(body, []byte("mykey")) {
		t.Errorf("argument value missing from the frame, got %q", body)
	}
}
