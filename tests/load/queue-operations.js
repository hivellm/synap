/**
 * K6 Load Test - Queue Operations
 * 
 * Tests: Publish, Consume, ACK workflow
 * Target: 50K+ messages/sec (durable mode)
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';

// Custom metrics
const publishRate = new Rate('publish_success_rate');
const consumeRate = new Rate('consume_success_rate');
const ackRate = new Rate('ack_success_rate');
const publishLatency = new Trend('publish_latency_ms');
const messagesPublished = new Counter('messages_published');
const messagesConsumed = new Counter('messages_consumed');

export const options = {
  scenarios: {
    // Publisher load
    publishers: {
      executor: 'constant-vus',
      vus: 100,
      duration: '2m',
      exec: 'publishMessages',
      tags: { role: 'publisher' },
    },
    
    // Consumer load
    consumers: {
      executor: 'constant-vus',
      vus: 20,
      duration: '2m',
      exec: 'consumeMessages',
      tags: { role: 'consumer' },
    },
  },
  
  thresholds: {
    'publish_success_rate': ['rate>0.99'],
    'consume_success_rate': ['rate>0.95'],
    'publish_latency_ms': ['p(95)<10', 'p(99)<20'],
  },
};

const BASE_URL = __ENV.SYNAP_URL || 'http://localhost:15500';
const QUEUE_NAME = 'loadtest-queue';

// Setup: Create queue once
export function setup() {
  const res = http.post(`${BASE_URL}/queue/${QUEUE_NAME}`,
    JSON.stringify({
      max_depth: 1000000,
      ack_deadline_secs: 60,
      default_max_retries: 3,
    }),
    { headers: { 'Content-Type': 'application/json' } }
  );
  
  console.log(`Queue setup: ${res.status}`);
  return { queueName: QUEUE_NAME };
}

// Publisher scenario
export function publishMessages(data) {
  const payload = JSON.stringify({
    type: 'test-job',
    vu: __VU,
    iter: __ITER,
    timestamp: Date.now(),
  });
  
  // Convert string to byte array (k6 compatible)
  const payloadBytes = [];
  for (let i = 0; i < payload.length; i++) {
    payloadBytes.push(payload.charCodeAt(i));
  }
  const priority = Math.floor(Math.random() * 10); // 0-9
  
  const start = Date.now();
  const res = http.post(`${BASE_URL}/queue/${data.queueName}/publish`,
    JSON.stringify({
      payload: payloadBytes,
      priority: priority,
      max_retries: 3,
    }),
    {
      headers: { 'Content-Type': 'application/json' },
      tags: { operation: 'publish' },
    }
  );
  publishLatency.add(Date.now() - start);
  
  const success = check(res, {
    'Publish status 200': (r) => r.status === 200,
    'Has message_id': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.message_id !== undefined;
      } catch {
        return false;
      }
    },
  });
  
  publishRate.add(success);
  if (success) messagesPublished.add(1);
  
  sleep(0.01); // 10ms between publishes
}

// Consumer scenario
export function consumeMessages(data) {
  const consumerId = `consumer-${__VU}`;
  
  // Consume message
  const consumeRes = http.get(`${BASE_URL}/queue/${data.queueName}/consume/${consumerId}`, {
    tags: { operation: 'consume' },
  });
  
  const consumed = check(consumeRes, {
    'Consume status 200 or 404': (r) => r.status === 200 || r.status === 404,
  });
  
  if (consumeRes.status === 200) {
    try {
      const msg = JSON.parse(consumeRes.body);
      messagesConsumed.add(1);
      
      // ACK message
      const ackRes = http.post(`${BASE_URL}/queue/${data.queueName}/ack`,
        JSON.stringify({ message_id: msg.message_id }),
        {
          headers: { 'Content-Type': 'application/json' },
          tags: { operation: 'ack' },
        }
      );
      
      const acked = check(ackRes, {
        'ACK status 200': (r) => r.status === 200,
      });
      
      ackRate.add(acked);
      consumeRate.add(true);
    } catch (e) {
      consumeRate.add(false);
    }
  } else {
    // No messages available
    consumeRate.add(true);
  }
  
  sleep(0.05); // 50ms between consume attempts
}

// Cleanup
export function teardown(data) {
  // Get final stats
  const stats = http.get(`${BASE_URL}/queue/${data.queueName}/stats`);
  console.log(`Final queue stats: ${stats.body}`);
}

export function handleSummary(data) {
  const metrics = data.metrics;
  
  let summary = '\n' + '='.repeat(60) + '\n';
  summary += 'K6 Load Test - Queue Operations\n';
  summary += '='.repeat(60) + '\n\n';
  
  summary += `Total Requests: ${metrics.http_reqs.values.count}\n`;
  summary += `Request Rate: ${metrics.http_reqs.values.rate.toFixed(2)} req/s\n`;
  summary += `Failed Requests: ${(metrics.http_req_failed.values.rate * 100).toFixed(2)}%\n\n`;
  
  summary += `Messages Published: ${metrics.messages_published.values.count}\n`;
  summary += `Messages Consumed: ${metrics.messages_consumed.values.count}\n`;
  summary += `Publish Rate: ${(metrics.messages_published.values.count / (data.state.testRunDurationMs / 1000)).toFixed(2)} msg/s\n\n`;
  
  summary += `Publish Success: ${(metrics.publish_success_rate.values.rate * 100).toFixed(2)}%\n`;
  summary += `Consume Success: ${(metrics.consume_success_rate.values.rate * 100).toFixed(2)}%\n`;
  summary += `ACK Success: ${(metrics.ack_success_rate.values.rate * 100).toFixed(2)}%\n\n`;
  
  summary += `Publish Latency:\n`;
  summary += `  P50: ${metrics.publish_latency_ms.values['p(50)'].toFixed(2)} ms\n`;
  summary += `  P95: ${metrics.publish_latency_ms.values['p(95)'].toFixed(2)} ms\n`;
  summary += `  P99: ${metrics.publish_latency_ms.values['p(99)'].toFixed(2)} ms\n\n`;
  
  summary += '='.repeat(60) + '\n';
  
  console.log(summary);
  
  return {
    'tests/load/results/queue-operations-summary.json': JSON.stringify(data, null, 2),
  };
}

