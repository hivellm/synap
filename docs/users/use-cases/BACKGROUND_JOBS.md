---
title: Background Jobs
module: use-cases
id: background-jobs
order: 2
description: RabbitMQ replacement for background job processing
tags: [use-cases, jobs, rabbitmq, queue, workers]
---

# Background Jobs

Using Synap as a RabbitMQ replacement for background job processing.

## Overview

Synap's message queues provide:
- Priority support (0-9)
- ACK/NACK guarantees
- Retry logic with DLQ
- High throughput (44K+ ops/sec)

## Basic Pattern

### Producer (Web Server)

```python
from synap_sdk import SynapClient
import json

client = SynapClient("http://localhost:15500")

def submit_job(job_type, job_data, priority=5):
    """Submit a background job"""
    payload = json.dumps({
        "type": job_type,
        "data": job_data
    }).encode('utf-8')
    
    client.queue.publish("jobs", payload, priority=priority)
    print(f"Job submitted: {job_type}")
```

### Consumer (Worker)

```python
from synap_sdk import SynapClient
import json
import time

client = SynapClient("http://localhost:15500")
worker_id = "worker-1"

def process_job(job_data):
    """Process a job"""
    job_type = job_data.get("type")
    data = job_data.get("data")
    
    if job_type == "process_video":
        process_video(data)
    elif job_type == "send_email":
        send_email(data)
    elif job_type == "generate_report":
        generate_report(data)
    else:
        raise ValueError(f"Unknown job type: {job_type}")

while True:
    message = client.queue.consume("jobs", worker_id)
    
    if message:
        try:
            # Decode payload
            job_data = json.loads(bytes(message.payload).decode('utf-8'))
            
            # Process job
            process_job(job_data)
            
            # ACK on success
            client.queue.ack("jobs", message.message_id)
            print(f"Job processed: {job_data['type']}")
        except Exception as e:
            # NACK on error (will retry)
            client.queue.nack("jobs", message.message_id)
            print(f"Error processing job: {e}")
    else:
        # No messages, wait before next poll
        time.sleep(1)
```

## Priority-Based Processing

### High Priority Jobs

```python
# Critical job (priority 9)
client.queue.publish("jobs", critical_job_data, priority=9)

# Normal job (priority 5)
client.queue.publish("jobs", normal_job_data, priority=5)

# Background job (priority 0)
client.queue.publish("jobs", background_job_data, priority=0)
```

### Priority Worker

```python
# Worker processes high priority first
# (Queue automatically orders by priority)
message = client.queue.consume("jobs", worker_id)
```

## Retry Logic

### Configure Retries

```python
# Job with max 3 retries
client.queue.publish("jobs", job_data, priority=5, max_retries=3)
```

### Handle Retries

```python
message = client.queue.consume("jobs", worker_id)

if message:
    if message.retry_count > 0:
        print(f"Retry attempt {message.retry_count}")
    
    try:
        process_job(message.payload)
        client.queue.ack("jobs", message.message_id)
    except RetryableError as e:
        # Will retry
        client.queue.nack("jobs", message.message_id)
    except PermanentError as e:
        # Don't retry, ACK to move to DLQ
        client.queue.ack("jobs", message.message_id)
        log_error(e)
```

## Dead Letter Queue

### Monitor DLQ

```python
# Check DLQ count
stats = client.queue.stats("jobs")
print(f"DLQ count: {stats.dlq_count}")

# Consume from DLQ
dlq_message = client.queue.consume_dlq("jobs", worker_id)
if dlq_message:
    # Manual processing or alerting
    handle_failed_job(dlq_message)
```

## Multiple Workers

### Load Balancing

```python
# Worker 1
worker1 = SynapClient("http://localhost:15500")
message1 = worker1.queue.consume("jobs", "worker-1")

# Worker 2
worker2 = SynapClient("http://localhost:15500")
message2 = worker2.queue.consume("jobs", "worker-2")

# Each worker gets different messages
```

## Real-World Example

### Video Processing Service

```python
from synap_sdk import SynapClient
import json

client = SynapClient("http://localhost:15500")

# Producer: Submit video processing job
def submit_video_job(video_id, format="mp4", priority=5):
    job = {
        "video_id": video_id,
        "format": format,
        "timestamp": time.time()
    }
    
    payload = json.dumps(job).encode('utf-8')
    client.queue.publish("video-processing", payload, priority=priority)
    print(f"Video job submitted: {video_id}")

# Consumer: Process video
def process_video_worker():
    worker_id = "video-worker-1"
    
    while True:
        message = client.queue.consume("video-processing", worker_id)
        
        if message:
            try:
                job = json.loads(bytes(message.payload).decode('utf-8'))
                video_id = job["video_id"]
                format = job["format"]
                
                # Process video
                process_video_file(video_id, format)
                
                # ACK
                client.queue.ack("video-processing", message.message_id)
                print(f"Video processed: {video_id}")
            except Exception as e:
                # NACK (will retry)
                client.queue.nack("video-processing", message.message_id)
                print(f"Error processing video {video_id}: {e}")
        else:
            time.sleep(1)
```

## Best Practices

### Idempotent Workers

Make workers idempotent to handle retries safely:

```python
def process_job(job_id, job_data):
    # Check if already processed
    if client.kv.exists(f"processed:{job_id}"):
        return  # Already processed
    
    # Process job
    result = do_work(job_data)
    
    # Mark as processed
    client.kv.set(f"processed:{job_id}", "1", ttl=3600)
```

### Monitor Queue Depth

```python
# Monitor queue depth
stats = client.queue.stats("jobs")
if stats.pending > 1000:
    # Alert: Queue depth too high
    send_alert(f"Queue depth: {stats.pending}")
```

### Use Appropriate Priorities

- Priority 9: Critical, time-sensitive
- Priority 5: Normal operations
- Priority 0-2: Background, batch processing

## Related Topics

- [Creating Queues](../queues/CREATING.md) - Queue creation
- [Publishing Messages](../queues/PUBLISHING.md) - Publishing messages
- [Consuming Messages](../queues/CONSUMING.md) - Consuming messages

