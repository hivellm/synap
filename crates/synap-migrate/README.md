# Synap Migration Tool

Migration utility for converting standalone Synap installations to HiveHub.Cloud SaaS mode.

## Overview

The migration tool adds user namespace prefixes (`user_{user_id}:`) to all resources (queues, streams, KV keys) in your Synap data, enabling multi-tenant isolation for HiveHub.Cloud integration.

## Installation

Build the migration tool:

```bash
cargo build --release --bin synap-migrate
```

The binary will be available at `target/release/synap-migrate` (or `synap-migrate.exe` on Windows).

## Commands

### 1. Check Status

Check the migration status of your Synap installation:

```bash
synap-migrate status --data-dir ./data
```

**Output:**
- `NOT MIGRATED` - All resources are unscoped (standalone mode)
- `MIGRATED` - All resources have user prefixes (Hub mode)
- `PARTIALLY MIGRATED` - Mixed state (requires attention)

### 2. Backup Data

Create a backup before migration:

```bash
synap-migrate backup \
  --data-dir ./data \
  --output ./backup
```

**What it does:**
- Recursively copies all files from data directory
- Preserves directory structure
- Shows progress with file count and total size

### 3. Migrate to Hub Mode

Migrate standalone data to user-scoped namespaces:

```bash
synap-migrate migrate \
  --data-dir ./data \
  --user-id 550e8400-e29b-41d4-a716-446655440000 \
  --backup-dir ./backup
```

**Options:**
- `--data-dir` - Path to Synap data directory (default: `./data`)
- `--user-id` - UUID to assign ownership to all resources
- `--backup-dir` - Directory for automatic backup (default: `./backup`)
- `--dry-run` - Preview changes without modifying data

**What it does:**
1. Creates automatic backup
2. Reads latest snapshot
3. Adds `user_{user_id}:` prefix to all resource names
4. Writes new snapshot with prefixed resources
5. Archives old snapshot with `.pre-migration` extension
6. Validates migration

**Example:**

```bash
# Dry run first (recommended)
synap-migrate migrate \
  --user-id 550e8400-e29b-41d4-a716-446655440000 \
  --dry-run

# Perform actual migration
synap-migrate migrate \
  --user-id 550e8400-e29b-41d4-a716-446655440000
```

### 4. Validate Migration

Verify all resources have correct user prefixes:

```bash
synap-migrate validate \
  --data-dir ./data \
  --user-id 550e8400-e29b-41d4-a716-446655440000
```

**What it checks:**
- All KV keys start with `user_{user_id}:`
- All queue names start with `user_{user_id}:`
- All stream names start with `user_{user_id}:`

**Output:**
- Success: Reports count of validated resources
- Failure: Lists resources with incorrect prefixes

### 5. Rollback Migration

Restore data from backup:

```bash
synap-migrate rollback \
  --data-dir ./data \
  --backup-dir ./backup \
  --force
```

**Options:**
- `--force` - Required to confirm rollback operation

**What it does:**
1. Validates backup exists
2. Clears current data directory
3. Restores all files from backup
4. Preserves backup directory structure

**Warning:** This will overwrite all current data!

## Migration Workflow

### Step 1: Prepare

1. **Stop Synap server:**
   ```bash
   # Stop the running server
   systemctl stop synap-server
   # OR
   pkill synap-server
   ```

2. **Check current status:**
   ```bash
   synap-migrate status --data-dir ./data
   ```

### Step 2: Test with Dry Run

```bash
synap-migrate migrate \
  --user-id YOUR_USER_ID \
  --data-dir ./data \
  --dry-run
```

Review the output to see what will be migrated.

### Step 3: Perform Migration

```bash
synap-migrate migrate \
  --user-id YOUR_USER_ID \
  --data-dir ./data
```

The tool automatically:
- Creates backup at `./backup`
- Migrates all resources
- Validates migration
- Reports any errors

### Step 4: Update Configuration

Update your `synap.yaml` to enable Hub integration:

```yaml
hub:
  enabled: true
  service_api_key: "${HIVEHUB_SERVICE_API_KEY}"
  base_url: "https://api.hivehub.cloud"
```

### Step 5: Start Synap Server

```bash
# Set service API key
export HIVEHUB_SERVICE_API_KEY="sk_service_..."

# Start server with Hub integration
./synap-server --config synap.yaml
```

