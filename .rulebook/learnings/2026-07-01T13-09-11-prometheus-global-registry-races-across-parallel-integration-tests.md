# Prometheus global registry races across parallel integration tests
**Source**: manual
**Date**: 2026-07-01
**Related Task**: phase1_broker-observability-metrics
**Tags**: testing, prometheus, flaky, issue-196
The prometheus crate's default registry is a process-global; prometheus::gather()/encode_metrics() sees ALL registered series. Each Rust integration-test FILE compiles to its own binary (own process), so cross-file isolation is automatic — but #[tokio::test] functions WITHIN one file run on parallel threads sharing that process's registry. A test that calls reset_broker_gauges() (IntGaugeVec::reset) will wipe series another test in the same file just set, causing flaky failures between set and assert. Fix: keep all metric-mutating assertions for a shared gauge family in a SINGLE test function so they run sequentially, or filter asserted series by test-unique labels. Verified in synap-server/tests/metrics_broker_tests.rs (issue #196).