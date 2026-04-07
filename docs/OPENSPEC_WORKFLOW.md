# OpenSpec Workflow Guide - Synap

> **Complete guide to using OpenSpec for spec-driven development in Synap**

## 📚 Table of Contents

1. [What is OpenSpec?](#what-is-openspec)
2. [Three-Stage Workflow](#three-stage-workflow)
3. [Quick Start Examples](#quick-start-examples)
4. [Detailed Workflows](#detailed-workflows)
5. [Best Practices](#best-practices)
6. [Troubleshooting](#troubleshooting)

---

## What is OpenSpec?

OpenSpec is a **spec-driven development framework** that helps you:

✅ **Plan before coding** - Write clear requirements and scenarios  
✅ **Track changes** - Manage feature proposals and implementations  
✅ **Maintain truth** - Keep specs in sync with actual code  
✅ **Collaborate** - Review proposals before implementation

### Key Concepts

**Specs (`openspec/specs/`)**: The **current truth** - what IS built
- Each capability has its own directory (e.g., `kv-store/`, `message-queue/`)
- Contains `spec.md` (requirements) and optional `design.md` (technical patterns)
- Reflects the deployed, production state

**Changes (`openspec/changes/`)**: **Proposals** - what SHOULD change
- Temporary workspace for planning new features
- Contains `proposal.md`, `tasks.md`, optional `design.md`, and spec deltas
- Moved to `archive/` after deployment

**Archive (`openspec/changes/archive/`)**: **Completed changes**
- Historical record of what was implemented and when
- Organized by date: `YYYY-MM-DD-change-name/`

---

## Three-Stage Workflow

```
┌─────────────────────────────────────────────────────────────┐
│  Stage 1: Creating Changes                                  │
│  ├─ Research context (openspec list, read specs)            │
│  ├─ Choose change-id (kebab-case, verb-led)                 │
│  ├─ Write proposal.md (why, what, impact)                   │
│  ├─ Write tasks.md (implementation checklist)               │
│  ├─ Write spec deltas (ADDED/MODIFIED/REMOVED)              │
│  └─ Validate (openspec validate --strict)                   │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  Stage 2: Implementing Changes                              │
│  ├─ Get approval on proposal                                │
│  ├─ Read proposal.md, design.md, tasks.md                   │
│  ├─ Implement tasks sequentially                            │
│  ├─ Update checklist as you complete items                  │
│  ├─ Run tests and quality checks                            │
│  └─ Mark all tasks [x] when complete                        │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  Stage 3: Archiving Changes                                 │
│  ├─ Verify all tasks completed                              │
│  ├─ Update specs/ with final state                          │
│  ├─ Move changes/X/ → changes/archive/YYYY-MM-DD-X/         │
│  ├─ Validate archived change                                │
│  └─ Commit to git                                           │
└─────────────────────────────────────────────────────────────┘
```

---

## Quick Start Examples

### Example 1: Adding a New Feature (Hash Data Structure)

#### Step 1: Research Context

```bash
# See what specs already exist
openspec list --specs

# Check for active changes
openspec list

# Review related specs
openspec show kv-store --type spec
```

#### Step 2: Create Proposal

```bash
# Create change directory
mkdir -p openspec/changes/add-hash-data-structure/specs/hash-operations

# Create proposal.md
cat > openspec/changes/add-hash-data-structure/proposal.md << 'EOF'
## Why

Synap currently lacks Hash data structures (field-value maps within a key), which are essential for:
- User profiles (field = attribute, value = data)
- Product catalogs (field = property, value = info)
- Configuration storage (field = setting, value = config)

Redis users rely heavily on Hashes (HSET, HGET, HGETALL, etc.), making this a critical gap for migration.

## What Changes

- **Add Hash Operations capability**: New module for hash commands
- Implement 15+ hash commands (HSET, HGET, HDEL, HMSET, HINCRBY, HSCAN, etc.)
- Storage: `HashMap<String, StoredValue>` within RadixMap (nested structure)
- REST API endpoints: POST /api/v1/hash/{set,get,mset,del,getall,incrby}
- StreamableHTTP commands: `hash.set`, `hash.get`, etc.
- MCP tools: `synap_hash_set`, `synap_hash_get`, `synap_hash_getall`
- Persistence integration: Hash changes append to OptimizedWAL
- Replication support: Hash operations replicated to slaves

**BREAKING**: None - purely additive feature

## Impact

- **Affected specs**: New capability `hash-operations`
- **Affected code**: 
  - `synap-server/src/core/` - New `hash.rs` module
  - `synap-server/src/server/handlers.rs` - New REST endpoints
  - `synap-server/src/persistence/` - WAL integration
  - `synap-server/src/replication/` - Replication protocol extension
- **Performance target**: <100µs for HSET/HGET operations
- **Test coverage**: 95%+ with 20+ unit tests
EOF

# Create tasks.md
cat > openspec/changes/add-hash-data-structure/tasks.md << 'EOF'
## 1. Core Implementation

- [ ] 1.1 Create `synap-server/src/core/hash.rs` module
- [ ] 1.2 Implement `HashStorage` struct with sharded HashMap
- [ ] 1.3 Implement HSET, HGET, HDEL (core 3 commands)
- [ ] 1.4 Implement HGETALL, HEXISTS, HKEYS, HVALS, HLEN
- [ ] 1.5 Implement HMSET, HMGET (batch operations)
- [ ] 1.6 Implement HINCRBY, HINCRBYFLOAT (atomic increment)
- [ ] 1.7 Implement HSETNX (conditional set)
- [ ] 1.8 Implement HSCAN (iteration with cursor)

## 2. API Layer

- [ ] 2.1 Add REST endpoints to `server/handlers.rs`
- [ ] 2.2 Add StreamableHTTP commands to `protocol/envelope.rs`
- [ ] 2.3 Implement MCP tools in `server/mcp_tools.rs`
- [ ] 2.4 Update OpenAPI spec with hash endpoints

## 3. Persistence & Replication

- [ ] 3.1 Add `HashOp` variants to `WalEntry` enum
- [ ] 3.2 Implement hash serialization for snapshots
- [ ] 3.3 Add hash operations to replication log
- [ ] 3.4 Test WAL recovery with hash data

## 4. Testing

- [ ] 4.1 Unit tests for all hash commands (20+ tests)
- [ ] 4.2 Concurrent access tests (100+ threads)
- [ ] 4.3 Persistence tests (WAL replay, snapshot recovery)
- [ ] 4.4 Replication tests (master-slave sync)
- [ ] 4.5 Performance benchmarks (<100µs target)
- [ ] 4.6 Integration tests with REST API
- [ ] 4.7 MCP tool tests

## 5. Documentation

- [ ] 5.1 API documentation in `docs/api/REST_API.md`
- [ ] 5.2 Usage examples in `docs/examples/HASH_USAGE.md`
- [ ] 5.3 Update `README.md` feature list
- [ ] 5.4 Update `CHANGELOG.md`

## 6. Quality Checks

- [ ] 6.1 Run `cargo +nightly fmt --all`
- [ ] 6.2 Run `cargo clippy -- -D warnings`
- [ ] 6.3 Verify 95%+ test coverage
- [ ] 6.4 All tests passing (cargo test --workspace)
- [ ] 6.5 Performance benchmarks meet targets
EOF

# Create spec delta
cat > openspec/changes/add-hash-data-structure/specs/hash-operations/spec.md << 'EOF'
## ADDED Requirements

### Requirement: Hash Set Operation

The system SHALL provide a hash set operation (HSET) that stores a field-value pair within a hash key.

#### Scenario: Set single field

- **WHEN** client calls HSET with key "user:1000", field "name", value "Alice"
- **THEN** the field is stored in the hash
- **AND** the operation returns 1 (field created)

#### Scenario: Update existing field

- **WHEN** client calls HSET with key "user:1000", field "name", value "Bob"
- **AND** the field "name" already exists
- **THEN** the field value is updated to "Bob"
- **AND** the operation returns 0 (field updated)

#### Scenario: Hash TTL applies to entire hash

- **WHEN** a hash has TTL set to 3600 seconds
- **AND** client performs HSET on any field
- **THEN** the TTL applies to the entire hash, not individual fields

---

### Requirement: Hash Get Operation

The system SHALL provide a hash get operation (HGET) that retrieves the value of a field from a hash.

#### Scenario: Get existing field

- **WHEN** client calls HGET with key "user:1000", field "name"
- **AND** the field exists with value "Alice"
- **THEN** the operation returns "Alice"

#### Scenario: Get non-existent field

- **WHEN** client calls HGET with key "user:1000", field "age"
- **AND** the field does not exist
- **THEN** the operation returns null/nil

#### Scenario: Get from non-existent hash

- **WHEN** client calls HGET with key "user:9999", field "name"
- **AND** the hash does not exist
- **THEN** the operation returns null/nil

---

### Requirement: Hash Delete Operation

The system SHALL provide a hash delete operation (HDEL) that removes one or more fields from a hash.

#### Scenario: Delete single field

- **WHEN** client calls HDEL with key "user:1000", field "email"
- **AND** the field exists
- **THEN** the field is removed
- **AND** the operation returns 1 (1 field deleted)

#### Scenario: Delete multiple fields

- **WHEN** client calls HDEL with key "user:1000", fields ["email", "phone", "address"]
- **AND** all fields exist
- **THEN** all fields are removed
- **AND** the operation returns 3 (3 fields deleted)

#### Scenario: Delete non-existent field

- **WHEN** client calls HDEL with key "user:1000", field "nonexistent"
- **THEN** the operation returns 0 (no fields deleted)

---

### Requirement: Hash Get All Operation

The system SHALL provide a hash get all operation (HGETALL) that retrieves all field-value pairs from a hash.

#### Scenario: Get all fields from hash

- **WHEN** client calls HGETALL with key "user:1000"
- **AND** the hash contains {"name": "Alice", "age": 30, "email": "alice@example.com"}
- **THEN** the operation returns all field-value pairs as a map

#### Scenario: Get all from empty hash

- **WHEN** client calls HGETALL with key "user:1000"
- **AND** the hash is empty (no fields)
- **THEN** the operation returns an empty map

#### Scenario: Get all from non-existent hash

- **WHEN** client calls HGETALL with key "user:9999"
- **AND** the hash does not exist
- **THEN** the operation returns an empty map

---

### Requirement: Hash Multi-Set Operation

The system SHALL provide a hash multi-set operation (HMSET) that sets multiple field-value pairs in a hash atomically.

#### Scenario: Set multiple fields atomically

- **WHEN** client calls HMSET with key "user:1000", fields {"name": "Alice", "age": 30, "email": "alice@example.com"}
- **THEN** all fields are set atomically
- **AND** the operation succeeds

#### Scenario: HMSET creates hash if not exists

- **WHEN** client calls HMSET with key "user:2000", fields {"name": "Bob"}
- **AND** the hash does not exist
- **THEN** a new hash is created with the field

---

### Requirement: Hash Increment Operation

The system SHALL provide a hash increment operation (HINCRBY) that atomically increments a numeric field value.

#### Scenario: Increment existing numeric field

- **WHEN** client calls HINCRBY with key "stats:user:1000", field "login_count", increment 1
- **AND** the current value is 42
- **THEN** the value is incremented to 43
- **AND** the operation returns 43

#### Scenario: Increment non-existent field initializes to zero

- **WHEN** client calls HINCRBY with key "stats:user:1000", field "new_count", increment 5
- **AND** the field does not exist
- **THEN** the field is initialized to 0 and incremented to 5
- **AND** the operation returns 5

#### Scenario: Negative increment (decrement)

- **WHEN** client calls HINCRBY with key "stats:user:1000", field "credits", increment -10
- **AND** the current value is 100
- **THEN** the value is decremented to 90

---

### Requirement: Hash Scan Operation

The system SHALL provide a hash scan operation (HSCAN) that iterates through fields in a hash using a cursor.

#### Scenario: Scan with cursor pagination

- **WHEN** client calls HSCAN with key "user:1000", cursor 0, count 10
- **AND** the hash has 50 fields
- **THEN** the operation returns:
  - New cursor (non-zero if more data)
  - Up to 10 field-value pairs

#### Scenario: Scan with pattern matching

- **WHEN** client calls HSCAN with key "config", cursor 0, pattern "db_*"
- **THEN** only fields matching the pattern are returned

#### Scenario: Scan complete returns cursor 0

- **WHEN** client calls HSCAN and all fields are returned
- **THEN** the returned cursor is 0 (iteration complete)

---

### Requirement: Hash Persistence

The system SHALL persist hash operations to the Write-Ahead Log (WAL) for durability.

#### Scenario: HSET written to WAL

- **WHEN** client performs HSET operation
- **THEN** a `HashSet` entry is appended to the WAL
- **AND** the entry contains key, field, and value

#### Scenario: Hash recovery from WAL

- **WHEN** server restarts after crash
- **THEN** hash operations are replayed from WAL
- **AND** hash state is reconstructed correctly

---

### Requirement: Hash Replication

The system SHALL replicate hash operations to slave nodes for high availability.

#### Scenario: HSET replicated to slave

- **WHEN** master executes HSET operation
- **THEN** a `HashSet` replication entry is sent to all connected slaves
- **AND** slaves apply the operation to their local state

#### Scenario: Hash state synced to new replica

- **WHEN** a new replica connects to master
- **THEN** existing hash data is included in full sync (snapshot transfer)
- **AND** subsequent hash operations are sent via partial sync

---

### Requirement: Hash Performance

The system SHALL achieve target performance metrics for hash operations.

#### Scenario: HSET latency target

- **WHEN** measuring HSET operation latency
- **THEN** p99 latency MUST be less than 100 microseconds

#### Scenario: HGET latency target

- **WHEN** measuring HGET operation latency
- **THEN** p99 latency MUST be less than 50 microseconds

#### Scenario: HGETALL latency for 100 fields

- **WHEN** measuring HGETALL with 100 fields
- **THEN** p99 latency MUST be less than 500 microseconds
EOF
```

#### Step 3: Validate Proposal

```bash
# Validate the change
openspec validate add-hash-data-structure --strict

# Review the diff
openspec diff add-hash-data-structure

# View complete proposal
openspec show add-hash-data-structure
```

#### Step 4: Get Approval (Before Implementation!)

- Share proposal with team
- Review requirements and scenarios
- Get sign-off from stakeholders
- **DO NOT START CODING** until approved

#### Step 5: Implement (Stage 2)

```bash
# Read the plan
openspec show add-hash-data-structure

# Implement tasks sequentially
# ... write code ...

# Update tasks.md as you complete items
# Change [ ] to [x] for completed tasks

# Run quality checks after each task
cargo +nightly fmt --all
cargo clippy -- -D warnings
cargo test --workspace
```

#### Step 6: Archive (Stage 3)

```bash
# After deployment and verification
openspec archive add-hash-data-structure --yes

# This moves:
# openspec/changes/add-hash-data-structure/
#   → openspec/changes/archive/2025-10-24-add-hash-data-structure/

# And updates:
# openspec/specs/hash-operations/ (new capability created)
```

---

### Example 2: Modifying Existing Feature (Update WAL Batching)

#### Creating the Proposal

```bash
mkdir -p openspec/changes/update-wal-batching/specs/persistence-layer

cat > openspec/changes/update-wal-batching/proposal.md << 'EOF'
## Why

Current WAL batching uses a fixed 100µs window, which can cause high latency spikes under low load (waiting for batch window to close even with 1 operation).

Redis uses adaptive batching: immediate flush for single ops, batching for bursts.

## What Changes

- Update `persistence/wal_optimized.rs` to use adaptive batching
- Immediate flush if batch queue is empty (no waiting)
- Batch flush if queue has pending ops or 100µs window expires
- Add metrics for batch size and flush latency

**BREAKING**: None - performance optimization only

## Impact

- **Affected specs**: `persistence-layer` (MODIFIED)
- **Affected code**: `synap-server/src/persistence/wal_optimized.rs`
- **Performance**: Improved p99 latency under low load (1ms → 100µs)
EOF

# Create spec delta with MODIFIED requirement
cat > openspec/changes/update-wal-batching/specs/persistence-layer/spec.md << 'EOF'
## MODIFIED Requirements

### Requirement: WAL Batching Strategy

The system SHALL use adaptive batching for Write-Ahead Log (WAL) writes to optimize both latency and throughput.

#### Scenario: Immediate flush for single operation

- **WHEN** a write operation arrives
- **AND** the batch queue is empty
- **THEN** the operation is flushed immediately without waiting

#### Scenario: Batching during burst

- **WHEN** multiple write operations arrive within 100µs
- **THEN** operations are batched together
- **AND** flushed when batch size reaches 10K ops OR 100µs window expires

#### Scenario: Metrics for batch monitoring

- **WHEN** WAL performs a flush
- **THEN** metrics are recorded for:
  - Batch size (number of operations)
  - Flush latency (time to fsync)
  - Flush rate (flushes per second)
EOF
```

**Note**: For MODIFIED, you MUST include the complete updated requirement with ALL scenarios (old + new). The archiver will replace the entire requirement.

---

## Detailed Workflows

### Decision Tree: Do I Need a Proposal?

```
┌─────────────────────────────────────────┐
│ What are you doing?                     │
└─────────────────────────────────────────┘
              ↓
    ┌─────────┴─────────┐
    │                   │
    ▼                   ▼
┌─────────┐       ┌─────────┐
│ Simple  │       │ Complex │
│ Change  │       │ Change  │
└─────────┘       └─────────┘
    │                   │
    │                   │
    ▼                   ▼
┌─────────────────────────────────────────┐
│ Simple (No Proposal Needed):            │
│ - Bug fix (restoring spec behavior)    │
│ - Typo/formatting                       │
│ - Non-breaking dependency update        │
│ - Config change (no behavior change)    │
│ - Test for existing behavior            │
└─────────────────────────────────────────┘
    │
    └─→ Fix directly, commit
    
┌─────────────────────────────────────────┐
│ Complex (Proposal Required):            │
│ - New feature/capability                │
│ - Breaking change (API, schema)         │
│ - Architecture change                   │
│ - Performance optimization (changes     │
│   behavior or contracts)                │
│ - Security pattern change               │
└─────────────────────────────────────────┘
    │
    └─→ Create OpenSpec proposal
```

### File Structure Template

```
openspec/changes/my-change-id/
├── proposal.md           # REQUIRED: Why, What, Impact
├── tasks.md             # REQUIRED: Implementation checklist
├── design.md            # OPTIONAL: See criteria below
└── specs/               # REQUIRED: At least one delta
    └── capability-name/
        └── spec.md      # Delta: ADDED/MODIFIED/REMOVED
```

**When to create `design.md`**:
- ✅ Cross-cutting change (multiple services/modules)
- ✅ New architectural pattern
- ✅ New external dependency
- ✅ Significant data model changes
- ✅ Security/performance/migration complexity
- ❌ Simple single-module feature
- ❌ Straightforward implementation

### Spec Delta Operations

#### ADDED (New Requirements)

Use when introducing completely new capabilities.

```markdown
## ADDED Requirements

### Requirement: New Feature Name

The system SHALL provide new functionality.

#### Scenario: Success case

- **WHEN** condition occurs
- **THEN** expected result
```

#### MODIFIED (Changed Requirements)

Use when changing existing behavior. **CRITICAL**: Paste the COMPLETE updated requirement (header + all scenarios).

```markdown
## MODIFIED Requirements

### Requirement: Existing Feature Name

The system SHALL provide updated functionality with new behavior.

#### Scenario: Original scenario (if unchanged)

- **WHEN** original condition
- **THEN** original result

#### Scenario: New scenario added

- **WHEN** new condition
- **THEN** new result
```

**Common Mistake**: Only pasting the new parts. This causes loss of detail at archive time. Always paste the full requirement.

#### REMOVED (Deprecated Requirements)

Use when removing features.

```markdown
## REMOVED Requirements

### Requirement: Old Feature Name

**Reason**: Why this is being removed

**Migration**: How users should migrate to alternative
```

#### RENAMED (Name Changes)

Use when only the requirement name changes.

```markdown
## RENAMED Requirements

- FROM: `### Requirement: Old Name`
- TO: `### Requirement: New Name`
```

If behavior also changes, use RENAMED + MODIFIED (referencing new name).

---

## Best Practices

### 1. Change Naming

**Good Examples**:
- `add-hash-data-structure` (verb-led, clear)
- `update-wal-batching-logic` (specific action)
- `remove-deprecated-rest-endpoints` (explicit)
- `refactor-storage-layer` (scope defined)

**Bad Examples**:
- `hashes` (no verb, unclear)
- `improve-performance` (too vague)
- `fix-bug-123` (use issue tracker, not OpenSpec)
- `new-feature` (what feature?)

### 2. Writing Scenarios

**Correct Format** (use `#### Scenario:`):
```markdown
#### Scenario: User login success

- **WHEN** valid credentials provided
- **THEN** JWT token returned
- **AND** user session created
```

**Incorrect Formats** (will break validation):
```markdown
❌ - **Scenario: User login**        # Bullet point
❌ **Scenario**: User login          # Bold, no heading
❌ ### Scenario: User login          # Wrong heading level
❌ Scenario: User login (plain text) # No heading marker
```

### 3. Requirement Wording

Use normative language:
- **SHALL** / **MUST**: Mandatory requirement
- **SHOULD**: Recommended but not mandatory
- **MAY**: Optional

**Examples**:
```markdown
✅ The system SHALL persist all write operations to WAL
✅ The system MUST return within 100ms
⚠️  The system SHOULD log errors (not enforced)
⚠️  The system MAY compress responses (optional)
```

### 4. Validation Workflow

```bash
# Always validate with --strict before sharing
openspec validate my-change --strict

# Check specific issues
openspec show my-change --json --deltas-only | jq '.deltas'

# Review diff before implementation
openspec diff my-change

# Validate all active changes
openspec validate --strict
```

### 5. Simplicity Guidelines

From `openspec/project.md`:

**Default to Simple**:
- <100 lines of new code per change
- Single-file implementations until proven insufficient
- Avoid frameworks without clear justification
- Choose boring, proven patterns

**Only Add Complexity When**:
- Performance data shows current solution too slow
- Concrete scale requirements (>1000 users, >100MB data)
- Multiple proven use cases requiring abstraction

---

## Troubleshooting

### Error: "Change must have at least one delta"

**Cause**: No spec delta files found

**Solution**:
```bash
# Check structure
ls -la openspec/changes/my-change/specs/

# Should have at least one:
openspec/changes/my-change/specs/capability-name/spec.md
```

### Error: "Requirement must have at least one scenario"

**Cause**: Scenarios not formatted correctly

**Solution**: Use exact format `#### Scenario: Name`

```markdown
✅ #### Scenario: Success case
❌ ### Scenario: Success case   # Wrong level
❌ - Scenario: Success case     # Bullet point
❌ **Scenario**: Success case   # Not a heading
```

### Error: "Silent scenario parsing failures"

**Cause**: Whitespace or formatting issues

**Debug**:
```bash
# Check JSON output
openspec show my-change --json --deltas-only

# Look for scenarios array (should not be empty)
openspec show my-change --json | jq '.deltas[].added[].scenarios'
```

### Error: "Validation fails after archive"

**Cause**: Incomplete MODIFIED requirement (missing old content)

**Solution**: When using MODIFIED, paste the COMPLETE requirement with ALL scenarios

```markdown
## MODIFIED Requirements

### Requirement: Feature X

[PASTE ENTIRE REQUIREMENT HERE - OLD + NEW CONTENT]

#### Scenario: Original scenario 1
...

#### Scenario: Original scenario 2
...

#### Scenario: New scenario 3 (the one you're adding)
...
```

---

## Cheat Sheet

### Essential Commands

```bash
# Context & Discovery
openspec list --specs         # What capabilities exist?
openspec list                 # What changes are active?
openspec show [item]          # View details

# Creating Changes
mkdir -p openspec/changes/add-feature/specs/capability
# ... create proposal.md, tasks.md, spec.md ...
openspec validate add-feature --strict

# Implementing
openspec show add-feature     # Read the plan
# ... implement tasks ...
# ... update tasks.md [x] ...

# Archiving
openspec archive add-feature --yes
openspec validate --strict    # Verify archive
```

### Quick Templates

**Minimal Proposal**:
```markdown
## Why
[1-2 sentences]

## What Changes
- [Change 1]
- [Change 2]

## Impact
- Affected specs: [list]
- Affected code: [list]
```

**Minimal Tasks**:
```markdown
## 1. Implementation
- [ ] 1.1 Task one
- [ ] 1.2 Task two

## 2. Testing
- [ ] 2.1 Unit tests
- [ ] 2.2 Integration tests

## 3. Documentation
- [ ] 3.1 Update docs
```

**Minimal Spec Delta**:
```markdown
## ADDED Requirements

### Requirement: Feature Name

The system SHALL do something.

#### Scenario: Success case

- **WHEN** condition
- **THEN** result
```

---

## Next Steps

1. **Read `openspec/project.md`** to understand Synap-specific context
2. **Review existing specs** with `openspec list --specs`
3. **Try creating a proposal** for a simple feature
4. **Validate and iterate** until clean

**Questions?** Check `openspec/AGENTS.md` for complete reference or ask the team!

---

**Last Updated**: October 24, 2025  
**Version**: 1.0  
**Related Docs**: `openspec/AGENTS.md`, `openspec/project.md`

