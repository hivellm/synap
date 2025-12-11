# Tasks: Fix REST API Request Formats

> **Status**: ✅ **COMPLETE** (100% Complete - SDKs Updated)  
> **Target**: v0.8.2  
> **Priority**: Medium  
> **Version**: 0.8.1  
> **Last Updated**: November 25, 2025

## 1. Analysis & Planning

- [x] 1.1 Identify all failing endpoints from REST API testing
- [x] 1.2 Document current request format vs expected format for each endpoint
- [x] 1.3 Review OpenAPI specification for format definitions
- [x] 1.4 Decide on standardized format for each endpoint (backward compatibility where possible)

## 2. String Extensions Fixes

- [x] 2.1 Fix `POST /kv/msetnx` - Update `MSetNxRequest` to accept object format `{"key": "...", "value": "..."}`
- [x] 2.2 Add test for `msetnx` with object format
- [x] 2.3 Verify backward compatibility with tuple format (if needed)

## 3. Hash Operations Fixes

- [x] 3.1 Fix `POST /hash/{key}/mset` - Update `HashMSetRequest` to accept array format
- [x] 3.2 Add test for `hash/mset` with array format
- [x] 3.3 Update handler to convert array to HashMap internally

## 4. List Operations Fixes

- [x] 4.1 Fix `POST /list/{key}/lpop` - Make `count` parameter optional (default to 1)
- [x] 4.2 Fix `POST /list/{key}/rpop` - Make `count` parameter optional (default to 1)
- [x] 4.3 Add tests for lpop/rpop with and without count parameter
- [x] 4.4 Update request struct to make count optional

## 5. Sorted Set Operations Fixes

- [x] 5.1 Fix `POST /sortedset/{key}/zadd` - Support both single member and array of members
- [x] 5.2 Update `ZAddRequest` to accept `members` array format (Redis-compatible)
- [x] 5.3 Add test for zadd with array of members
- [x] 5.4 Maintain backward compatibility with single member format

## 6. Pub/Sub Operations Fixes

- [x] 6.1 Fix `POST /pubsub/{topic}/publish` - Accept both `payload` and `data` fields
- [x] 6.2 Update handler to check both field names
- [x] 6.3 Add test for publish with `data` field
- [x] 6.4 Document preferred field name in API docs

## 7. Geospatial Operations Fixes

- [x] 7.1 Verify correct format for `POST /geospatial/{key}/geoadd`
- [x] 7.2 Fix handler if format is incorrect
- [x] 7.3 Add test for geoadd with correct format
- [x] 7.4 Update documentation with correct format

## 8. Stream Operations Fixes

- [x] 8.1 Verify correct format for `POST /stream/{room}` (room creation)
- [x] 8.2 Fix handler if format is incorrect
- [x] 8.3 Add test for stream room creation
- [x] 8.4 Update documentation

## 9. Memory Usage Fixes

- [x] 9.1 Fix `GET /memory/{key}/usage` - Return appropriate response for non-existent keys (0 or empty)
- [x] 9.2 Add test for memory usage with non-existent key
- [x] 9.3 Update handler to handle missing keys gracefully

## 10. StreamableHTTP Fixes

- [x] 10.1 Verify correct format for `POST /api/v1/command`
- [x] 10.2 Fix handler if format is incorrect
- [x] 10.3 Add test for StreamableHTTP command endpoint
- [x] 10.4 Update StreamableHTTP protocol documentation

## 11. Testing

- [x] 11.1 Create comprehensive REST API test suite covering all fixed endpoints
- [x] 11.2 Test backward compatibility where applicable
- [x] 11.3 Verify all 85 routes pass (100% success rate)
- [x] 11.4 Add integration tests for format variations
- [x] 11.5 Test error handling for invalid formats

## 12. Documentation

- [x] 12.1 Update `docs/api/REST_API.md` with correct request formats
- [x] 12.2 Update `docs/api/openapi.yml` with corrected schemas
- [x] 12.3 Add examples for each fixed endpoint
- [x] 12.4 Document backward compatibility where applicable
- [x] 12.5 Update CHANGELOG.md with fixes

## 13. Validation

- [x] 13.1 Run full REST API test suite
- [x] 13.2 Verify 100% route success rate
- [x] 13.3 Check OpenAPI schema validation
- [x] 13.4 Validate backward compatibility
- [x] 13.5 Code review and quality checks

## Success Criteria

- ✅ All 85 REST API routes passing (100% success rate)
- ✅ Request formats documented and consistent
- ✅ Backward compatibility maintained where possible
- ✅ Comprehensive test coverage for all fixed endpoints
- ✅ OpenAPI specification updated and validated
