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
- The migrated data is written to a **new** directory; the original data
  directory is only read and stays your rollback
- Every snapshot file and the WAL are verified first; corrupt or unsupported
  input is rejected before anything is written

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

# Check disk space (the migrated copy needs about as much as the data)
df -h
```

3. **Check the layout:** the tool reads `./data/snapshots/` and
   `./data/wal/synap.wal`, the server's default layout. A snapshot is not
   required: the newest snapshot (if any) plus every WAL record written after
   it is what gets migrated, exactly what the server would recover.

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
  --output ./data-hub \
  --dry-run
```

**Review output:** the report lists the snapshot and WAL it read and every
structure, plus a `Note:` for anything the input format cannot carry (for
example key expiry in a snapshot written by 1.3.x):
```
Snapshot: ./data/snapshots/snapshot-v5-00000000000000000007.bin (format v5)
WAL: ./data/wal/synap.wal (format v2): 1204 records, 37 replayed after the snapshot
Read:
  KV keys:       800 (120 with expiry)
  ...
  queues:        25 (310 messages, 2 dead letters)
  streams:       10 (4000 events)
Dry run: nothing was written
```

### Step 5: Keep the Original Directory

The migration never writes to `./data`: it stays your rollback. If you want
an extra copy, take it with your usual backup tooling (for example
`cp -a ./data ./data-backup`).

### Step 6: Perform Migration

```bash
./target/release/synap-migrate migrate \
  --user-id YOUR_USER_ID \
  --data-dir ./data \
  --output ./data-hub
```

`--output` must not exist yet or be an empty directory, outside `./data`. If
the data comes from Synap 1.3.x and its server had custom queue settings, add
`--server-config synap.yaml` so those queues keep them (older snapshots do not
store them).

**Expected output:** the same report as the dry run, ending with
```
Wrote: ./data-hub/snapshots/snapshot-v5-00000000000000000001.bin
The source directory ./data is unchanged and stays the rollback. ...
```

### Step 7: Verify Migration

```bash
./target/release/synap-migrate validate \
  --user-id YOUR_USER_ID \
  --data-dir ./data-hub
```

**Check status:**
```bash
./target/release/synap-migrate status --data-dir ./data-hub
```

**Expected output:**
```
Status: MIGRATED
Total resources: 835
Scoped resources: 835 (100%)
Tenants: 1 (550e8400-e29b-41d4-a716-446655440000)
```

### Step 8: Configure Hub Integration

Update `synap.yaml`:

```yaml
# Server settings
server:
  host: "0.0.0.0"
  port: 15500

# The migrated data directory
persistence:
  enabled: true
  wal:
    path: "./data-hub/wal/synap.wal"
  snapshot:
    directory: "./data-hub/snapshots"

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

The output has no WAL yet; the server creates `./data-hub/wal/synap.wal` at
its first start.

### Step 9: Start Synap with Hub Integration

> **Note:** Hub request authentication is currently fail-closed: the HiveHub
> SDK cannot validate access-key secrets yet (hivellm/synap#230), so a server
> in Hub mode answers every request that carries an access key with 503 (and
> one without a key with 401) until that lands. `/health` and `/metrics` stay
> open.

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

The original data directory was never written to, so rolling back is a
configuration change.

### 1. Stop Synap Server

```bash
sudo systemctl stop synap-server
# OR
pkill synap-server
```

### 2. Restore Standalone Configuration

Point `synap.yaml` back at the original directory and disable Hub
integration:

```yaml
persistence:
  wal:
    path: "./data/wal/synap.wal"
  snapshot:
    directory: "./data/snapshots"

hub:
  enabled: false
```

### 3. Start Server

```bash
./synap-server --config synap.yaml
```

### 4. Remove the Migrated Copy

Once the standalone server is back, delete `./data-hub`. Anything written in
Hub mode after the migration lives only there.

### 5. Verify Rollback

```bash
./target/release/synap-migrate status --data-dir ./data
```

**Expected:**
```
Status: NOT MIGRATED
```

## Troubleshooting

### Issue: "the input already holds Hub-scoped names"

**Cause:** Resources already have `user_{user_id}:` prefixes: the directory
was migrated before.

**Solutions:**
1. Check if this is expected: `synap-migrate status --data-dir ./data`
2. If accidental: migrate from the original standalone directory instead

### Issue: "the output ... is not empty"

**Cause:** `--output` must be a new or empty directory.

**Solution:** choose a new path; the tool never overwrites existing files.

### Issue: "Snapshot ... is damaged: checksum mismatch" (or a WAL error)

**Cause:** a snapshot file or the WAL is corrupt, or was written by a newer
Synap. Nothing was written.

**Solutions:**
1. Restore the named file from a backup and re-run
2. For a damaged older snapshot, move it out of `snapshots/` if you accept
   losing what only it holds
3. For a newer format, use the migrator of that release

### Issue: "Validation failed: KV key \"xyz\" does not have the expected prefix"

**Cause:** the directory holds unscoped resources or another tenant's.

**Solutions:**
1. Check `synap-migrate status --data-dir ./data-hub`
2. Migrate again from the original directory into a new output
3. Report the issue with the status output

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

Keep the original standalone directory until the Hub deployment is verified,
and back up the new data directory with your usual backup tooling.

## Migration Checklist

- [ ] Synap server stopped
- [ ] New, empty output directory chosen
- [ ] User ID obtained from HiveHub
- [ ] Status check shows "NOT MIGRATED"
- [ ] Dry run successful (notes reviewed)
- [ ] Original data directory kept as the rollback
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
