# Testing a timer-driven writer with fixed real sleeps

**Category**: testing
**Tags**: tokio, timers, flaky-tests, wal, phase20

## Description

The phase20 WAL tests (`periodic_mode_fsyncs_while_idle`, `periodic_interval_fsync_failure_poisons`) slept on the wall clock around the Periodic fsync interval and failed under load. Three timing assumptions were wrong: (1) `tokio::time::interval`'s first tick completes immediately, so a write queued before the writer task first runs races that tick in `select!` and gets fsynced within milliseconds — the test mostly observed that tick, not the idle interval; (2) the writer's `sync_due` compared wall-clock time while the timer ran on tokio time, so a stall longer than the interval made the write's own commit fsync and consume an injected failure; (3) fixed sleeps had to cover a real fsync on a loaded Windows disk. Fix: `#[tokio::test(start_paused = true)]` (the paused clock only advances when the runtime is idle, so blocking fsync work finishes first), `yield_now()` after opening so the writer consumes its immediate first tick before any write, and an interval (1 h) no wall-clock stall can reach.

## When to Use

Any test of code driven by tokio::time::interval/sleep, especially when the code also checks real elapsed time.

## When NOT to Use

Tests that must exercise real I/O latency or real wall-clock deadlines.
