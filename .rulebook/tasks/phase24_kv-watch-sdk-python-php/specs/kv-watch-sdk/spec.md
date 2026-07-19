# Spec: KV Watch SDK surface — Python & PHP

## ADDED Requirements

### Requirement: Async-iterator watch in the Python SDK
The Python SDK SHALL expose `kv.watch(pattern, mode="value")` returning an async iterator
of `WatchEvent` (key, event, version, value, truncated), decoded from the server envelope.

#### Scenario: Async iteration yields events
Given a Python client iterating `async for event in kv.watch("user:1")`
When the server applies `SET user:1 "alice"`
Then the iterator yields a WatchEvent with key `user:1` and value `alice`

#### Scenario: Closing the iterator unwatches
Given an active watch iterator
When the iterator is closed
Then the SDK issues UNWATCH for the pattern

### Requirement: Callback/iterator watch in the PHP SDK
The PHP SDK SHALL expose a watch API accepting a pattern and a callable (plus an iterator
variant), delivering `WatchEvent` objects, with an explicit unwatch operation.

#### Scenario: Callback receives events
Given a PHP client with `watch("user:*", $cb)` registered
When the server applies `SET user:42 v`
Then `$cb` is invoked with a WatchEvent for `user:42`

#### Scenario: Unwatch stops callbacks
Given a registered watch callback
When `unwatch("user:*")` is called and the key is set again
Then the callback is not invoked

### Requirement: Mode selection
Both SDKs MUST support watch mode `value` (default) and `notify`.

#### Scenario: Notify mode yields events without value
Given a watch in notify mode
When the watched key is set
Then the delivered event has no value and version is present
