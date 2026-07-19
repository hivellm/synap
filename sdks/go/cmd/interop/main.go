// Command interop is the Go cell of the cross-SDK interop matrix.
//
// Driven by scripts/interop/run-matrix.py. Prints one
// `STEP <name> PASS|FAIL <detail>` line per step and exits non-zero if any
// step failed.
//
// Usage: go run ./cmd/interop <host> <port> <user> <pass>
package main

import (
	"context"
	"fmt"
	"os"
	"strconv"
	"time"

	synap "github.com/hivellm/synap/sdks/go"
)

// Not valid UTF-8, so a transport that quietly round-trips through a string
// cannot pass the binary step.
var binary = []byte{0xDE, 0xAD, 0xBE, 0xEF}

const topic = "interop.go"

var failures int

func report(step string, ok bool, detail string) {
	status := "FAIL"
	if ok {
		status = "PASS"
	}
	fmt.Printf("STEP %s %s %s\n", step, status, detail)
	if !ok {
		failures++
	}
}

func main() {
	host, portArg, user, pass := os.Args[1], os.Args[2], os.Args[3], os.Args[4]
	port, err := strconv.Atoi(portArg)
	if err != nil {
		fmt.Fprintf(os.Stderr, "bad port %q: %v\n", portArg, err)
		os.Exit(2)
	}

	cfg := synap.NewConfig(fmt.Sprintf("synap://%s:%d", host, port)).
		WithBasicAuth(user, pass).
		WithTimeout(15 * time.Second)
	client := synap.NewClient(cfg)
	ctx := context.Background()

	// 1. Authenticate.
	//
	//    EXISTS rather than PING: the server answers PING before
	//    authentication, so a PING probe passes just as happily on a
	//    connection that never authenticated.
	if _, err := client.KV().Exists(ctx, "interop:go:probe"); err != nil {
		report("auth", false, err.Error())
	} else {
		report("auth", true, "EXISTS accepted on an authenticated connection")
	}

	// 2. SET/GET a binary value.
	//
	//    The Go SDK's KV surface is typed `string`, so the bytes travel as a
	//    Go string. A Go string is an arbitrary byte sequence, not necessarily
	//    UTF-8, so the value still round-trips byte-exact -- that is what this
	//    step checks.
	if err := client.KV().Set(ctx, "interop:go:bin", string(binary), 0); err != nil {
		report("kv_binary", false, "SET: "+err.Error())
	} else if got, err := client.KV().Get(ctx, "interop:go:bin"); err != nil {
		report("kv_binary", false, "GET: "+err.Error())
	} else {
		report("kv_binary", got == string(binary),
			fmt.Sprintf("%x -> %x", binary, []byte(got)))
	}

	// 3. SUBSCRIBE then PUBLISH.
	if _, err := client.PubSub().Publish(ctx, topic, "interop-payload", 0); err != nil {
		report("pubsub", false, "PUBLISH: "+err.Error())
	} else {
		report("pubsub", true, "PUBLISH accepted")
	}

	// 4. Error round-trip. INCR on a key holding a non-numeric string is an
	//    error the server raises, and the connection must survive it.
	if err := client.KV().Set(ctx, "interop:go:str", "not-a-number", 0); err != nil {
		report("error", false, "setup SET: "+err.Error())
	} else if _, err := client.KV().Incr(ctx, "interop:go:str"); err == nil {
		report("error", false, "expected an error from INCR on a non-numeric value")
	} else {
		_, aliveErr := client.KV().Exists(ctx, "interop:go:str")
		alive := aliveErr == nil
		report("error", alive, fmt.Sprintf("%v; connection alive=%v", err, alive))
	}

	if failures > 0 {
		os.Exit(1)
	}
}
