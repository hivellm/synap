# Proposal: phase1_broker-observability-metrics

Source: GitHub issue #196 — "Observability gap: /metrics exposes only process
CPU+memory (no stream/consumer/lag gauges); high idle CPU"

## Why
`GET /metrics` currently exposes only process CPU + memory gauges. There are no
broker-level gauges — no per-stream length, no consumer-group lag/offset, no
pending/in-flight counts. For a message broker this makes it impossible to see
backlog or a stuck consumer group from the outside. Cortex runs multiple consumer
groups (cortex-embedder, graph, fulltext, classifier, consolidator) and cannot
observe their lag at all.

Separately, `synap_process_cpu_usage_percent` reported ~1–2 cores fully busy on an
idle broker (no active ingestion), suggesting a busy poll loop without backoff.

## What Changes
1. **Broker gauges populated at scrape time**: on each `/metrics` scrape, read live
   broker state and set per-stream and per-consumer-group gauges:
   - stream length (events per stream/room)
   - consumer-group lag = last-published offset − last-acked offset
   - pending / in-flight (delivered-but-unacked) counts
   - reuse/populate the already-declared `STREAM_BUFFER_SIZE`, `STREAM_SUBSCRIBERS`
     gauges; add new consumer-group gauges for lag/pending/offset.
2. **Wire broker state into the metrics update path**: pass the shared stream +
   consumer-group registry into the `/metrics` handler so a scrape can enumerate
   streams and groups and read their counts.
3. **Idle CPU fix**: profile the idle path, find the busy poll loop, and replace
   the tight poll with a blocking `await` / proper backoff so idle CPU drops to
   ~0.
4. **Tests**: unit/integration coverage asserting the new gauges appear in
   `/metrics` output with correct values after publish/ack, and (where feasible)
   a regression guard around the fixed loop.

## Impact
- Affected specs: observability / metrics
- Affected code: `synap-server/src/metrics/mod.rs`,
  `synap-server/src/server/metrics_handler.rs`, `synap-server/src/server/{router,mod}.rs`,
  stream/consumer-group modules, and the identified busy-loop module.
- Breaking change: NO (additive metrics; internal loop fix)
- User benefit: operators can see stream backlog and consumer-group lag in
  Prometheus/Grafana; idle CPU no longer wasted on a busy loop.
