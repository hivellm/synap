/**
 * Simple K6 Load Test - KV Operations Only
 * 
 * Tests: SET and GET operations
 * Target: Measure actual throughput
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Counter, Rate, Trend } from 'k6/metrics';

// Custom metrics
const setOps = new Counter('set_operations');
const getOps = new Counter('get_operations');
const setSuccess = new Rate('set_success_rate');
const getSuccess = new Rate('get_success_rate');
const setLatency = new Trend('set_latency_ms');
const getLatency = new Trend('get_latency_ms');

export const options = {
  scenarios: {
    load_test: {
      executor: 'constant-vus',
      vus: 100,
      duration: '1m',
    },
  },
  
  thresholds: {
    'http_req_duration': ['p(95)<10', 'p(99)<20'],
    'set_success_rate': ['rate>0.99'],
    'get_success_rate': ['rate>0.99'],
  },
};

const BASE_URL = __ENV.SYNAP_URL || 'http://localhost:15500';

export default function() {
  const key = `test:vu${__VU}:iter${__ITER}`;
  const value = `value_${Date.now()}_${Math.random().toString(36)}`;
  
  // SET operation
  const setStart = Date.now();
  const setRes = http.post(`${BASE_URL}/kv/set`,
    JSON.stringify({
      key: key,
      value: value,
      ttl: 60
    }),
    {
      headers: { 'Content-Type': 'application/json' },
    }
  );
  setLatency.add(Date.now() - setStart);
  
  const setOk = check(setRes, {
    'SET status 200': (r) => r.status === 200,
  });
  setSuccess.add(setOk);
  setOps.add(1);
  
  // GET operation
  const getStart = Date.now();
  const getRes = http.get(`${BASE_URL}/kv/get/${key}`);
  getLatency.add(Date.now() - getStart);
  
  const getOk = check(getRes, {
    'GET status 200': (r) => r.status === 200,
    'GET correct value': (r) => r.body === `"${value}"` || r.body.includes(value),
  });
  getSuccess.add(getOk);
  getOps.add(1);
  
  // Small sleep to avoid overwhelming
  sleep(0.001);
}

export function handleSummary(data) {
  const metrics = data.metrics;
  const duration = data.state.testRunDurationMs / 1000;
  
  const totalOps = metrics.set_operations.values.count + metrics.get_operations.values.count;
  const opsPerSec = totalOps / duration;
  
  let summary = '\n' + '='.repeat(70) + '\n';
  summary += 'K6 Load Test - Simple KV Operations\n';
  summary += '='.repeat(70) + '\n\n';
  
  summary += `Test Duration: ${duration.toFixed(2)} seconds\n`;
  summary += `Virtual Users: 100\n\n`;
  
  summary += `TOTAL OPERATIONS: ${totalOps.toLocaleString()}\n`;
  summary += `OPERATIONS/SEC: ${opsPerSec.toFixed(2)} ops/s\n\n`;
  
  // Check 100K target
  if (opsPerSec >= 100000) {
    summary += `🎯 ✅ TARGET ACHIEVED!\n`;
    summary += `   ${opsPerSec.toLocaleString()} ops/s >= 100,000 ops/s\n`;
    summary += `   Exceeded by: ${((opsPerSec / 100000 - 1) * 100).toFixed(1)}%\n\n`;
  } else {
    summary += `🎯 ⚠️  Below 100K target\n`;
    summary += `   ${opsPerSec.toLocaleString()} ops/s (${((opsPerSec / 100000) * 100).toFixed(1)}% of target)\n`;
    summary += `   Note: Mixed operations expected to be slower than pure GET\n\n`;
  }
  
  summary += `Operations Breakdown:\n`;
  summary += `  SET: ${metrics.set_operations.values.count.toLocaleString()} ops (${(metrics.set_operations.values.count / duration).toFixed(2)} ops/s)\n`;
  summary += `  GET: ${metrics.get_operations.values.count.toLocaleString()} ops (${(metrics.get_operations.values.count / duration).toFixed(2)} ops/s)\n\n`;
  
  summary += `Success Rates:\n`;
  summary += `  SET: ${(metrics.set_success_rate.values.rate * 100).toFixed(2)}%\n`;
  summary += `  GET: ${(metrics.get_success_rate.values.rate * 100).toFixed(2)}%\n\n`;
  
  summary += `Latency (ms):\n`;
  summary += `  SET - P50: ${metrics.set_latency_ms.values['p(50)'].toFixed(2)}  P95: ${metrics.set_latency_ms.values['p(95)'].toFixed(2)}  P99: ${metrics.set_latency_ms.values['p(99)'].toFixed(2)}\n`;
  summary += `  GET - P50: ${metrics.get_latency_ms.values['p(50)'].toFixed(2)}  P95: ${metrics.get_latency_ms.values['p(95)'].toFixed(2)}  P99: ${metrics.get_latency_ms.values['p(99)'].toFixed(2)}\n\n`;
  
  summary += `HTTP Metrics:\n`;
  summary += `  Total Requests: ${metrics.http_reqs.values.count.toLocaleString()}\n`;
  summary += `  Request Rate: ${metrics.http_reqs.values.rate.toFixed(2)} req/s\n`;
  summary += `  Failed: ${(metrics.http_req_failed.values.rate * 100).toFixed(4)}%\n`;
  summary += `  Duration P95: ${metrics.http_req_duration.values['p(95)'].toFixed(2)} ms\n`;
  summary += `  Duration P99: ${metrics.http_req_duration.values['p(99)'].toFixed(2)} ms\n\n`;
  
  summary += '='.repeat(70) + '\n';
  
  console.log(summary);
  
  return {
    'stdout': summary,
    'tests/load/results/simple-kv-summary.json': JSON.stringify(data, null, 2),
    'tests/load/results/simple-kv-report.txt': summary,
  };
}

