/**
 * K6 Load Test - Mixed Workload (Realistic)
 * 
 * Simulates real-world usage with:
 * - 60% KV operations (cache-like)
 * - 25% Queue operations (background jobs)
 * - 10% Stream operations (real-time events)
 * - 5% Pub/Sub operations (notifications)
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Counter, Rate, Trend } from 'k6/metrics';

// Operation counters
const kvOps = new Counter('kv_operations');
const queueOps = new Counter('queue_operations');
const streamOps = new Counter('stream_operations');
const pubsubOps = new Counter('pubsub_operations');

// Success rates
const kvSuccess = new Rate('kv_success_rate');
const queueSuccess = new Rate('queue_success_rate');
const streamSuccess = new Rate('stream_success_rate');
const pubsubSuccess = new Rate('pubsub_success_rate');

// Latencies
const kvLatency = new Trend('kv_latency_ms');
const queueLatency = new Trend('queue_latency_ms');
const streamLatency = new Trend('stream_latency_ms');

export const options = {
  scenarios: {
    mixed_load: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '30s', target: 100 },
        { duration: '1m', target: 300 },
        { duration: '2m', target: 300 },
        { duration: '30s', target: 0 },
      ],
    },
  },
  
  thresholds: {
    'http_req_duration': ['p(95)<10', 'p(99)<20'],
    'kv_success_rate': ['rate>0.99'],
    'queue_success_rate': ['rate>0.95'],
    'stream_success_rate': ['rate>0.95'],
  },
};

const BASE_URL = __ENV.SYNAP_URL || 'http://localhost:15500';

export function setup() {
  // Setup queue
  http.post(`${BASE_URL}/queue/mixed-queue`,
    JSON.stringify({ max_depth: 100000, ack_deadline_secs: 30 }),
    { headers: { 'Content-Type': 'application/json' } }
  );
  
  // Setup stream
  http.post(`${BASE_URL}/stream/mixed-stream`);
  
  return { ready: true };
}

export default function() {
  const rand = Math.random();
  
  if (rand < 0.60) {
    // 60% - KV operations
    kvOperation();
  } else if (rand < 0.85) {
    // 25% - Queue operations
    queueOperation();
  } else if (rand < 0.95) {
    // 10% - Stream operations
    streamOperation();
  } else {
    // 5% - Pub/Sub operations
    pubsubOperation();
  }
  
  sleep(0.01);
}

function kvOperation() {
  const operation = Math.random();
  
  if (operation < 0.7) {
    // 70% GET (cache reads)
    const key = `cache:item:${Math.floor(Math.random() * 1000)}`;
    const start = Date.now();
    const res = http.get(`${BASE_URL}/kv/get/${key}`);
    kvLatency.add(Date.now() - start);
    
    kvSuccess.add(check(res, { 'status 200': (r) => r.status === 200 }));
  } else {
    // 30% SET (cache writes)
    const key = `cache:item:${Math.floor(Math.random() * 1000)}`;
    const value = JSON.stringify({ data: 'cached_value', timestamp: Date.now() });
    
    const start = Date.now();
    const res = http.post(`${BASE_URL}/kv/set`,
      JSON.stringify({ key, value, ttl: 300 }),
      { headers: { 'Content-Type': 'application/json' } }
    );
    kvLatency.add(Date.now() - start);
    
    kvSuccess.add(check(res, { 'status 200': (r) => r.status === 200 }));
  }
  
  kvOps.add(1);
}

function queueOperation() {
  const operation = Math.random();
  
  if (operation < 0.6) {
    // 60% Publish
    const payloadStr = JSON.stringify({ job: 'process', id: __VU, iter: __ITER });
    const payload = [];
    for (let i = 0; i < payloadStr.length; i++) {
      payload.push(payloadStr.charCodeAt(i));
    }
    
    const start = Date.now();
    const res = http.post(`${BASE_URL}/queue/mixed-queue/publish`,
      JSON.stringify({ payload, priority: Math.floor(Math.random() * 10) }),
      { headers: { 'Content-Type': 'application/json' } }
    );
    queueLatency.add(Date.now() - start);
    
    queueSuccess.add(check(res, { 'status 200': (r) => r.status === 200 }));
  } else {
    // 40% Consume + ACK
    const start = Date.now();
    const res = http.get(`${BASE_URL}/queue/mixed-queue/consume/consumer-${__VU}`);
    queueLatency.add(Date.now() - start);
    
    if (res.status === 200) {
      try {
        const msg = JSON.parse(res.body);
        http.post(`${BASE_URL}/queue/mixed-queue/ack`,
          JSON.stringify({ message_id: msg.message_id }),
          { headers: { 'Content-Type': 'application/json' } }
        );
      } catch {}
    }
    
    queueSuccess.add(check(res, { 'status 200 or 404': (r) => r.status === 200 || r.status === 404 }));
  }
  
  queueOps.add(1);
}

function streamOperation() {
  // Publish to stream
  const start = Date.now();
  const res = http.post(`${BASE_URL}/stream/mixed-stream/publish`,
    JSON.stringify({
      event: 'test.event',
      data: JSON.stringify({ vu: __VU, iter: __ITER })
    }),
    { headers: { 'Content-Type': 'application/json' } }
  );
  streamLatency.add(Date.now() - start);
  
  streamSuccess.add(check(res, { 'status 200': (r) => r.status === 200 }));
  streamOps.add(1);
}

function pubsubOperation() {
  // Publish to topic
  const topics = ['events.user', 'events.order', 'notifications.email'];
  const topic = topics[Math.floor(Math.random() * topics.length)];
  
  const res = http.post(`${BASE_URL}/pubsub/${topic}/publish`,
    JSON.stringify({ message: `Event from VU ${__VU}` }),
    { headers: { 'Content-Type': 'application/json' } }
  );
  
  pubsubSuccess.add(check(res, { 'status 200': (r) => r.status === 200 }));
  pubsubOps.add(1);
}

export function handleSummary(data) {
  const metrics = data.metrics;
  const duration = data.state.testRunDurationMs / 1000;
  
  // Calculate total ops/sec
  const totalOps = 
    metrics.kv_operations.values.count +
    metrics.queue_operations.values.count +
    metrics.stream_operations.values.count +
    metrics.pubsub_operations.values.count;
  
  const opsPerSec = totalOps / duration;
  
  let summary = '\n' + '='.repeat(60) + '\n';
  summary += 'K6 Stress Test - Maximum Throughput\n';
  summary += '='.repeat(60) + '\n\n';
  
  summary += `Test Duration: ${duration.toFixed(2)} seconds\n`;
  summary += `Peak VUs: 2000\n\n`;
  
  summary += `TOTAL OPERATIONS: ${totalOps}\n`;
  summary += `OPERATIONS/SEC: ${opsPerSec.toFixed(2)} ops/s\n\n`;
  
  // Target validation
  if (opsPerSec >= 100000) {
    summary += `🎯 ✅ TARGET ACHIEVED!\n`;
    summary += `   ${opsPerSec.toFixed(2)} ops/s >= 100,000 ops/s target\n`;
    summary += `   Exceeded by: ${((opsPerSec / 100000 - 1) * 100).toFixed(2)}%\n\n`;
  } else {
    summary += `🎯 ⚠️  TARGET NOT MET\n`;
    summary += `   ${opsPerSec.toFixed(2)} ops/s < 100,000 ops/s target\n`;
    summary += `   Gap: ${(100000 - opsPerSec).toFixed(2)} ops/s (${((1 - opsPerSec / 100000) * 100).toFixed(2)}%)\n\n`;
  }
  
  summary += `Operations Breakdown:\n`;
  summary += `  KV:     ${metrics.kv_operations.values.count.toLocaleString()} (${((metrics.kv_operations.values.count / totalOps) * 100).toFixed(1)}%)\n`;
  summary += `  Queue:  ${metrics.queue_operations.values.count.toLocaleString()} (${((metrics.queue_operations.values.count / totalOps) * 100).toFixed(1)}%)\n`;
  summary += `  Stream: ${metrics.stream_operations.values.count.toLocaleString()} (${((metrics.stream_operations.values.count / totalOps) * 100).toFixed(1)}%)\n`;
  summary += `  PubSub: ${metrics.pubsub_operations.values.count.toLocaleString()} (${((metrics.pubsub_operations.values.count / totalOps) * 100).toFixed(1)}%)\n\n`;
  
  summary += `Success Rates:\n`;
  summary += `  Overall: ${(metrics.success_rate.values.rate * 100).toFixed(2)}%\n`;
  summary += `  KV:      ${(metrics.kv_success_rate.values.rate * 100).toFixed(2)}%\n`;
  summary += `  Queue:   ${(metrics.queue_success_rate.values.rate * 100).toFixed(2)}%\n`;
  summary += `  Stream:  ${(metrics.stream_success_rate.values.rate * 100).toFixed(2)}%\n`;
  summary += `  PubSub:  ${(metrics.pubsub_success_rate.values.rate * 100).toFixed(2)}%\n\n`;
  
  summary += `Latency (P95/P99):\n`;
  summary += `  KV:     ${metrics.kv_latency_ms.values['p(95)'].toFixed(2)} / ${metrics.kv_latency_ms.values['p(99)'].toFixed(2)} ms\n`;
  summary += `  Queue:  ${metrics.queue_latency_ms.values['p(95)'].toFixed(2)} / ${metrics.queue_latency_ms.values['p(99)'].toFixed(2)} ms\n`;
  summary += `  Stream: ${metrics.stream_latency_ms.values['p(95)'].toFixed(2)} / ${metrics.stream_latency_ms.values['p(99)'].toFixed(2)} ms\n\n`;
  
  summary += `HTTP Performance:\n`;
  summary += `  Total Requests: ${metrics.http_reqs.values.count.toLocaleString()}\n`;
  summary += `  Request Rate: ${metrics.http_reqs.values.rate.toFixed(2)} req/s\n`;
  summary += `  Failed: ${(metrics.http_req_failed.values.rate * 100).toFixed(2)}%\n\n`;
  
  summary += '='.repeat(60) + '\n';
  
  console.log(summary);
  
  return {
    'stdout': summary,
    'tests/load/results/mixed-workload-summary.json': JSON.stringify(data, null, 2),
    'tests/load/results/mixed-workload-report.txt': summary,
  };
}

