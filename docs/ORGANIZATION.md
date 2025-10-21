# Documentation Organization

**Last Updated**: 2025-10-21

## Overview

The Synap documentation has been reorganized for better clarity and maintainability. This document describes the current structure and organization principles.

## Directory Structure

```
docs/
├── api/                          # API Documentation
│   ├── REST_API.md              # HTTP REST API reference
│   └── PROTOCOL_MESSAGES.md     # Protocol message formats
│
├── benchmarks/                   # Performance Benchmarks
│   ├── README.md                # Benchmark guide
│   ├── BENCHMARK_RESULTS_EXTENDED.md
│   ├── PERSISTENCE_BENCHMARKS.md
│   └── QUEUE_CONCURRENCY_TESTS.md
│
├── diagrams/                     # Architecture Diagrams
│   ├── system-architecture.mmd
│   ├── event-stream.mmd
│   ├── message-flow.mmd
│   └── replication-flow.mmd
│
├── examples/                     # Usage Examples
│   ├── CHAT_SAMPLE.md
│   ├── EVENT_BROADCAST.md
│   ├── PUBSUB_PATTERN.md
│   └── TASK_QUEUE.md
│
├── protocol/                     # Protocol Specifications
│   ├── STREAMABLE_HTTP.md
│   ├── MCP_INTEGRATION.md
│   └── UMICP_INTEGRATION.md
│
├── sdks/                         # SDK Documentation
│   ├── TYPESCRIPT.md
│   ├── PYTHON.md
│   └── RUST.md
│
├── specs/                        # Technical Specifications
│   ├── COMPRESSION_AND_CACHE.md
│   ├── CONFIGURATION.md
│   ├── DEPLOYMENT.md
│   ├── DEVELOPMENT.md
│   ├── EVENT_STREAM.md
│   ├── GUI_DASHBOARD.md
│   ├── KEY_VALUE_STORE.md
│   ├── OPTIMIZATION.md
│   ├── PACKAGING_AND_DISTRIBUTION.md
│   ├── PERFORMANCE.md
│   ├── PERFORMANCE_OPTIMIZATIONS.md
│   ├── PERSISTENCE.md
│   ├── PROTOCOL_SUPPORT.md
│   ├── PUBSUB.md
│   ├── QUEUE_SYSTEM.md
│   └── REPLICATION.md
│
├── ARCHITECTURE.md               # System Architecture
├── AUTHENTICATION.md             # Authentication Guide
├── BUILD.md                      # Build Instructions
├── CLI_GUIDE.md                  # CLI Documentation
├── COMPETITIVE_ANALYSIS.md       # Competitive Analysis
├── DESIGN_DECISIONS.md           # Design Rationale
├── INDEX.md                      # Documentation Index (Start Here!)
├── PROJECT_DAG.md                # Component Dependencies
├── ROADMAP.md                    # Development Roadmap
└── TESTING.md                    # Testing Guide
```

## Organization Principles

### 1. Specifications in `specs/`
All technical specifications live in the `specs/` directory:
- Component specifications (KEY_VALUE_STORE, QUEUE_SYSTEM, etc.)
- Configuration and deployment specs
- Performance and optimization specs
- Development and packaging specs

### 2. Benchmarks in `benchmarks/`
All performance benchmarks and test results:
- Separated from specifications for clarity
- Includes README with methodology
- Easy to update without touching specs

### 3. No Redundancy
- Removed duplicate CONFIGURATION.md from root
- Consolidated performance documentation
- Single source of truth for each topic

### 4. Clear Separation of Concerns
- **api/** - API references and protocol messages
- **benchmarks/** - Performance data and analysis
- **diagrams/** - Visual architecture representations
- **examples/** - Practical usage examples
- **protocol/** - Protocol specifications
- **sdks/** - Client library documentation
- **specs/** - Technical specifications

### 5. Root-Level Only for High-Level Docs
Root `docs/` directory only contains:
- High-level architecture and design
- Guides (BUILD, CLI, TESTING)
- Project planning (ROADMAP, PROJECT_DAG)
- Navigation (INDEX.md)

## Changes Made (2025-10-21)

### Created
- ✅ `benchmarks/` directory
- ✅ `benchmarks/README.md` - Benchmark guide and methodology

### Moved
- ✅ `BENCHMARK_RESULTS_EXTENDED.md` → `benchmarks/`
- ✅ `PERSISTENCE_BENCHMARKS.md` → `benchmarks/`
- ✅ `QUEUE_CONCURRENCY_TESTS.md` → `benchmarks/`
- ✅ `OPTIMIZATION.md` → `specs/`
- ✅ `PERFORMANCE_OPTIMIZATIONS.md` → `specs/`

### Removed
- ✅ `CONFIGURATION.md` (duplicate, kept in `specs/`)
- ✅ `IMPLEMENTATION_STATUS.md` (temporary file)
- ✅ `PHASE1_SUMMARY.md` (temporary file)
- ✅ `SUMMARY.md` (redundant)

### Updated
- ✅ `INDEX.md` - Updated all links to reflect new structure
- ✅ Added benchmarks section to INDEX.md
- ✅ Updated documentation status table

## Finding Documentation

### Start Here
📍 **[INDEX.md](INDEX.md)** - Complete documentation index with links to everything

### By Purpose
- **Learning Synap**: Start with README → ARCHITECTURE → specs/
- **Using Synap**: Check examples/ and sdks/
- **Deploying Synap**: See specs/DEPLOYMENT.md and specs/CONFIGURATION.md
- **Performance Data**: Browse benchmarks/
- **API Reference**: See api/REST_API.md
- **Building Synap**: Follow BUILD.md

### By Component
Each component has:
1. Specification in `specs/`
2. API documentation in `api/REST_API.md`
3. Usage examples in `examples/`
4. Benchmark data in `benchmarks/`
5. SDK support in `sdks/`

## Maintenance Guidelines

### Adding New Documentation
1. **Specifications** → Add to `specs/`
2. **Benchmarks** → Add to `benchmarks/`
3. **Examples** → Add to `examples/`
4. **Diagrams** → Add to `diagrams/`
5. **API Changes** → Update `api/`
6. **Always update INDEX.md** with new files

### Updating Existing Documentation
1. Keep directory structure intact
2. Update INDEX.md if changing file locations
3. Maintain cross-references between documents
4. Update "Last Updated" dates

### Deprecating Documentation
1. Don't delete immediately - move to `deprecated/` (create if needed)
2. Update INDEX.md
3. Add deprecation notice to file header
4. Keep for at least one release cycle

## Documentation Statistics

| Category | Files | Lines |
|----------|-------|-------|
| Specifications | 16 | ~8,500 |
| Benchmarks | 3 | ~2,200 |
| Examples | 4 | ~1,800 |
| API Reference | 2 | ~3,400 |
| SDKs | 3 | ~2,100 |
| Architecture | 3 | ~4,200 |
| Diagrams | 4 | ~500 |
| Guides | 6 | ~3,200 |
| **Total** | **48** | **~26,000** |

## Quality Standards

All documentation must:
- ✅ Be written in English
- ✅ Include code examples where applicable
- ✅ Have proper markdown formatting
- ✅ Cross-reference related documents
- ✅ Include "Last Updated" dates for specs
- ✅ Follow the existing structure and style

## Contributing

See [DEVELOPMENT.md](specs/DEVELOPMENT.md) for contribution guidelines.

When adding documentation:
1. Review this organization guide
2. Place files in appropriate directories
3. Update INDEX.md
4. Maintain cross-references
5. Follow quality standards

