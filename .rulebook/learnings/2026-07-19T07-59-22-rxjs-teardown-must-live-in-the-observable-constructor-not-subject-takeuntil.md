# rxjs teardown must live in the Observable constructor, not Subject+takeUntil
**Source**: manual
**Date**: 2026-07-19
**Related Task**: phase23_kv-watch-sdk-rust-ts
**Tags**: typescript, rxjs, kv-watch, sdk, phase23
The TS SDK's pubsub.subscribe builds streams as Subject + takeUntil(stopSignal) + share(), where only a separate manager.unsubscribe() fires the stop signal — a plain subscription.unsubscribe() never unwinds the server-side registration or closes the push connection. For kv.watch this would leak a dedicated TCP connection per abandoned watch. Correct shape: new Observable(subscriber => { async setup; return teardownFn }) — teardown runs on unsubscribe, can issue KV.UNWATCH + close, and must handle the torn-down-mid-handshake race (cancel flag checked after the async setup resolves). Applies to any SDK surface that owns a connection per subscription.