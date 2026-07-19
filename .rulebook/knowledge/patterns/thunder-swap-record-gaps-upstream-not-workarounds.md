# Adopting a shared protocol library: file the gap, then annotate the mitigation

**Category**: architecture
**Tags**: thunder, rpc, protocol, dependency-adoption, phase12

## Description

Swapping a hand-maintained subsystem for a shared library (Synap's SynapRPC server → `thunder-rpc`) always surfaces capabilities the library does not express. The failure mode is absorbing them silently: a `to_vec()` here, a dropped config knob there, and six months later nobody knows the product regressed or why. The rule applied in phase12 was: for every capability Thunder could not express, (1) open an issue upstream with the Synap call site and a suggested API shape, (2) implement the mitigation, and (3) comment the mitigation at the call site naming the upstream request. Five gaps came out of the server swap: `Value::Bytes(Vec<u8>)` killing the zero-copy GET/SET path (hivellm/thunder#1), no per-listener `max_connections` (#2), no per-command metrics hook, which degraded two Prometheus histograms to counters (#3), `Principal` carrying only a name, forcing a credential re-lookup per admin command (#4), and `ListenerHandle::stop(self)` making an observer and a graceful stopper mutually exclusive (#5).

**All five were fixed upstream within a day** (`thunder-rpc` 0.1.2 and 0.2.0), and every Synap mitigation was reverted before the release shipped. That is the actual payoff: an issue with a concrete call site and a suggested API shape is cheap for a maintainer to act on, whereas a silent workaround is invisible and permanent. Write the issue *first*, then the mitigation — the issue is what gets the workaround deleted.

## Example

// Mitigation annotated at the call site, naming the upstream request.
/// `thunder::Value::Bytes` owns a `Vec<u8>`, so the buffer is copied once here
/// instead — tracked upstream as the `Arc<[u8]>` payload request.
fn arg_shared(args: &[SynapValue], idx: usize) -> Result<Arc<[u8]>, String> {
    match args.get(idx) { Some(SynapValue::Bytes(b)) => Ok(b.as_slice().into()), /* ... */ }
}

// …and after thunder-rpc 0.2.0 closed the issue, the comment led straight
// back to the line to revert:
fn arg_shared(args: &[SynapValue], idx: usize) -> Result<Arc<[u8]>, String> {
    match args.get(idx) { Some(SynapValue::Bytes(b)) => Ok(Arc::clone(b)), /* ... */ }
}

## When to Use

Any adoption of a shared/external library that replaces first-party code with
its own opinions — protocol crates, runtimes, storage engines.

## When Not To Use

Cosmetic API differences with no behavioral or performance consequence; noise
upstream costs more than it returns.
