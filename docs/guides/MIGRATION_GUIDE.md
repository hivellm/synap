# Migration Guide: Standalone to HiveHub.Cloud

This guide walks you through migrating an existing standalone Synap installation to HiveHub.Cloud SaaS mode.

## Prerequisites

- [ ] Existing Synap standalone installation with data
- [ ] HiveHub.Cloud account created
- [ ] Service API key obtained from HiveHub dashboard
- [ ] Migration tool built: `cargo build --release --bin synap-migrate`
- [ ] **IMPORTANT:** Synap server must be stopped during migration

## Migration Overview

```
Standalone Mode          Migration Tool         Hub SaaS Mode
┌─────────────┐         ┌──────────────┐       ┌──────────────┐
│ Resources:  │         │ Add Prefixes │       │ Resources:   │
│ - my-queue  │  ────►  │ user_{id}:   │  ───► │ - user_...:  │
│ - my-stream │         │              │       │   my-queue   │
│ - my-key    │         │              │       │ - user_...:  │
└─────────────┘         └──────────────┘       │   my-stream  │
                                                │ - user_...:  │
                                                │   my-key     │
                                                └──────────────┘
```

**What happens:**
- All resources get prefixed with `user_{user_id}:`
- Original resource names preserved after prefix
- Data content remains unchanged
- Automatic backup created before migration

## Step-by-Step Migration

### Step 1: Prepare Environment

1. **Stop Synap server:**

```bash
# If using systemd
sudo systemctl stop synap-server

# If running manually
pkill synap-server

# Verify it's stopped
ps aux | grep synap-server
```

2. **Check data directory:**

```bash
# List data files
ls -lh ./data/

# Check disk space (ensure enough for backup)
df -h
```

3. **Verify snapshot exists:**

```bash
ls -lh ./data/snapshots/
```

If no snapshots exist:
```bash
# Start server briefly to create snapshot
./synap-server --config synap.yaml &
sleep 5
pkill synap-server
```

### Step 2: Get User ID from HiveHub

