---
title: Backup and Restore
module: operations
id: backup-restore
order: 5
description: Backup procedures and restore operations
tags: [operations, backup, restore, recovery]
---

# Backup and Restore

Complete guide to backing up and restoring Synap data.

## Backup Methods

### Snapshot Backup

Synap automatically creates snapshots:

```yaml
persistence:
  snapshot:
    enabled: true
    directory: "./data/snapshots"
    interval_secs: 3600
```

### Manual Snapshot

```bash
# Create snapshot manually
curl -X POST http://localhost:15500/snapshot
```

**Response:**
```json
{
  "success": true,
  "snapshot_path": "./data/snapshots/snapshot-v2-1234567890.bin"
}
```

### List Snapshots

```bash
curl http://localhost:15500/snapshots
```

**Response:**
```json
{
  "snapshots": [
    {
      "path": "snapshot-v2-1234567890.bin",
      "size_bytes": 1048576,
      "created_at": "2025-01-01T12:00:00Z"
    }
  ]
}
```

## Backup Procedures

### Full Backup

```bash
#!/bin/bash
# Backup script

BACKUP_DIR="/backup/synap"
DATE=$(date +%Y%m%d_%H%M%S)

# Create backup directory
mkdir -p "$BACKUP_DIR/$DATE"

# Copy snapshots
cp -r /data/snapshots "$BACKUP_DIR/$DATE/"

# Copy WAL
cp /data/wal/synap.wal "$BACKUP_DIR/$DATE/"

# Copy configuration
cp /etc/synap/config.yml "$BACKUP_DIR/$DATE/"

# Compress
tar czf "$BACKUP_DIR/synap_backup_$DATE.tar.gz" -C "$BACKUP_DIR/$DATE" .

echo "Backup completed: $BACKUP_DIR/synap_backup_$DATE.tar.gz"
```

### Automated Backup

```bash
# Add to crontab
0 2 * * * /path/to/backup.sh
```

### Docker Backup

```bash
# Backup data volume
docker run --rm \
  -v synap-data:/data \
  -v $(pwd)/backup:/backup \
  alpine tar czf /backup/synap_backup_$(date +%Y%m%d).tar.gz -C /data .
```

## Restore Procedures

### From Snapshot

1. Stop Synap server
2. Restore snapshot files
3. Restore WAL (if needed)
4. Start server (automatic recovery)

### Restore Steps

```bash
# 1. Stop server
systemctl stop synap

# 2. Backup current data (safety)
mv /data /data.backup

# 3. Restore snapshot
tar xzf backup/synap_backup_20250101.tar.gz -C /data

# 4. Start server (automatic recovery)
systemctl start synap
```

### Docker Restore

```bash
# Stop container
docker stop synap

# Restore data
docker run --rm \
  -v synap-data:/data \
  -v $(pwd)/backup:/backup \
  alpine tar xzf /backup/synap_backup_20250101.tar.gz -C /data

# Start container
docker start synap
```

## Disaster Recovery

### Full Disaster Recovery

1. **Install Synap** on new server
2. **Restore configuration** from backup
3. **Restore data** (snapshots + WAL)
4. **Start server** (automatic recovery)
5. **Verify** data integrity

### Recovery Time

- **1M keys**: ~1-5 seconds
- **10M keys**: ~10-30 seconds
- **100M keys**: ~1-5 minutes

## Backup Best Practices

### Regular Backups

- **Daily**: Full backup
- **Hourly**: Incremental (snapshots)
- **Before major changes**: Manual backup

### Backup Storage

- **Local**: Fast restore
- **Remote**: Disaster recovery
- **Multiple locations**: Redundancy

### Backup Verification

```bash
# Verify backup integrity
tar tzf backup/synap_backup_20250101.tar.gz

# Test restore on test server
```

## Related Topics

- [Persistence Configuration](../configuration/PERSISTENCE.md) - WAL and snapshots
- [Troubleshooting](./TROUBLESHOOTING.md) - Common problems
- [Service Management](./SERVICE_MANAGEMENT.md) - Service management

