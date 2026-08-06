# Proposal: phase9_server-hang-under-error-load

## Why

A release build of 1.3.1 stopped serving **every** transport during the PHP
transport-parity work, and did not recover. Observed directly:

- HTTP `GET /health` on `127.0.0.1:15500`: connection accepted, then nothing —
  `curl -m 5` timed out with "0 bytes received".
- RESP3 `PING` on `6379`: connection accepted, no reply within 5s.
- The process was alive (`synap-server.exe`, PID 69924, RSS ~17 MB) and all
  three listeners still accepted TCP connections.
- The log's last three lines are `Command error: Invalid request: Missing
  'payload' field` from `handlers/mod.rs:517`, inside a `resp3.conn` span, at
  14:21:17.27. **Nothing** was logged afterwards — including the periodic
  snapshot task, which had been logging "Creating periodic snapshot" every
  cycle up to that point. A tokio worker deadlock is the obvious reading: a
  synchronous lock held across an await, or a re-entrant acquisition, would
  stop the runtime's tasks the way this stopped them.

What ran just before: the parity harness driving KV, hash, list, set, stream,
queue and pub/sub calls over HTTP, SynapRPC and RESP3 in quick succession, with
several deliberately malformed queue publishes (a JSON object where the handler
wants a byte array) reaching the RESP3 connection.

Two direct reproduction attempts afterwards — malformed `QPUBLISH` against a
missing queue, then against an existing one, interleaved with `PING` — did not
hang a fresh server, so it is not a single-command trigger. That makes it a
race, and a hang that takes down every transport at once is the most severe
failure mode this server has: no client can even get an error.

## What Changes

- Reproduce it: replay the parity harness against a debug build under load,
  concurrently across all three transports, with the malformed publishes
  included. Repeat until it reproduces.
- Capture the state when it hangs: attach a debugger or dump stacks
  (`tokio-console`, or a `SIGQUIT`-equivalent thread dump on Windows) to find
  which task holds what.
- Audit the suspects the evidence points at: `parking_lot` guards held across
  `.await` in the command-dispatch path, and any lock the periodic snapshot
  task and the request path both take.
- Add a regression test that hammers every transport with a mix of valid and
  malformed commands and asserts the server still answers.

## Impact

- Affected specs: none yet — a spec follows once the mechanism is known
- Affected code: likely `crates/synap-server/src/server/handlers/mod.rs`,
  the RESP3/SynapRPC connection loops, and whichever store the snapshot task
  shares with them
- Breaking change: NO
- User benefit: the server stops being one unlucky interleaving away from
  going silent on every port at once.
