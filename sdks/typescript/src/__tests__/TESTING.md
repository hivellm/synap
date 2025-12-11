# Testing Strategy

The Synap TypeScript SDK uses a dual testing approach with **Unit Tests (Mock)** and **Server-to-Server (S2S) Tests**.

## Test Types

### 1. Unit Tests (Mock) - No Server Required ✅
**Files:** `*.test.ts`  
**Purpose:** Fast, isolated tests using mocked client  
**When to run:** Always (CI/CD, development, pre-commit)  
**Total:** 47 tests

```bash
# Run unit tests only (default)
npm test
npm run test:unit

# Watch mode
npm run test:watch
```

**Features:**
- ✅ No server dependency
- ✅ Fast execution (~1 second)
- ✅ Isolated testing
- ✅ Perfect for CI/CD
- ✅ Tests business logic and API contracts

### 2. Server-to-Server Tests - Requires Synap Server ⚙️
**Files:** `*.s2s.test.ts`  
**Purpose:** Integration tests with real server  
**When to run:** Optional (integration testing, manual verification)  
**Total:** 68 tests

```bash
# Run s2s tests (requires server on localhost:15500)
npm run test:s2s

# Run all tests
npm run test:all
```

**Features:**
- ⚠️ Requires running Synap server
- ⚠️ Slower execution (~7 seconds)
- ✅ End-to-end validation
- ✅ Tests real server behavior
- ✅ Integration testing

## Test Structure

```
src/__tests__/
├── __mocks__/
│   └── client.mock.ts         - Mock client factory
│
├── *.test.ts                  - Unit tests (mock) ✅
│   ├── client.test.ts         - 5 tests
│   ├── kv.test.ts             - 20 tests
│   ├── queue.reactive.test.ts - 9 tests
│   └── stream.test.ts         - 13 tests
│
└── *.s2s.test.ts             - S2S tests (server) ⚙️
    ├── client.s2s.test.ts     - 5 tests
    ├── kv.s2s.test.ts         - 18 tests
    ├── queue.s2s.test.ts      - 12 tests
    ├── queue.reactive.s2s.test.ts - 17 tests
    └── stream.s2s.test.ts     - 16 tests
```

## Test Results

### Unit Tests (Mock)
```
✅ 47/47 passing (100%)

┌────────────────────┬────────┬──────────┬────────┐
│ Test Suite         │ Tests  │ Passing  │   %    │
├────────────────────┼────────┼──────────┼────────┤
│ Client             │   5    │    5     │ 100% ✅│
│ KV Store           │  20    │   20     │ 100% ✅│
│ Queue (reactive)   │   9    │    9     │ 100% ✅│
│ Stream             │  13    │   13     │ 100% ✅│
├────────────────────┼────────┼──────────┼────────┤
│ TOTAL (Unit)       │  47    │   47     │ 100% ✅│
└────────────────────┴────────┴──────────┴────────┘
```

### S2S Tests (Server Required)
```
✅ 68/68 passing (100%)

┌────────────────────┬────────┬──────────┬────────┐
│ Test Suite         │ Tests  │ Passing  │   %    │
├────────────────────┼────────┼──────────┼────────┤
│ Client             │   5    │    5     │ 100% ✅│
│ KV Store           │  18    │   18     │ 100% ✅│
│ Queue (trad.)      │  12    │   12     │ 100% ✅│
│ Queue (reactive)   │  17    │   17     │ 100% ✅│
│ Stream             │  16    │   16     │ 100% ✅│
├────────────────────┼────────┼──────────┼────────┤
│ TOTAL (S2S)        │  68    │   68     │ 100% ✅│
└────────────────────┴────────┴──────────┴────────┘
```

### Combined Total
```
🎉 115 TOTAL TESTS
   - 47 Unit Tests (mock)
   - 68 S2S Tests (server)
   - 100% passing in both modes
```

## Mock Client

The mock client simulates server responses for testing without a running server.

### Basic Usage

```typescript
import { createMockClient } from './__mocks__/client.mock';
import { QueueManager } from '../queue';

const mockClient = createMockClient();
const queue = new QueueManager(mockClient);

// Mock returns default responses automatically
const result = await queue.createQueue('test');
```

### Custom Responses

```typescript
import { vi } from 'vitest';

const mockClient = createMockClient();

// Mock specific command
vi.mocked(mockClient.sendCommand).mockResolvedValue({ success: true });

// Mock with custom logic
vi.mocked(mockClient.sendCommand).mockImplementation(async (cmd, payload) => {
  if (cmd === 'queue.publish') {
    return { message_id: 'custom-id' };
  }
  return { success: true };
});
```

### Scenario Mocks

