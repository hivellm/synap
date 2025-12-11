/**
 * K6 Stress Test - Maximum Throughput
 * 
 * Goal: Find breaking point and validate 100K ops/sec target
 */

import http from 'k6/http';
import { check } from 'k6';
import { Counter, Rate, Trend } from 'k6/metrics';

// Custom metrics
const opsTotal = new Counter('total_operations');
const successRate = new Rate('success_rate');
const latency = new Trend('operation_latency_ms');

export const options = {
  scenarios: {
    // Aggressive ramp-up to find limits
    stress: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '1m', target: 100 },    // Warm-up
        { duration: '1m', target: 500 },    // Ramp to 500 VUs
        { duration: '1m', target: 1000 },   // Ramp to 1000 VUs
        { duration: '1m', target: 2000 },   // Ramp to 2000 VUs
        { duration: '2m', target: 2000 },   // Sustained high load
        { duration: '1m', target: 0 },      // Ramp down
      ],
    },
  },
  
  thresholds: {
    'http_req_duration': ['p(95)<50'], // Relaxed for stress test
    'success_rate': ['rate>0.95'], // 95% success acceptable under stress
  },
};

const BASE_URL = __ENV.SYNAP_URL || 'http://localhost:15500';

export default function() {
  const operations = [
    () => testKVSet(),
    () => testKVGet(),
    () => testQueuePublish(),
  ];
  
  // Random operation
  const op = operations[Math.floor(Math.random() * operations.length)];
  op();
}

function testKVSet() {
  const key = `stress:vu${__VU}:${__ITER}`;
  const value = `val_${Date.now()}`;
  
  const start = Date.now();
  const res = http.post(`${BASE_URL}/kv/set`,
    JSON.stringify({ key, value, ttl: 60 }),
    { headers: { 'Content-Type': 'application/json' } }
  );
  latency.add(Date.now() - start);
  
  const success = check(res, {
    'KV SET OK': (r) => r.status === 200,
  });
  
  successRate.add(success);
  opsTotal.add(1);
}

function testKVGet() {
  const key = `stress:vu${__VU % 100}:${__ITER % 100}`;
  
  const start = Date.now();
  const res = http.get(`${BASE_URL}/kv/get/${key}`);
  latency.add(Date.now() - start);
  
  const success = check(res, {
    'KV GET OK': (r) => r.status === 200,
  });
  
  successRate.add(success);
  opsTotal.add(1);
}

function testQueuePublish() {
  // Convert string to byte array without TextEncoder (k6 compatible)
  const str = `job_${__VU}_${__ITER}`;
  const payload = [];
  for (let i = 0; i < str.length; i++) {
    payload.push(str.charCodeAt(i));
  }
  
  const start = Date.now();
  const res = http.post(`${BASE_URL}/queue/stress-queue/publish`,
    JSON.stringify({ payload, priority: 5 }),
    { headers: { 'Content-Type': 'application/json' } }
  );
  latency.add(Date.now() - start);
  
  const success = check(res, {
    'Queue Publish OK': (r) => r.status === 200 || r.status === 201,
  });
  
  successRate.add(success);
  opsTotal.add(1);
}

export function setup() {
  // Create queue
  http.post(`${BASE_URL}/queue/stress-queue`,
    JSON.stringify({ max_depth: 1000000, ack_deadline_secs: 30 }),
    { headers: { 'Content-Type': 'application/json' } }
  );
  
  console.log('Stress test setup complete');
}

export function handleSummary(data) {
  const metrics = data.metrics;
  const duration = data.state.testRunDurationMs / 1000;
  const totalOps = metrics.total_operations.values.count;
  const opsPerSec = totalOps / duration;
  
  let summary = '\n' + '='.repeat(60) + '\n';
  summary += 'K6 Stress Test - Maximum Throughput\n';
  summary += '='.repeat(60) + '\n\n';
  
  summary += `Test Duration: ${duration.toFixed(2)} seconds\n`;
  summary += `Total Operations: ${totalOps}\n`;
  summary += `Operations/sec: ${opsPerSec.toFixed(2)} ops/s\n\n`;
  
  // Check if we hit 100K ops/sec target
  if (opsPerSec >= 100000) {
    summary += `✅ TARGET ACHIEVED: ${opsPerSec.toFixed(2)} ops/s >= 100,000 ops/s\n\n`;
  } else {
    summary += `❌ TARGET NOT MET: ${opsPerSec.toFixed(2)} ops/s < 100,000 ops/s\n`;
    summary += `   (Gap: ${(100000 - opsPerSec).toFixed(2)} ops/s)\n\n`;
  }
  
  summary += `HTTP Requests: ${metrics.http_reqs.values.count}\n`;
  summary += `Request Rate: ${metrics.http_reqs.values.rate.toFixed(2)} req/s\n`;
  summary += `Success Rate: ${(metrics.success_rate.values.rate * 100).toFixed(2)}%\n\n`;
  
  summary += `Latency:\n`;
  summary += `  P50: ${metrics.operation_latency_ms.values['p(50)'].toFixed(2)} ms\n`;
  summary += `  P95: ${metrics.operation_latency_ms.values['p(95)'].toFixed(2)} ms\n`;
  summary += `  P99: ${metrics.operation_latency_ms.values['p(99)'].toFixed(2)} ms\n`;
  summary += `  Max: ${metrics.operation_latency_ms.values.max.toFixed(2)} ms\n\n`;
  
  summary += `Peak VUs: 2000\n`;
  summary += `Peak Request Rate: ${metrics.http_reqs.values.rate.toFixed(2)} req/s\n\n`;
  
  summary += '='.repeat(60) + '\n';
  
  console.log(summary);
  
  return {
    'stdout': summary,
    'tests/load/results/stress-test-summary.json': JSON.stringify(data, null, 2),
    'tests/load/results/stress-test-report.txt': summary,
  };
}

