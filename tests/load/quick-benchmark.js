/**
 * Quick Benchmark - Realistic Throughput Test
 * 
 * Tests with moderate load to find sustainable throughput
 * Duration: 1 minute
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Counter, Rate } from 'k6/metrics';

// Custom metrics
const totalOps = new Counter('total_operations');
const successRate = new Rate('success_rate');

export const options = {
  scenarios: {
    benchmark: {
      executor: 'constant-vus',
      vus: 50,  // Moderate load
      duration: '1m',
    },
  },
};

const BASE_URL = __ENV.SYNAP_URL || 'http://localhost:15500';

export default function() {
  // 70% GET, 30% SET (realistic cache pattern)
  const isGet = Math.random() < 0.7;
  
  if (isGet) {
    // GET operation
    const key = `bench:item:${Math.floor(Math.random() * 1000)}`;
    const res = http.get(`${BASE_URL}/kv/get/${key}`);
    
    const ok = check(res, {
      'status 200': (r) => r.status === 200,
    });
    successRate.add(ok);
  } else {
    // SET operation
    const key = `bench:item:${Math.floor(Math.random() * 1000)}`;
    const value = `val_${Date.now()}_${__VU}`;
    
    const res = http.post(`${BASE_URL}/kv/set`,
      JSON.stringify({ key, value, ttl: 300 }),
      { headers: { 'Content-Type': 'application/json' } }
    );
    
    const ok = check(res, {
      'status 200': (r) => r.status === 200,
    });
    successRate.add(ok);
  }
  
  totalOps.add(1);
}

export function handleSummary(data) {
  const metrics = data.metrics;
  const duration = data.state.testRunDurationMs / 1000;
  const ops = metrics.total_operations.values.count;
  const opsPerSec = ops / duration;
  
  const summary = `
======================================================================
K6 Quick Benchmark - Synap Throughput Test
======================================================================

Test Configuration:
  Virtual Users: 50
  Duration: ${duration.toFixed(2)} seconds
  
Results:
  Total Operations: ${ops.toLocaleString()}
  Operations/sec: ${opsPerSec.toLocaleString()} ops/s
  
  ${opsPerSec >= 100000 ? '✅ TARGET ACHIEVED (>= 100K ops/s)' : `📊 Current throughput: ${((opsPerSec / 100000) * 100).toFixed(1)}% of 100K target`}
  
  Success Rate: ${(metrics.success_rate.values.rate * 100).toFixed(2)}%
  
HTTP Performance:
  Total Requests: ${metrics.http_reqs.values.count.toLocaleString()}
  Request Rate: ${metrics.http_reqs.values.rate.toLocaleString()} req/s
  Failed: ${(metrics.http_req_failed.values.rate * 100).toFixed(4)}%
  
  Latency:
    P50: ${metrics.http_req_duration.values['p(50)'].toFixed(2)} ms
    P95: ${metrics.http_req_duration.values['p(95)'].toFixed(2)} ms
    P99: ${metrics.http_req_duration.values['p(99)'].toFixed(2)} ms
    Max: ${metrics.http_req_duration.values.max.toFixed(2)} ms

======================================================================
`;
  
  console.log(summary);
  
  return {
    'stdout': summary,
    'tests/load/results/quick-benchmark.txt': summary,
    'tests/load/results/quick-benchmark.json': JSON.stringify(data, null, 2),
  };
}

