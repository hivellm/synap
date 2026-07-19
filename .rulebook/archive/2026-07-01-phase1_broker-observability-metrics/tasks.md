## 1. Metrics — declarations
- [x] 1.1 Add consumer-group gauges to `metrics/mod.rs`: stream length/last-offset, partition messages/end-offset, group members, committed offset, consumer lag — labelled by stream/topic/partition/group; plus correctly-named host gauges
- [x] 1.2 Add snapshot setter helpers + `reset_broker_gauges()` that set all broker gauges from live state

## 2. Metrics — wiring
- [x] 2.1 Reuse existing manager snapshot methods (`list_rooms`/`room_stats`, `list_topics`/`topic_stats`, `list_groups`/`get_offset`, `list_queues`/`stats`) for per-stream length and per-group offsets/lag
- [x] 2.2 Inject `State<AppState>` into the `/metrics` handler and call `update_broker_metrics` on each scrape (alongside `update_system_metrics`)

## 3. Idle CPU
- [x] 3.1 Audited every background loop (Explore recon): all `.await` on timers/channels/accepts — NO busy-poll loop exists
- [x] 3.2 Root cause was mislabeled metrics (host load-avg reported as process CPU). Fixed by sampling process CPU/RSS via `sysinfo`; host stats moved to `synap_host_*` gauges

## 4. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 4.1 Update or create documentation covering the implementation — `docs/observability.md` + CHANGELOG `[Unreleased]`
- [x] 4.2 Write tests covering the new behavior — `tests/metrics_broker_tests.rs` asserts stream length, partition end-offset, consumer lag/committed, queue depth, stale-series clearing, and process/host metric split
- [x] 4.3 Run tests and confirm they pass — `cargo check` → `clippy -D warnings` → `cargo test` (new test + 704 lib tests green)