### Step 6: Verify

```bash
# Check server health
curl http://localhost:15500/health

# Test with Hub access key
curl -H "Authorization: Bearer sk_test_..." \
  http://localhost:15500/queue/list
```

## Rollback if Needed

If something goes wrong:

```bash
# Stop server
systemctl stop synap-server

# Rollback to backup
synap-migrate rollback \
  --data-dir ./data \
  --backup-dir ./backup \
  --force

# Start server in standalone mode (remove hub config)
./synap-server --config synap-standalone.yaml
```

## Examples

### Example 1: Simple Migration

```bash
# Single command migration
synap-migrate migrate \
  --user-id 550e8400-e29b-41d4-a716-446655440000
```

### Example 2: Custom Paths

```bash
# Specify custom data and backup directories
synap-migrate migrate \
  --user-id 550e8400-e29b-41d4-a716-446655440000 \
  --data-dir /var/lib/synap \
  --backup-dir /backup/synap
```

### Example 3: Verbose Logging

```bash
# Enable debug logging
synap-migrate --verbose migrate \
  --user-id 550e8400-e29b-41d4-a716-446655440000
```

### Example 4: Verify Before Production

```bash
# 1. Check status
synap-migrate status

# 2. Create manual backup
synap-migrate backup

# 3. Dry run
synap-migrate migrate --user-id $USER_ID --dry-run

# 4. Actual migration
synap-migrate migrate --user-id $USER_ID

# 5. Validate
synap-migrate validate --user-id $USER_ID
```

## Troubleshooting

### Issue: "No snapshot found"

**Cause:** Data directory is empty or no snapshots exist.

**Solution:**
- For fresh install: Migration creates empty snapshot automatically
- For existing install: Start Synap server once to create initial snapshot

### Issue: "Cannot migrate already-migrated data"

**Cause:** Resources already have `user_` prefixes.

**Solution:**
- Check status: `synap-migrate status`
- If accidentally migrated twice, use rollback
- Verify user_id is correct

### Issue: "Validation failed"

**Cause:** Some resources don't have correct user prefix.

**Solution:**
1. Check which resources failed (shown in error output)
2. Rollback: `synap-migrate rollback --force`
3. Investigate the issue
4. Re-run migration

### Issue: "Backup directory empty"

**Cause:** Backup was not created or was deleted.

**Solution:**
- Always run backup before migration
- Keep backups for at least 7 days
- Use `--backup-dir` to specify custom location

## Resource Naming Convention

After migration, all resources follow this pattern:

```
user_{user_id}:{resource_name}
```

**Examples:**

Before migration:
- Queue: `my-queue`
- Stream: `events`
- KV key: `config:app`

After migration (user_id = `550e8400e29b41d4a716446655440000`):
- Queue: `user_550e8400e29b41d4a716446655440000:my-queue`
- Stream: `user_550e8400e29b41d4a716446655440000:events`
- KV key: `user_550e8400e29b41d4a716446655440000:config:app`

## Technical Details

### What Gets Migrated?

- **KV Store:** All keys
- **Queues:** All queue names and messages
- **Streams:** All stream names and entries

### What Doesn't Change?

- Message content (unchanged)
- Timestamps (preserved)
- Message ordering (maintained)
- Data structure internals

### Snapshot Format

The tool reads and writes Synap's binary snapshot format (version 2):
- Header: Magic bytes + version + metadata
- KV data: Length-prefixed key-value pairs
- Queue data: Queue names + messages
- Stream data: Stream names + entries

### Safety Features

1. **Automatic Backup:** Creates backup before migration
2. **Dry Run Mode:** Preview changes without modifying data
3. **Validation:** Automatic post-migration validation
4. **Rollback:** Restore from backup if issues occur
5. **Archive:** Old snapshot saved as `.pre-migration`

## Performance

Migration performance depends on data size:

- **Small** (< 1000 resources): < 1 second
- **Medium** (1K-100K resources): 1-10 seconds
- **Large** (100K-1M resources): 10-60 seconds
- **Very Large** (> 1M resources): 1-5 minutes

Progress bar shows real-time migration status.

## See Also

- [HiveHub Configuration Guide](../docs/guides/HUB_CONFIGURATION.md)
- [Hub Integration Specification](../docs/specs/HUB_INTEGRATION.md)
- [Synap Documentation](../README.md)
