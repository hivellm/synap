# Cross-SDK interop matrix

One server build, every SDK, the same four steps.

Each SDK has its own test suite, and each one passes. What none of them can
prove is the thing that actually matters to a user: that a Thunder-based Synap
server and every Synap client still agree on the wire — including any
pre-Thunder client still deployed.

That is this matrix. It is the release gate for 1.2.0.

## Running it

```bash
cargo build --release -p synap-server     # the matrix refuses to run without it
python scripts/interop/run-matrix.py                 # every cell
python scripts/interop/run-matrix.py rust go         # a subset
python scripts/interop/run-matrix.py --list          # what exists
python scripts/interop/run-matrix.py --json out.json # machine-readable results
```

The driver starts one server from [`server-config.yml`](server-config.yml) in a
temporary directory, runs each client against it, and renders the matrix. Ports
are deliberately off the defaults (25500/25501/26379) so a developer's own
server is neither used by accident nor disturbed.

Authentication is **on and required**. The pre-Thunder RPC transports never sent
`AUTH` at all, so an open server would have hidden that bug in every SDK instead
of surfacing it in the `auth` column — which is exactly what it did.

## The four steps

| Step | What it proves |
|---|---|
| `auth` | Credentials reach the server on the RPC port. Probed with `EXISTS`, not `PING`: the server answers `PING` *before* authentication, so a `PING` probe passes just as happily on a connection that never authenticated. |
| `kv_binary` | A value that is not valid UTF-8 (`deadbeef`) survives `SET`/`GET` byte-exact. |
| `pubsub` | `SUBSCRIBE` establishes a push channel and a `PUBLISH` reaches it. |
| `error` | A server-side error surfaces as an error *and* leaves the multiplexed connection usable. |

## Adding a language

Add a directory under `clients/` and a `Cell` entry in `run-matrix.py`. The
driver never looks inside a client; the contract is the whole interface:

```
argv:   <host> <port> <user> <pass>
stdout: one `STEP <name> PASS|FAIL <detail>` line per step
exit:   0 if every step passed
```

Where the SDK exposes its transport as public API (Python, TypeScript, C#), the
client drives the transport directly — that is where the wire lives. Where it
does not (Go), the client uses the public module API and the difference is
recorded in the matrix's Transport column.

## Toolchain overrides

`SYNAP_INTEROP_<CELL>` replaces the launcher for one cell:

```bash
SYNAP_INTEROP_PHP='C:\php\php.exe' python scripts/interop/run-matrix.py php
```

Needed when the name on `PATH` is not something that can be spawned — a
winget-installed PHP resolves to a Windows App Execution Alias, which fails to
launch from a subprocess with "Access denied".

## Results

The recorded run, and what each red or unrun cell means, is in
[`docs/thunder-interop-matrix.md`](../../docs/thunder-interop-matrix.md).
