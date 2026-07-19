# Documentation Organization

**Last Updated**: 2026-07-19

## Overview

This document describes how `docs/` is structured and where new documentation
belongs. The guiding split is **audience**: what a topic is *for* decides its
directory, not what subsystem it happens to touch.

## Directory Structure

```
docs/
├── INDEX.md                      # Documentation index (start here)
├── ARCHITECTURE.md               # System architecture and data flow
├── PROJECT_DAG.md                # Component dependencies and critical path
├── ROADMAP.md                    # Development roadmap and release timeline
│
├── analysis/                     # Deep-dive investigations (per-topic dirs)
│   ├── kv-watch-observable/
│   ├── redis-parity-deep-dive.md
│   ├── synap-audit/
│   ├── synap-v1-release/
│   └── synap-vs-redis/
│
├── api/                          # API reference
│   ├── README.md
│   ├── REST_API.md
│   ├── PROTOCOL_MESSAGES.md
│   └── openapi.{json,yml}
│
├── benchmarks/                   # Performance data and comparisons
│   ├── README.md
│   ├── BENCHMARK_RESULTS_EXTENDED.md
│   ├── COMPRESSION_BENCHMARKS.md
│   ├── PERSISTENCE_BENCHMARKS.md
│   ├── QUEUE_CONCURRENCY_TESTS.md
│   ├── REDIS_COMPARISON.md
│   └── redis-vs-synap.md
│
├── development/                  # Working ON Synap (contributor-facing)
│   ├── BUILD.md
│   ├── TESTING.md
│   ├── OPENSPEC_WORKFLOW.md
│   ├── RELEASE_PROCESS.md
│   └── rust-target-hygiene.md
│
├── diagrams/                     # Mermaid architecture diagrams
│
├── examples/                     # Worked usage examples
│
├── features/                     # User-visible capabilities
│   ├── ADAPTIVE_CACHING.md
│   ├── AUTHENTICATION.md
│   ├── PROMETHEUS_METRICS.md
│   ├── REPLICATION.md
│   ├── broker-retention-and-prefetch.md
│   ├── kv-watch.md
│   └── transactions.md
│
├── guides/                       # Using Synap (user/admin-facing how-to)
│   ├── ADMIN_GUIDE.md
│   ├── CLI_GUIDE.md
│   ├── HUB_CONFIGURATION.md
│   ├── MIGRATION_AUTH.md
│   ├── MIGRATION_GUIDE.md
│   ├── TUTORIALS.md
│   └── USER_GUIDE.md
│
├── internals/                    # How the engine works inside
│   ├── kv-store.md
│   ├── memory-accounting.md
│   ├── persistence-snapshot-format.md
│   ├── security-auth.md
│   └── simd.md
│
├── operations/                   # Running a server in production
│   ├── network-limits.md
│   └── observability.md
│
├── project/                      # Project meta and status snapshots
│   ├── ORGANIZATION.md           # This file
│   ├── STATUS.md
│   ├── IMPLEMENTATION_COMPLETE.md
│   ├── PHASE4_PROGRESS.md
│   └── TEST_COVERAGE_SUMMARY.md
│
├── protocol/                     # Wire protocols and transports
│   ├── STREAMABLE_HTTP.md
│   ├── MCP_INTEGRATION.md
│   ├── MCP_USAGE.md
│   ├── MCP_TEST_RESULTS.md
│   ├── UMICP_INTEGRATION.md
│   ├── transports.md
│   └── thunder-interop-matrix.md
│
├── sdks/                         # Per-language SDK documentation
│
├── specs/                        # Technical specifications (SHALL/MUST)
│
└── users/                        # End-user documentation site
```

## Organization Principles

### 1. Audience decides the directory

The same subsystem can legitimately appear in several places, described for a
different reader each time. KV watch is the worked example:

| Directory | What it answers | KV watch |
|---|---|---|
| `features/` | What can I do with it? | `features/kv-watch.md` |
| `specs/` | What must it guarantee? | requirement specs |
| `internals/` | How is it built? | notifier/version-counter design |
| `operations/` | How do I run it safely? | limits, observability |
| `api/` | What is the exact call? | REST/RPC reference |

### 2. `guides/` is *using*, `development/` is *building*

`guides/` is for someone operating or integrating against Synap.
`development/` is for someone changing the source: build, test, release, and
repository hygiene.

### 3. Root holds only navigation and whole-project shape

`docs/` root is deliberately four files: the index plus the three documents
that describe the project as a whole (architecture, dependency DAG, roadmap).
Anything narrower belongs in a directory.

### 4. Status snapshots live in `project/`

Dated progress reports and completion summaries are historical records, not
reference documentation. They stay in `project/` so they never get mistaken
for current behavior — check the date before trusting one.

### 5. Single source of truth

A topic gets one authoritative home and cross-links from anywhere else that
mentions it. When a document moves, update every inbound link in the same
change; broken links are worse than a stale location.

## Finding Documentation

📍 Start at **[INDEX.md](../INDEX.md)** — the complete index.

By purpose:

- **Learning Synap** → `../ARCHITECTURE.md`, then `specs/`
- **Using Synap** → `guides/`, `examples/`, `sdks/`
- **A specific capability** → `features/`
- **Deploying / operating** → `operations/`, `specs/DEPLOYMENT.md`, `specs/CONFIGURATION.md`
- **Performance data** → `benchmarks/`
- **API reference** → `api/REST_API.md`
- **Contributing** → `development/`
- **How it works inside** → `internals/`

## Maintenance Guidelines

### Adding documentation

1. Pick the directory by **audience** (see the table above), not by subsystem.
2. Add the file, then link it from `INDEX.md` — an unlinked document is an
   invisible one.
3. Cross-reference the related documents in the other audience directories.

### Moving documentation

1. Use `git mv` so history follows the file.
2. Fix every inbound link — including code comments and config files, which
   reference doc paths too.
3. Update `INDEX.md` and this file if a directory is added or repurposed.

### Deprecating documentation

1. Do not delete: move it under `project/` (or `analysis/` for investigations)
   and add a dated header saying what superseded it.
2. Update `INDEX.md`.

## Quality Standards

All documentation must:

- Be written in English
- Include code examples where applicable
- Cross-reference related documents
- State a date on anything time-sensitive (status, benchmarks, roadmaps)
- Follow the existing structure and style

## Contributing

See [DEVELOPMENT.md](../specs/DEVELOPMENT.md) for contribution guidelines.
