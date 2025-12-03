# Fix REST API Request Formats

> **Status**: 🔄 **IN PROGRESS** (90% Complete)  
> **Priority**: Medium  
> **Target**: v0.8.2  
> **Duration**: 1-2 days  
> **Last Updated**: November 25, 2025

## Why

During comprehensive REST API testing, we identified ~10 endpoints (12% of total) that are failing due to request body format mismatches. While the server implementation is correct, the request formats expected by the handlers don't match the formats being sent by clients. This creates confusion and prevents proper API usage.

**Current Status**: 88% of routes working correctly, but format inconsistencies prevent full API utilization.

## What Changes

Fix request body format mismatches for the following endpoints:

### 1. String Extensions
- **`POST /kv/msetnx`**: Currently expects tuple format `(key, value)`, but clients send object format `{"key": "...", "value": "..."}`
  - **Fix**: Update handler to accept both formats or standardize on object format

### 2. Hash Operations
- **`POST /hash/{key}/mset`**: Currently expects `HashMap<String, serde_json::Value>`, but clients send array format
  - **Fix**: Update handler to accept array of field-value pairs or update documentation

### 3. List Operations
- **`POST /list/{key}/lpop`** and **`POST /list/{key}/rpop`**: Currently require body with `count` field, but should be optional
  - **Fix**: Make count parameter optional (default to 1) or allow empty body

### 4. Sorted Set Operations
- **`POST /sortedset/{key}/zadd`**: Currently expects single `member` and `score` fields, but clients send array of members
  - **Fix**: Support both single member and array of members format (like Redis ZADD)

### 5. Pub/Sub Operations
- **`POST /pubsub/{topic}/publish`**: Currently expects `payload` field, but clients send `data` field
  - **Fix**: Accept both `payload` and `data` fields, or standardize on one

### 6. Geospatial Operations
- **`POST /geospatial/{key}/geoadd`**: Request format needs verification and potential adjustment
  - **Fix**: Verify and document correct format, update handler if needed

### 7. Stream Operations
- **`POST /stream/{room}`**: Room creation format may need adjustment
  - **Fix**: Verify format and update handler/documentation

### 8. Memory Usage
- **`GET /memory/{key}/usage`**: Returns 404 when key doesn't exist (expected behavior, but should return 0 or empty response)
  - **Fix**: Return appropriate response for non-existent keys

### 9. StreamableHTTP
- **`POST /api/v1/command`**: Request format needs verification
  - **Fix**: Verify and document correct StreamableHTTP envelope format

## Impact

**Affected Specs**: 
- `docs/api/REST_API.md` - Update request format documentation
- `docs/api/openapi.yml` - Update OpenAPI schema definitions

**Affected Code**:
- `synap-server/src/server/handlers.rs` - Update request deserialization for affected endpoints
- Request struct definitions (e.g., `MSetNxRequest`, `HashMSetRequest`, `ZAddRequest`, etc.)

**Breaking Change**: NO (backward compatible - will accept both old and new formats where possible)

**User Benefit**:
- Improved API usability and consistency
- Better error messages for format mismatches
- Complete API coverage (100% of routes working)
- Clearer documentation for request formats

## Testing Results

**Current Status** (from REST API testing):
- ✅ **75/85 routes working** (88% success rate)
- ❌ **10 routes with format issues** (12% need fixes)

**Implementation Status** (Updated: November 25, 2025):
- ✅ **All critical format fixes implemented** (MSETNX, HMSET, LPOP/RPOP, ZADD, Pub/Sub, Memory Usage)
- ✅ **Comprehensive test coverage added** (10+ integration tests)
- ✅ **OpenAPI specification updated** with corrected schemas and examples
- ✅ **CHANGELOG.md updated** with all fixes
- 🔄 **REST_API.md documentation** - In progress
- ⏳ **Full test suite validation** - Pending

**Categories**:
- Health & Monitoring: 4/5 (80%)
- KV Operations: 6/6 (100%) ✅
- String Extensions: 5/6 (83%)
- Key Management: 4/4 (100%) ✅
- Hash Operations: 12/13 (92%)
- List Operations: 7/10 (70%)
- Set Operations: 8/8 (100%) ✅
- Sorted Set Operations: 5/6 (83%)
- Queue Operations: 5/5 (100%) ✅
- Stream Operations: 4/5 (80%)
- Pub/Sub Operations: 3/4 (75%)
- Transaction Operations: 4/4 (100%) ✅
- Lua Scripting: 4/4 (100%) ✅
- Geospatial Operations: 2/3 (67%)
- Bitmap Operations: 4/4 (100%) ✅
