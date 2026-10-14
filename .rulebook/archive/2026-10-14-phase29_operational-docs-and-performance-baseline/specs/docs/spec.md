# Guarantees Documentation and Performance Baseline

## ADDED Requirements

### Requirement: Recorded benchmark runs
Every benchmark run MUST record p50, p95 and p99 latency, throughput, memory,
workload, durability mode, build and hardware alongside its results.

#### Scenario: Manual run
Given the benchmark workflow is triggered manually on the controlled runner
When it completes
Then its report contains every recorded field for each workload

### Requirement: No noisy gates
Latency regression gates MUST NOT run on shared CI runners.

#### Scenario: Pull request on a shared runner
Given a pull request
When CI runs on a shared runner
Then no latency threshold can fail the build

### Requirement: Current guarantees documented
The documentation MUST describe the verified 1.4.0 durability, authorization,
Hub and limit behavior, and every historical analysis MUST state the revision
it describes.

#### Scenario: Historical analysis
Given `docs/analysis/redis-parity-deep-dive.md`
When a reader opens it
Then it states the revision it describes and links the current architecture
