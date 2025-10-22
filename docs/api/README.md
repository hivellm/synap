# Synap API Documentation

Complete OpenAPI 3.0 specification for the Synap REST API.

## 📄 Files

- **`openapi.yml`** (1,698 lines, 46KB) - YAML format (recommended for editing)
- **`openapi.json`** (2,543 lines, 65KB) - JSON format (for tools that require JSON)

## 🔗 View Documentation

### Online Viewers

**Swagger UI**:
```
https://petstore.swagger.io/?url=https://raw.githubusercontent.com/hivellm/synap/main/docs/api/openapi.yml
```

**Redoc**:
```
https://redocly.github.io/redoc/?url=https://raw.githubusercontent.com/hivellm/synap/main/docs/api/openapi.yml
```

### Local Viewers

**Using Docker (Swagger UI)**:
```bash
docker run -p 8080:8080 -e SWAGGER_JSON=/api/openapi.yml \
  -v $(pwd):/api swaggerapi/swagger-ui
```

**Using npx (Swagger UI)**:
```bash
npx @redocly/cli preview-docs docs/api/openapi.yml
```

**Using VS Code**:
Install the **OpenAPI (Swagger) Editor** extension and open `openapi.yml`.

## 📚 API Coverage

### 🔑 Key-Value Store (4 endpoints)
- `POST /kv/set` - Store key-value
- `GET /kv/get/{key}` - Retrieve value
- `DELETE /kv/del/{key}` - Delete key
- `GET /kv/stats` - Get statistics

### 📨 Message Queue (9 endpoints)
- `POST /queue/{name}` - Create queue
- `POST /queue/{name}/publish` - Publish message
- `GET /queue/{name}/consume/{consumer_id}` - Consume message
- `POST /queue/{name}/ack` - Acknowledge
- `POST /queue/{name}/nack` - Negative acknowledge
- `GET /queue/{name}/stats` - Queue statistics
- `POST /queue/{name}/purge` - Purge queue
- `DELETE /queue/{name}` - Delete queue
- `GET /queue/list` - List all queues

### 📡 Event Streams - Simple (6 endpoints)
- `POST /stream/{room}` - Create room
- `POST /stream/{room}/publish` - Publish event
- `GET /stream/{room}/consume/{subscriber_id}` - Consume events
- `GET /stream/{room}/stats` - Room statistics
- `DELETE /stream/{room}` - Delete room
- `GET /stream/list` - List rooms

### 🎯 Event Streams - Partitioned (5 endpoints)
Kafka-style partitioned topics:
- `GET /topics` - List all topics
- `POST /topics/{topic}` - Create partitioned topic
- `DELETE /topics/{topic}` - Delete topic
- `GET /topics/{topic}/stats` - Topic statistics
- `POST /topics/{topic}/publish` - Publish to topic
- `POST /topics/{topic}/partitions/{id}/consume` - Consume from partition

### 👥 Consumer Groups (9 endpoints)
Kafka-style consumer group coordination:
- `GET /consumer-groups` - List consumer groups
- `POST /consumer-groups/{group_id}` - Create consumer group
- `POST /consumer-groups/{group_id}/join` - Join group
- `DELETE /consumer-groups/{group_id}/members/{member_id}/leave` - Leave group
- `GET /consumer-groups/{group_id}/members/{member_id}/assignment` - Get partition assignment
- `POST /consumer-groups/{group_id}/members/{member_id}/heartbeat` - Send heartbeat
- `POST /consumer-groups/{group_id}/offsets/commit` - Commit offset
- `GET /consumer-groups/{group_id}/offsets/{partition_id}` - Get committed offset
- `GET /consumer-groups/{group_id}/stats` - Group statistics

### 🔔 Pub/Sub (4 endpoints)
- `POST /pubsub/{topic}/publish` - Publish message
- `GET /pubsub/stats` - System statistics
- `GET /pubsub/topics` - List topics
- `GET /pubsub/{topic}/info` - Topic info

### 💾 Persistence (1 endpoint)
- `POST /snapshot` - Trigger manual snapshot

### 🔒 Authentication

Three authentication methods supported:

**1. Basic Auth** (Redis-style):
```bash
curl -u admin:password http://localhost:15500/kv/stats
```

**2. Bearer Token** (API Key):
```bash
curl -H "Authorization: Bearer sk_abc123..." http://localhost:15500/queue/list
```

**3. API Key Query Parameter**:
```bash
curl http://localhost:15500/queue/list?api_key=sk_abc123...
```

## 🛠️ Code Generation

Generate client SDKs from the OpenAPI spec:

**TypeScript/JavaScript**:
```bash
npx @openapitools/openapi-generator-cli generate \
  -i docs/api/openapi.yml \
  -g typescript-axios \
  -o sdks/typescript
```

**Python**:
```bash
openapi-generator-cli generate \
  -i docs/api/openapi.yml \
  -g python \
  -o sdks/python
```

**Rust**:
```bash
openapi-generator-cli generate \
  -i docs/api/openapi.yml \
  -g rust \
  -o sdks/rust
```

**Go**:
```bash
openapi-generator-cli generate \
  -i docs/api/openapi.yml \
  -g go \
  -o sdks/go
```

## 📝 Validation

Validate the OpenAPI spec:

```bash
# Using Redocly CLI
npx @redocly/cli lint docs/api/openapi.yml

# Using Swagger CLI
npx swagger-cli validate docs/api/openapi.yml
```

## 🔄 Updating

When adding new endpoints:

1. Edit `openapi.yml` (YAML is easier to edit)
2. Regenerate JSON:
   ```bash
   python3 -c "import yaml, json; json.dump(yaml.safe_load(open('openapi.yml')), open('openapi.json', 'w'), indent=2)"
   ```
3. Validate changes
4. Commit both files

## 📖 Additional Documentation

- **[REST API Guide](REST_API.md)** - Detailed API usage guide
- **[StreamableHTTP Protocol](../protocol/STREAMABLE_HTTP.md)** - Alternative protocol
- **[Authentication Guide](../AUTHENTICATION.md)** - Security and auth details
- **[Examples](../examples/)** - Code examples and use cases

## 🔗 Resources

- **OpenAPI Specification**: https://spec.openapis.org/oas/v3.0.3
- **Swagger Tools**: https://swagger.io/tools/
- **Redocly**: https://redocly.com/
- **OpenAPI Generator**: https://openapi-generator.tech/

## 📊 Statistics

- **Total Endpoints**: 47+
- **API Categories**: 7
- **Request Schemas**: 15+
- **Response Schemas**: 20+
- **Authentication Schemes**: 3
- **Tags**: 8

---

**Version**: 0.3.0-rc  
**Format**: OpenAPI 3.0.3  
**Last Updated**: October 22, 2025