1. Log in to [HiveHub.Cloud Dashboard](https://hivehub.cloud)
2. Navigate to **Account Settings**
3. Copy your **User ID** (UUID format)

Example: `550e8400-e29b-41d4-a716-446655440000`

### Step 3: Check Current Status

```bash
./target/release/synap-migrate status --data-dir ./data
```

**Expected output:**
```
Status: NOT MIGRATED
Total resources: 1500
All resources are unscoped (standalone mode)
```

If you see `MIGRATED` or `PARTIALLY MIGRATED`, **STOP** - data is already migrated or in an inconsistent state.

### Step 4: Dry Run (Test Mode)

Perform a test run to preview changes:

```bash
./target/release/synap-migrate migrate \
  --user-id YOUR_USER_ID \
  --data-dir ./data \
  --dry-run
```

**Review output:**
```
Migration plan:
  - KV entries: 800
  - Queues: 25
  - Streams: 10

DRY RUN - No changes will be made
Dry-run completed successfully
```

### Step 5: Create Backup (Optional but Recommended)

While the migration tool creates an automatic backup, you may want a separate copy:

```bash
# Create timestamped backup
BACKUP_DIR="./backup-$(date +%Y%m%d-%H%M%S)"
./target/release/synap-migrate backup \
  --data-dir ./data \
  --output $BACKUP_DIR

echo "Backup created at: $BACKUP_DIR"
```

### Step 6: Perform Migration

```bash
./target/release/synap-migrate migrate \
  --user-id YOUR_USER_ID \
  --data-dir ./data \
  --backup-dir ./backup
```

**Expected output:**
```
INFO Starting migration for user 550e8400-e29b-41d4-a716-446655440000
INFO Creating backup before migration...
INFO Backup completed: 42 files, 12.5 MB
INFO Reading snapshot: "./data/snapshots/snapshot-v2-1701234567.bin"
INFO Migration plan:
  - KV entries: 800
  - Queues: 25
  - Streams: 10

[================================>   ] 800/835 Migrating stream data...

INFO Writing migrated snapshot to "./data/snapshots/snapshot-v2-1701234999.bin"
INFO Archiving old snapshot to ".../snapshot-v2-1701234567.bin.pre-migration"
INFO Migration completed successfully

INFO Validating migrated data...
INFO Validation successful:
  - 800 KV entries validated
  - 25 queues validated
  - 10 streams validated

INFO Validation passed. Migration successful!
```

### Step 7: Verify Migration

```bash
./target/release/synap-migrate validate \
  --user-id YOUR_USER_ID \
  --data-dir ./data
```

**Check status:**
```bash
./target/release/synap-migrate status --data-dir ./data
```

**Expected output:**
```
Status: MIGRATED
Total resources: 835
Scoped resources: 835 (100%)
Unique users: 1
User IDs: ["550e8400e29b41d4a716446655440000"]
```

### Step 8: Configure Hub Integration

Update `synap.yaml`:

```yaml
# Server settings
server:
  host: "0.0.0.0"
  port: 15500

# HiveHub.Cloud Integration
hub:
  enabled: true
  service_api_key: "${HIVEHUB_SERVICE_API_KEY}"
  base_url: "https://api.hivehub.cloud"

  auth:
    require_hub_auth: true
    allow_local_auth_fallback: false

  quota:
    cache_ttl_seconds: 60
    usage_report_interval: 300
```

### Step 9: Start Synap with Hub Integration

```bash
# Set service API key from HiveHub dashboard
export HIVEHUB_SERVICE_API_KEY="sk_service_a1b2c3d4..."

# Start Synap server
./synap-server --config synap.yaml
```

**Check logs for Hub integration:**
```
INFO synap_server::hub: HiveHub integration enabled
INFO synap_server::hub: Connected to HiveHub API: https://api.hivehub.cloud
INFO synap_server::hub: UsageReporter started (5-minute intervals)
```

### Step 10: Test with Hub Access Key

1. Create access key in HiveHub dashboard:
   - Navigate to **Access Keys**
   - Click **Create Access Key**
   - Copy the key: `sk_test_...`

2. Test API access:

```bash
export ACCESS_KEY="sk_test_..."

# List queues (should show migrated resources)
curl -H "Authorization: Bearer $ACCESS_KEY" \
  http://localhost:15500/queue/list

# Expected response:
["user_550e8400e29b41d4a716446655440000:my-queue", ...]
```

3. Test quota enforcement:

```bash
curl -H "Authorization: Bearer $ACCESS_KEY" \
  http://localhost:15500/hub/quota

# Expected response:
{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "plan": "Free",
  "quotas": {
    "storage": {"limit": 104857600, "used": 12500, ...},
    "operations": {"limit": 100000, "used": 42, ...}
  }
}
```

## Rollback Procedure

If you encounter issues, rollback to the backup:

### 1. Stop Synap Server

```bash
sudo systemctl stop synap-server
# OR
pkill synap-server
```

### 2. Rollback Migration

```bash
./target/release/synap-migrate rollback \
  --data-dir ./data \
  --backup-dir ./backup \
  --force
```

**Expected output:**
```
INFO Rolling back migration from "./backup"
INFO Backup found with 42 entries
WARN Removing existing data from "./data"
INFO Restoring from backup...
INFO Rollback completed: 42 files restored, 12.5 MB
```

### 3. Restore Standalone Configuration

Update `synap.yaml` to disable Hub integration:

```yaml
hub:
  enabled: false
```

### 4. Start Server

```bash
./synap-server --config synap.yaml
```

### 5. Verify Rollback

```bash
./target/release/synap-migrate status --data-dir ./data
```

**Expected:**
```
Status: NOT MIGRATED
```

## Troubleshooting

### Issue: "Cannot migrate already-migrated data"

**Cause:** Resources already have `user_` prefixes.

**Solutions:**
1. Check if this is expected: `synap-migrate status`
2. If accidental: Rollback and re-run migration
3. If intentional: No action needed

### Issue: "No snapshot found"

**Cause:** Synap hasn't created a snapshot yet.

**Solutions:**
1. Start Synap server briefly to create snapshot:
   ```bash
   ./synap-server --config synap.yaml &
   sleep 10
   pkill synap-server
   ```
2. Re-run migration

### Issue: "Validation failed: KV key 'xyz' does not have expected user prefix"

**Cause:** Some resources weren't migrated.

**Solutions:**
1. Rollback: `synap-migrate rollback --force`
2. Check for errors in migration logs
3. Report issue with snapshot details
4. Re-run migration

### Issue: "Hub integration failed to start"

**Cause:** Invalid or missing service API key.

**Solutions:**
1. Verify service API key is correct
2. Check HiveHub dashboard for key status
3. Ensure environment variable is set:
   ```bash
   echo $HIVEHUB_SERVICE_API_KEY
   ```
4. Test Hub API connectivity:
   ```bash
   curl -H "Authorization: Bearer $HIVEHUB_SERVICE_API_KEY" \
     https://api.hivehub.cloud/v1/health
   ```

### Issue: "Access key validation failed"

**Cause:** Access key not recognized by Hub.

**Solutions:**
1. Verify access key format: `sk_test_...` or `sk_live_...`
2. Check key is not revoked in HiveHub dashboard
3. Ensure service API key is correct
4. Wait 60 seconds for cache expiration
5. Create new access key if needed

## Post-Migration Tasks

### 1. Update Client Applications

Update your client code to use Hub access keys:

**Before (standalone):**
```bash
curl http://localhost:15500/queue/publish \
  -H "Content-Type: application/json" \
  -d '{"queue": "my-queue", "message": "Hello"}'
```

**After (Hub mode):**
```bash
curl http://localhost:15500/queue/publish \
  -H "Authorization: Bearer $ACCESS_KEY" \
  -H "Content-Type: application/json" \
  -d '{"queue": "my-queue", "message": "Hello"}'
```

### 2. Configure Monitoring

Set up monitoring for:
- Hub API connection status
- Quota usage (storage, operations)
- Failed authentication attempts
- Rate limit violations

See [HUB_CONFIGURATION.md](HUB_CONFIGURATION.md#configure-monitoring) for details.

### 3. Test Quota Limits

Deliberately test quota enforcement:
- Exceed storage quota
- Exceed operations quota
- Verify proper 429 responses

### 4. Plan for Scaling

If using cluster mode:
- Review [CLUSTER_INTEGRATION.md](../specs/CLUSTER_INTEGRATION.md)
- Configure distributed quota management
- Set up master-replica architecture

### 5. Keep Backups

Maintain backups for at least 30 days:
```bash
# Create weekly backups
0 0 * * 0 /path/to/synap-migrate backup \
  --data-dir /var/lib/synap \
  --output /backup/synap-$(date +\%Y\%m\%d)
```

## Migration Checklist

- [ ] Synap server stopped
- [ ] Snapshot exists
- [ ] User ID obtained from HiveHub
- [ ] Status check shows "NOT MIGRATED"
- [ ] Dry run successful
- [ ] Backup created
- [ ] Migration completed without errors
- [ ] Validation passed
- [ ] Hub configuration updated
- [ ] Service API key configured
- [ ] Server started successfully
- [ ] Access key created
- [ ] API tests passed
- [ ] Client applications updated
- [ ] Monitoring configured

## Support

If you encounter issues during migration:

1. **Check logs:** Review Synap server logs for errors
2. **Rollback:** Use the rollback procedure to restore
3. **Documentation:** Review [HUB_CONFIGURATION.md](HUB_CONFIGURATION.md)
4. **Community:** Join Discord at https://discord.gg/hivellm
5. **Support:** Email support@hivehub.cloud

## Next Steps

After successful migration:

- **Read:** [HUB_CONFIGURATION.md](HUB_CONFIGURATION.md) for advanced configuration
- **Learn:** [Hub Integration Specification](../specs/HUB_INTEGRATION.md) for technical details
- **Explore:** [CLUSTER_INTEGRATION.md](../specs/CLUSTER_INTEGRATION.md) for scaling
- **Monitor:** Set up alerts and dashboards for production