```typescript
import { createScenarioMock } from './__mocks__/client.mock';

// Empty queue scenario
const emptyQueue = createScenarioMock('empty-queue');

// Full queue scenario
const fullQueue = createScenarioMock('full-queue');

// Error scenario
const errorClient = createScenarioMock('error');
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Tests

on: [push, pull_request]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: '18'
      
      - run: npm install
      - run: npm run test:unit        # Fast unit tests
      - run: npm run test:coverage
      
      - name: Upload coverage
        uses: codecov/codecov-action@v3

  integration-tests:
    runs-on: ubuntu-latest
    services:
      synap:
        image: synap-server:latest
        ports:
          - 15500:15500
    
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: '18'
      
      - run: npm install
      - run: npm run test:s2s         # Integration tests with server
```

## Test Development

### Writing Unit Tests

```typescript
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { QueueManager } from '../queue';
import { createMockClient } from './__mocks__/client.mock';

describe('QueueManager (Unit Tests)', () => {
  let mockClient: SynapClient;
  let queue: QueueManager;

  beforeEach(() => {
    mockClient = createMockClient();
    queue = new QueueManager(mockClient);
  });

  it('should do something', async () => {
    // Arrange
    vi.mocked(mockClient.sendCommand).mockResolvedValue({ success: true });

    // Act
    const result = await queue.createQueue('test');

    // Assert
    expect(result).toBe(true);
  });
});
```

### Writing S2S Tests

```typescript
import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { Synap } from '../index';

describe('QueueManager (S2S Tests)', () => {
  let synap: Synap;

  beforeAll(async () => {
    synap = new Synap({
      url: process.env.SYNAP_URL || 'http://localhost:15500',
    });
  });

  afterAll(() => {
    synap.close();
  });

  it('should work with real server', async () => {
    await synap.queue.createQueue('real-queue');
    // ... test with real server
  });
});
```

## Best Practices

### 1. Default to Unit Tests
Always write unit tests first. They're faster and don't require infrastructure.

### 2. Use S2S for Integration
Use S2S tests for:
- End-to-end validation
- Testing actual server behavior
- Verifying protocol compatibility
- Performance testing

### 3. Mock Realistic Responses
Mock responses should match actual server responses:
```typescript
// Good
vi.mocked(client.sendCommand).mockResolvedValue({ 
  message_id: 'msg-123',
  success: true 
});

// Bad  
vi.mocked(client.sendCommand).mockResolvedValue({ ok: true });
```

### 4. Keep Tests Independent
Each test should:
- Set up its own state
- Not depend on other tests
- Clean up after itself

### 5. Test Both Success and Failure
```typescript
it('should handle success', async () => {
  vi.mocked(client.sendCommand).mockResolvedValue({ success: true });
  // ...
});

it('should handle errors', async () => {
  vi.mocked(client.sendCommand).mockRejectedValue(new Error('Network error'));
  // ...
});
```

## Commands Reference

```bash
# Unit tests (no server) - DEFAULT
npm test                    # Same as test:unit
npm run test:unit          # Run unit tests only
npm run test:watch         # Watch mode (unit tests)
npm run test:coverage      # Coverage report

# S2S tests (requires server)
npm run test:s2s           # Run s2s tests only

# All tests
npm run test:all           # Run unit + s2s tests

# Specific test file
npm test -- queue.test     # Unit test for queue
npm run test:s2s -- queue.reactive.s2s.test  # S2S queue reactive
```

## Environment Variables

```bash
# For S2S tests
export SYNAP_URL=http://localhost:15500

# Enable S2S tests
export RUN_S2S=true
```

## Coverage

```bash
# Generate coverage report (unit tests only)
npm run test:coverage

# View coverage
open coverage/index.html
```

## Troubleshooting

### Unit Tests Failing
- Check mock setup
- Verify mock responses match expected format
- Check import paths

### S2S Tests Failing
- Ensure Synap server is running
- Check server URL (`SYNAP_URL`)
- Verify server version compatibility
- Check network connectivity

### All Tests Failing
- Run `npm install` to ensure dependencies
- Check TypeScript compilation: `npm run build`
- Verify vitest configuration

## Performance

| Test Type | Duration | When to Use |
|-----------|----------|-------------|
| Unit (47 tests) | ~1 second | Always |
| S2S (68 tests) | ~7 seconds | Integration |
| Combined | ~8 seconds | Full validation |

## Conclusion

- **Development:** Use unit tests (`npm test`)
- **CI/CD:** Use unit tests only for speed
- **Pre-release:** Run all tests (`npm run test:all`)
- **Integration:** Use s2s tests with real server

This dual approach provides:
- ✅ Fast feedback during development
- ✅ No infrastructure requirements for basic testing
- ✅ Full integration validation when needed
- ✅ Flexible testing strategy

---

**Total Test Coverage: 115 tests**  
**Unit Tests: 47 (100% passing)**  
**S2S Tests: 68 (100% passing)**  
**Overall: 100% passing** ✅

