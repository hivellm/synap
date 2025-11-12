/**
 * K6 Load Test - KV Operations
 * 
 * Tests: GET, SET, DELETE operations
 * Target: 100K ops/sec sustained
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

// Custom metrics
const setRate = new Rate('set_success_rate');
const getRate = new Rate('get_success_rate');
const delRate = new Rate('delete_success_rate');
const setLatency = new Trend('set_latency_ms');
const getLatency = new Trend('get_latency_ms');

// Test configuration
export const options = {
  scenarios: {
    // Warm-up phase
    warmup: {
      executor: 'constant-vus',
      vus: 10,
      duration: '10s',
      tags: { phase: 'warmup' },
    },
    
    // Ramp-up test
    rampup: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '30s', target: 50 },
        { duration: '30s', target: 100 },
        { duration: '30s', target: 200 },
        { duration: '30s', target: 500 },
      ],
      startTime: '10s',
      tags: { phase: 'rampup' },
    },
    
    // Sustained load
    sustained: {
      executor: 'constant-vus',
      vus: 500,
      duration: '2m',
      startTime: '2m10s',
      tags: { phase: 'sustained' },
    },
    
    // Spike test
    spike: {
      executor: 'ramping-vus',
      startVUs: 500,
      stages: [
        { duration: '10s', target: 1000 },
        { duration: '30s', target: 1000 },
        { duration: '10s', target: 500 },
      ],
      startTime: '4m10s',
      tags: { phase: 'spike' },
    },
  },
  
  thresholds: {
    'http_req_duration': ['p(95)<5', 'p(99)<10'], // 95% < 5ms, 99% < 10ms
    'http_req_failed': ['rate<0.01'], // Error rate < 1%
    'set_success_rate': ['rate>0.99'], // 99% success
    'get_success_rate': ['rate>0.99'],
  },
};

const BASE_URL = __ENV.SYNAP_URL || 'http://localhost:15500';

export default function() {
  const vuId = __VU;
  const iter = __ITER;
  
  // Test data
  const key = `loadtest:vu${vuId}:iter${iter}`;
  const value = `value_${Date.now()}_${Math.random()}`;
  
  // SET operation
  const setStart = Date.now();
  const setRes = http.post(`${BASE_URL}/kv/set`, 
    JSON.stringify({
      key: key,
      value: value,
      ttl: 300
    }),
    {
      headers: { 'Content-Type': 'application/json' },
      tags: { operation: 'set' },
    }
  );
  setLatency.add(Date.now() - setStart);
  
  const setSuccess = check(setRes, {
    'SET status 200': (r) => r.status === 200,
    'SET has response': (r) => r.body.length > 0,
  });
  setRate.add(setSuccess);
  
  // GET operation
  const getStart = Date.now();
  const getRes = http.get(`${BASE_URL}/kv/get/${key}`, {
    tags: { operation: 'get' },
  });
  getLatency.add(Date.now() - getStart);
  
  const getSuccess = check(getRes, {
    'GET status 200': (r) => r.status === 200,
    'GET correct value': (r) => r.body.includes(value) || r.body === `"${value}"`,
  });
  getRate.add(getSuccess);
  
  // DELETE operation (every 10 iterations to avoid filling memory)
  if (iter % 10 === 0) {
    const delRes = http.del(`${BASE_URL}/kv/del/${key}`, {
      tags: { operation: 'delete' },
    });
    
    const delSuccess = check(delRes, {
      'DELETE status 200': (r) => r.status === 200,
    });
    delRate.add(delSuccess);
  }
  
  // Small delay to avoid overwhelming server
  sleep(0.01); // 10ms between iterations
}

export function handleSummary(data) {
  return {
    'stdout': textSummary(data, { indent: ' ', enableColors: true }),
    'tests/load/results/kv-operations-summary.json': JSON.stringify(data, null, 2),
  };
}

function textSummary(data, options) {
  const indent = options.indent || '';
  const enableColors = options.enableColors || false;
  
  let summary = '\n' + indent + '='.repeat(60) + '\n';
  summary += indent + 'K6 Load Test - KV Operations\n';
  summary += indent + '='.repeat(60) + '\n\n';
  
  // Extract key metrics
  const metrics = data.metrics;
  
  summary += indent + 'HTTP Requests:\n';
  summary += indent + `  Total: ${metrics.http_reqs.values.count}\n`;
  summary += indent + `  Rate: ${metrics.http_reqs.values.rate.toFixed(2)} req/s\n`;
  summary += indent + `  Failed: ${(metrics.http_req_failed.values.rate * 100).toFixed(2)}%\n\n`;
  
  summary += indent + 'Latency:\n';
  summary += indent + `  P50: ${metrics.http_req_duration.values['p(50)'].toFixed(2)} ms\n`;
  summary += indent + `  P95: ${metrics.http_req_duration.values['p(95)'].toFixed(2)} ms\n`;
  summary += indent + `  P99: ${metrics.http_req_duration.values['p(99)'].toFixed(2)} ms\n\n`;
  
  if (metrics.set_success_rate) {
    summary += indent + 'Operation Success Rates:\n';
    summary += indent + `  SET: ${(metrics.set_success_rate.values.rate * 100).toFixed(2)}%\n`;
    summary += indent + `  GET: ${(metrics.get_success_rate.values.rate * 100).toFixed(2)}%\n`;
    summary += indent + `  DELETE: ${(metrics.delete_success_rate.values.rate * 100).toFixed(2)}%\n\n`;
  }
  
  summary += indent + '='.repeat(60) + '\n';
  
  return summary;
}

