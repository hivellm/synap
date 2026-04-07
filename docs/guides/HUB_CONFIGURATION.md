# HiveHub.Cloud Configuration Guide

This guide walks you through configuring Synap to integrate with HiveHub.Cloud for multi-tenant SaaS deployment.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Getting Your Service API Key](#getting-your-service-api-key)
3. [Configuration](#configuration)
4. [Starting the Server](#starting-the-server)
5. [Verifying the Integration](#verifying-the-integration)
6. [Testing with Access Keys](#testing-with-access-keys)
7. [Troubleshooting](#troubleshooting)
8. [Next Steps](#next-steps)

## Prerequisites

Before integrating with HiveHub.Cloud, ensure you have:

- [ ] **Synap server** compiled with `hub-integration` feature:
  ```bash
  cargo build --release --features hub-integration
  ```

- [ ] **HiveHub.Cloud account** (sign up at https://hivehub.cloud)

- [ ] **Service deployed** on HiveHub.Cloud platform

- [ ] **Admin access** to your HiveHub.Cloud dashboard

## Getting Your Service API Key

Your Service API Key allows Synap to communicate with HiveHub.Cloud to validate user access keys and manage quotas.

### Step 1: Log in to HiveHub.Cloud

Visit https://hivehub.cloud and log in to your account.

### Step 2: Navigate to Service Settings

1. Go to your **Dashboard**
2. Select your **Synap service**
3. Click on **Settings** → **API Keys**

### Step 3: Create Service API Key

1. Click **"Generate Service API Key"**
2. Copy the key immediately (shown only once)
3. Store it securely (password manager or secrets vault)

**Format**: `sk_service_<64_hex_chars>`

**Example**: `sk_service_a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd`

**Important**: Keep this key secret - it grants full access to your service configuration!

## Configuration

### Option 1: Environment Variables (Recommended for Production)

Set the service API key as an environment variable:

**Linux/macOS**:
```bash
export HIVEHUB_SERVICE_API_KEY="sk_service_a1b2c3d4..."
```

**Windows (PowerShell)**:
```powershell
$env:HIVEHUB_SERVICE_API_KEY="sk_service_a1b2c3d4..."
```

**Windows (Command Prompt)**:
```cmd
set HIVEHUB_SERVICE_API_KEY=sk_service_a1b2c3d4...
```

### Option 2: Configuration File

Create or update your `synap.yaml` configuration file:

```yaml
# synap.yaml

# Server settings
server:
  host: "0.0.0.0"
  port: 15500

# HiveHub.Cloud Integration
hub:
  # Enable Hub integration
  enabled: true

  # Service API key (use environment variable in production)
  service_api_key: "${HIVEHUB_SERVICE_API_KEY}"

  # Hub API base URL
  base_url: "https://api.hivehub.cloud"

  # Access key validation settings
  access_key:
    cache_ttl_seconds: 60        # Cache validated keys for 60 seconds
    cache_max_entries: 10000     # Maximum cached keys in memory

  # Authentication settings
  auth:
    require_hub_auth: true       # Require Hub access keys
    allow_local_auth_fallback: false  # Disable local auth (Hub-only mode)

  # Quota settings
  quota:
    cache_ttl_seconds: 60        # Cache quota data for 60 seconds
    usage_report_interval: 300   # Report usage every 5 minutes

  # Usage tracking
  usage:
    buffer_size: 10000           # Max metrics in memory before flush
    flush_interval_seconds: 300  # Force flush every 5 minutes
```

### Configuration Options Explained

#### `hub.enabled`
- **Type**: Boolean
- **Default**: `false`
- **Description**: Enable HiveHub.Cloud integration. Set to `true` for SaaS mode.

#### `hub.service_api_key`
- **Type**: String (secret)
- **Required**: Yes (when `hub.enabled = true`)
- **Description**: Service API key from HiveHub.Cloud. Always use environment variable in production.

#### `hub.base_url`
- **Type**: String (URL)
- **Default**: `https://api.hivehub.cloud`
- **Description**: HiveHub.Cloud API endpoint. Change only for custom deployments.

#### `hub.access_key.cache_ttl_seconds`
- **Type**: Integer
- **Default**: `60`
- **Description**: How long to cache access key validations. Higher values reduce Hub API calls but increase revocation latency.

#### `hub.auth.require_hub_auth`
- **Type**: Boolean
- **Default**: `true` (when Hub enabled)
- **Description**: Require Hub access keys for all requests.

#### `hub.auth.allow_local_auth_fallback`
- **Type**: Boolean
- **Default**: `false`
- **Description**: Allow local API keys as fallback. Set to `true` for hybrid mode during migration.

## Starting the Server

### With Environment Variable

```bash
# Set service API key
export HIVEHUB_SERVICE_API_KEY="sk_service_a1b2c3d4..."

# Start Synap server
./synap-server --config synap.yaml
```

### With Docker

```bash
docker run -d \
  --name synap-server \
  -p 15500:15500 \
  -e HIVEHUB_SERVICE_API_KEY="sk_service_a1b2c3d4..." \
  -v $(pwd)/synap.yaml:/etc/synap/synap.yaml \
  synap:latest
```

### With Docker Compose

```yaml
# docker-compose.yml
version: '3.8'

services:
  synap:
    image: synap:latest
    ports:
      - "15500:15500"
    environment:
      - HIVEHUB_SERVICE_API_KEY=${HIVEHUB_SERVICE_API_KEY}
    volumes:
      - ./synap.yaml:/etc/synap/synap.yaml
      - synap-data:/var/lib/synap
    restart: unless-stopped

volumes:
  synap-data:
```

Start with:
```bash
export HIVEHUB_SERVICE_API_KEY="sk_service_a1b2c3d4..."
docker-compose up -d
```

## Verifying the Integration

### Step 1: Check Server Logs

Look for Hub integration initialization messages:

```
INFO synap_server::hub: HiveHub integration enabled
INFO synap_server::hub: Connected to HiveHub API: https://api.hivehub.cloud
INFO synap_server::hub: UsageReporter started (5-minute intervals)
```

### Step 2: Test Hub API Connectivity

```bash
# Using curl
curl -X GET \
  -H "Authorization: Bearer $HIVEHUB_SERVICE_API_KEY" \
  https://api.hivehub.cloud/v1/health

# Expected response:
{
  "status": "healthy",
  "service": "HiveHub API",
  "version": "1.0.0"
}
```

### Step 3: Check Server Health

```bash
# Synap health endpoint
curl http://localhost:15500/health

# Expected response:
{
  "status": "healthy",
  "hub_integration": "enabled",
  "hub_connected": true
}
```

## Testing with Access Keys

### Step 1: Create a Test User Access Key

1. Log in to HiveHub.Cloud dashboard
2. Navigate to **Access Keys**
3. Click **"Create Access Key"**
4. Select environment: `test`
5. Copy the access key: `sk_test_<64_hex_chars>`

### Step 2: Test Authentication

**Using Bearer Token**:
```bash
export ACCESS_KEY="sk_test_a1b2c3d4..."

curl -X POST http://localhost:15500/queue/publish \
  -H "Authorization: Bearer $ACCESS_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "queue": "test-queue",
    "message": "Hello Hub!"
  }'
```

**Using Custom Header**:
```bash
curl -X POST http://localhost:15500/queue/publish \
  -H "X-Hub-Access-Key: $ACCESS_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "queue": "test-queue",
    "message": "Hello Hub!"
  }'
```

**Expected Response** (success):
```json
{
  "status": "published",
  "queue": "test-queue",
  "message_id": "msg_12345..."
}
```

### Step 3: Test Multi-Tenant Isolation

Create two test users and verify resource isolation:

**User 1** (sk_test_user1...):
```bash
# Publish to queue
curl -X POST http://localhost:15500/queue/publish \
  -H "Authorization: Bearer sk_test_user1..." \
  -d '{"queue": "my-queue", "message": "User 1 message"}'

# List queues (should only see user1's queues)
curl -X GET http://localhost:15500/queue/list \
  -H "Authorization: Bearer sk_test_user1..."

# Expected: ["user_<user1_id>:my-queue"]
```

**User 2** (sk_test_user2...):
```bash
# List queues (should NOT see user1's queues)
curl -X GET http://localhost:15500/queue/list \
  -H "Authorization: Bearer sk_test_user2..."

# Expected: [] (empty, user2 has no queues yet)
```

### Step 4: Test Quota Enforcement

Check current quota usage:

```bash
curl -X GET http://localhost:15500/hub/quota \
  -H "Authorization: Bearer $ACCESS_KEY"

# Response:
{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "plan": "Free",
  "quotas": {
    "storage": {
      "limit": 104857600,
      "used": 1024,
      "available": 104856576,
      "percentage": 0.001
    },
    "operations": {
      "limit": 100000,
      "used": 15,
      "available": 99985,
      "percentage": 0.015
    }
  }
}
```

## Troubleshooting

### Issue: "Hub integration failed to start"

**Symptom**: Server fails to start with Hub error

**Possible Causes**:

1. **Missing service API key**
   ```
   Error: HIVEHUB_SERVICE_API_KEY environment variable not set
   ```

   **Solution**: Set the environment variable or add to config file

2. **Invalid service API key**
   ```
   Error: Failed to authenticate with Hub API: 401 Unauthorized
   ```

   **Solution**: Verify the service API key is correct and not expired

3. **Hub API unreachable**
   ```
   Error: Failed to connect to Hub API: Connection timeout
   ```

   **Solution**: Check network connectivity, firewall rules, DNS resolution

### Issue: "Access key validation failed"

**Symptom**: All requests return `401 Unauthorized`

**Possible Causes**:

1. **Invalid access key format**
   ```
   Error: Invalid access key format
   ```

   **Solution**: Verify key starts with `sk_test_` or `sk_live_` and has 71 characters

2. **Access key revoked**
   ```
   Error: Access key has been revoked
   ```

   **Solution**: Create a new access key via Hub dashboard

3. **Cache issues**
   ```
   Error: Failed to validate access key
   ```

   **Solution**:
   - Wait 60 seconds for cache expiration
   - Restart Synap server to clear cache
   - Verify Hub API is accessible

### Issue: "Quota exceeded unexpectedly"

**Symptom**: Requests return `429 Too Many Requests` despite low usage

**Possible Causes**:

1. **Stale quota cache**

   **Solution**: Wait 60 seconds for quota cache to refresh

2. **Multiple servers using same user**

   **Solution**: Review active connections and scale quota

3. **Plan limits reached**

   **Solution**: Upgrade plan via Hub dashboard

### Issue: "Multi-tenant isolation not working"

**Symptom**: Users can see each other's resources

**Possible Causes**:

1. **Hub integration not enabled**

   **Solution**: Verify `hub.enabled: true` in configuration

2. **Using same access key**

   **Solution**: Ensure each user has their own access key

3. **Local auth fallback active**

   **Solution**: Set `hub.auth.allow_local_auth_fallback: false`

### Debug Commands

**Check Hub connection status**:
```bash
# View server logs
journalctl -u synap-server -f

# Look for:
# - "HiveHub integration enabled"
# - "Connected to HiveHub API"
# - "UsageReporter started"
```

**Test Hub API directly**:
```bash
# Test service API key
curl -X GET \
  -H "Authorization: Bearer $HIVEHUB_SERVICE_API_KEY" \
  https://api.hivehub.cloud/v1/services/self

# Test user access key validation
curl -X POST \
  -H "Authorization: Bearer $HIVEHUB_SERVICE_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"access_key": "sk_test_..."}' \
  https://api.hivehub.cloud/v1/access-keys/validate
```

**Check quota cache**:
```bash
# Monitor quota requests
tail -f synap-server.log | grep "quota"
```

## Next Steps

### For Development

1. **Use test environment**:
   - Create test access keys (`sk_test_...`)
   - Lower rate limits for testing
   - Shorter TTLs for rapid iteration

2. **Test quota limits**:
   - Deliberately exceed quotas
   - Verify error responses
   - Test upgrade flows

3. **Test multi-tenancy**:
   - Create multiple test users
   - Verify resource isolation
   - Test permission enforcement

### For Production

1. **Security checklist**:
   - [ ] Service API key stored in secrets manager
   - [ ] HTTPS enabled (TLS 1.3)
   - [ ] Firewall rules configured
   - [ ] Monitoring alerts set up
   - [ ] Log rotation configured

2. **Configure monitoring**:
   ```yaml
   # Add to synap.yaml
   monitoring:
     metrics_enabled: true
     prometheus_endpoint: "/metrics"

   logging:
     level: "info"
     format: "json"
     output: "/var/log/synap/synap.log"
   ```

3. **Set up alerting**:
   - Hub API connection failures
   - High quota usage (>80%)
   - Failed authentication attempts
   - Rate limit violations

4. **Configure backups**:
   ```bash
   # Backup Synap data directory
   rsync -av /var/lib/synap/ /backup/synap/
   ```

5. **Plan for scaling**:
   - Multiple Synap instances (cluster mode - Phase 7)
   - Load balancer configuration
   - Distributed quota management

### Migration from Standalone

If you're migrating from standalone mode:

1. **Enable hybrid mode first**:
   ```yaml
   hub:
     enabled: true
     auth:
       allow_local_auth_fallback: true
   ```

2. **Gradually migrate users**:
   - Create Hub access keys for existing users
   - Update client applications incrementally
   - Monitor both auth methods

3. **Disable local auth when ready**:
   ```yaml
   hub:
     auth:
       allow_local_auth_fallback: false
   ```

### Learn More

- **[HUB_INTEGRATION.md](../specs/HUB_INTEGRATION.md)** - Technical specification
- **[QUOTA_MANAGEMENT.md](../specs/QUOTA_MANAGEMENT.md)** - Quota system details
- **[ACCESS_KEYS.md](../specs/ACCESS_KEYS.md)** - Access key authentication
- **[AUTHENTICATION.md](../AUTHENTICATION.md)** - Authentication overview

### Get Help

- **HiveHub.Cloud Documentation**: https://docs.hivehub.cloud
- **Support**: support@hivehub.cloud
- **GitHub Issues**: https://github.com/hivellm/synap/issues
- **Community Discord**: https://discord.gg/hivellm

## Summary

You've successfully configured Synap to integrate with HiveHub.Cloud! Your server now supports:

✅ **Multi-tenant isolation** - Users can only access their own resources
✅ **Plan-based quotas** - Storage, operations, and rate limits enforced
✅ **Access key authentication** - Secure, revocable user credentials
✅ **Usage tracking** - Automatic reporting to HiveHub API
✅ **Rate limiting** - Per-user, Plan-based request limits

Happy building! 🚀
