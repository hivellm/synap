# Distributed Task Queue Sample

## Overview

This example demonstrates how to build a distributed task processing system using Synap's Queue System with acknowledgment and retry logic.

## Use Case

A video processing platform where:
- Users upload videos
- Videos are queued for processing
- Multiple workers process videos concurrently
- Failed jobs are retried with exponential backoff
- Dead letter queue handles permanently failed jobs

## Architecture

```
┌─────────────────────────────────────────────────┐
│              Task Producers                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐      │
│  │  Web App │  │   API    │  │  Mobile  │      │
│  └──────────┘  └──────────┘  └──────────┘      │
└─────────────────────────────────────────────────┘
         │              │              │
         └──────────────┼──────────────┘
                        │ PUBLISH
         ┌──────────────────────────────────┐
         │         Synap Server             │
         │                                  │
         │  Queue: video-processing         │
         │  ├─ Pending: 50 messages         │
         │  ├─ In-flight: 10 (being processed)
         │  └─ Dead Letter: 2               │
         └──────────────────────────────────┘
                        │ CONSUME
         ┌──────────────┼──────────────┐
         │              │              │
┌──────────────┐  ┌──────────┐  ┌──────────┐
│  Worker 1    │  │ Worker 2 │  │ Worker 3 │
│  (idle)      │  │(processing)│ │(processing)
└──────────────┘  └──────────┘  └──────────┘
```

## Task Types

```typescript
enum TaskType {
  TRANSCODE = 'transcode',
  THUMBNAIL = 'thumbnail',
  UPLOAD_TO_CDN = 'upload_cdn',
  NOTIFY_USER = 'notify_user',
}

interface VideoTask {
  type: TaskType;
  videoId: string;
  userId: string;
  settings?: {
    resolution?: string;
    format?: string;
    quality?: number;
  };
}
```

## Producer Implementation

### TypeScript Producer

```typescript
import { SynapClient } from '@hivellm/synap-client';

class VideoTaskProducer {
  private synap: SynapClient;
  
  constructor(synapUrl: string, apiKey: string) {
    this.synap = new SynapClient({ url: synapUrl, apiKey });
  }
  
  async queueTranscoding(
    videoId: string,
    userId: string,
    settings: any
  ): Promise<string> {
    const task: VideoTask = {
      type: TaskType.TRANSCODE,
      videoId,
      userId,
      settings
    };
    
    const result = await this.synap.queue.publish(
      'video-processing',
      task,
      {
        priority: this.calculatePriority(userId),
        headers: {
          'source': 'web-upload',
          'user_id': userId,
          'video_id': videoId
        }
      }
    );
    
    console.log(`Task ${result.messageId} queued at position ${result.position}`);
    return result.messageId;
  }
  
  async queueThumbnail(videoId: string, userId: string): Promise<string> {
    const result = await this.synap.queue.publish(
      'video-processing',
      {
        type: TaskType.THUMBNAIL,
        videoId,
        userId
      },
      { priority: 7 }  // Higher priority for thumbnails
    );
    
    return result.messageId;
  }
  
  private calculatePriority(userId: string): number {
    // Premium users get higher priority
    return isPremiumUser(userId) ? 9 : 5;
  }
}

// Usage
const producer = new VideoTaskProducer(
  'http://localhost:15500',
  process.env.SYNAP_API_KEY
);

app.post('/api/videos/upload', async (req, res) => {
  const { videoId, userId } = req.body;
  
  // Queue transcoding task
  const taskId = await producer.queueTranscoding(
    videoId,
    userId,
    { resolution: '1080p', format: 'mp4' }
  );
  
  // Queue thumbnail generation
  await producer.queueThumbnail(videoId, userId);
  
  res.json({
    success: true,
    taskId,
    status: 'queued'
  });
});
```

### Python Producer

```python
from synap import AsyncSynapClient
from dataclasses import dataclass
from enum import Enum

class TaskType(str, Enum):
    TRANSCODE = 'transcode'
    THUMBNAIL = 'thumbnail'
    UPLOAD_CDN = 'upload_cdn'
    NOTIFY_USER = 'notify_user'

@dataclass
class VideoTask:
    type: TaskType
    video_id: str
    user_id: str
    settings: dict = None

class VideoTaskProducer:
    def __init__(self, synap_url: str, api_key: str):
        self.client = AsyncSynapClient(url=synap_url, api_key=api_key)
    
    async def queue_transcoding(
        self,
        video_id: str,
        user_id: str,
        settings: dict
    ) -> str:
        task = VideoTask(
            type=TaskType.TRANSCODE,
            video_id=video_id,
            user_id=user_id,
            settings=settings
        )
        
        result = await self.client.queue.publish(
            'video-processing',
            task.__dict__,
            priority=self._calculate_priority(user_id),
            headers={
                'source': 'api',
                'user_id': user_id,
                'video_id': video_id
            }
        )
        
        print(f'Task {result.message_id} queued at position {result.position}')
        return result.message_id
    
    def _calculate_priority(self, user_id: str) -> int:
        return 9 if is_premium_user(user_id) else 5

# Usage
async def main():
    producer = VideoTaskProducer(
        'http://localhost:15500',
        os.getenv('SYNAP_API_KEY')
    )
    
    task_id = await producer.queue_transcoding(
        video_id='vid_123',
        user_id='user_456',
        settings={'resolution': '1080p', 'format': 'mp4'}
    )
    
    print(f'Queued task: {task_id}')
```

## Worker Implementation

### TypeScript Worker

```typescript
import { SynapClient, QueueMessage } from '@hivellm/synap-client';

class VideoWorker {
  private synap: SynapClient;
  private queueName: string;
  private running: boolean = false;
  private workerId: string;
  
  constructor(synapUrl: string, apiKey: string, queueName: string) {
    this.synap = new SynapClient({ url: synapUrl, apiKey });
    this.queueName = queueName;
    this.workerId = `worker-${process.pid}`;
  }
  
  async start() {
    this.running = true;
    console.log(`${this.workerId} started, waiting for tasks...`);
    
    while (this.running) {
      try {
        // Wait up to 30 seconds for task
        const msg = await this.synap.queue.consume(
          this.queueName,
          {
            timeout: 30,
            ackDeadline: 300  // 5 minutes to process
          }
        );
        
        if (!msg) continue;
        
        console.log(`${this.workerId} processing ${msg.messageId}`);
        
        // Process task
        await this.processTask(msg);
        
        // Acknowledge completion
        await this.synap.queue.ack(this.queueName, msg.messageId);
        console.log(`${this.workerId} completed ${msg.messageId}`);
        
      } catch (error) {
        console.error(`${this.workerId} error:`, error);
        
        // NACK on error (will retry)
        if (msg) {
          await this.synap.queue.nack(
            this.queueName,
            msg.messageId,
            true  // requeue
          );
        }
        
        // Brief pause before next task
        await new Promise(resolve => setTimeout(resolve, 1000));
      }
    }
  }
  
  private async processTask(msg: QueueMessage): Promise<void> {
    const task = msg.message as VideoTask;
    
    switch (task.type) {
      case TaskType.TRANSCODE:
        await this.transcodeVideo(task);
        break;
      case TaskType.THUMBNAIL:
        await this.generateThumbnail(task);
        break;
      case TaskType.UPLOAD_TO_CDN:
        await this.uploadToCDN(task);
        break;
      case TaskType.NOTIFY_USER:
        await this.notifyUser(task);
        break;
      default:
        throw new Error(`Unknown task type: ${task.type}`);
    }
  }
  
  private async transcodeVideo(task: VideoTask): Promise<void> {
    console.log(`Transcoding video ${task.videoId}`);
    
    // Simulate video transcoding (would use FFmpeg in reality)
    await new Promise(resolve => setTimeout(resolve, 5000));
    
    // Update status in KV store
    await this.synap.kv.set(
      `video:${task.videoId}:status`,
      { status: 'transcoded', completedAt: Date.now() },
      3600
    );
    
    // Queue next task (thumbnail generation)
    await this.synap.queue.publish(
      this.queueName,
      {
        type: TaskType.THUMBNAIL,
        videoId: task.videoId,
        userId: task.userId
      },
      { priority: 7 }
    );
  }
  
  private async generateThumbnail(task: VideoTask): Promise<void> {
    console.log(`Generating thumbnail for ${task.videoId}`);
    await new Promise(resolve => setTimeout(resolve, 2000));
    
    await this.synap.kv.set(
      `video:${task.videoId}:thumbnail`,
      { url: `https://cdn.example.com/thumb/${task.videoId}.jpg` },
      86400
    );
  }
  
  private async uploadToCDN(task: VideoTask): Promise<void> {
    console.log(`Uploading ${task.videoId} to CDN`);
    await new Promise(resolve => setTimeout(resolve, 3000));
  }
  
  private async notifyUser(task: VideoTask): Promise<void> {
    console.log(`Notifying user ${task.userId} about video ${task.videoId}`);
    
    // Use pub/sub to notify
    await this.synap.pubsub.publish(
      `notifications.user.${task.userId}`,
      {
        type: 'video_ready',
        videoId: task.videoId,
        message: 'Your video is ready!'
      }
    );
  }
  
  stop() {
    this.running = false;
  }
}

// Run worker
const worker = new VideoWorker(
  'http://localhost:15500',
  process.env.SYNAP_API_KEY,
  'video-processing'
);

// Graceful shutdown
process.on('SIGINT', () => {
  console.log('Shutting down gracefully...');
  worker.stop();
});

worker.start();
```

### Python Worker

```python
import asyncio
import signal
from synap import AsyncSynapClient, QueueMessage
from dataclasses import dataclass

class VideoWorker:
    def __init__(self, synap_url: str, api_key: str, queue_name: str):
        self.client = AsyncSynapClient(url=synap_url, api_key=api_key)
        self.queue_name = queue_name
        self.running = False
        self.worker_id = f'worker-{os.getpid()}'
    
    async def start(self):
        self.running = True
        print(f'{self.worker_id} started, waiting for tasks...')
        
        while self.running:
            try:
                # Wait for task
                msg = await self.client.queue.consume(
                    self.queue_name,
                    timeout=30,
                    ack_deadline=300
                )
                
                if not msg:
                    continue
                
                print(f'{self.worker_id} processing {msg.message_id}')
                
                # Process task
                await self.process_task(msg)
                
                # Acknowledge completion
                await self.client.queue.ack(
                    self.queue_name,
                    msg.message_id
                )
                
                print(f'{self.worker_id} completed {msg.message_id}')
                
            except Exception as e:
                print(f'{self.worker_id} error: {e}')
                
                if msg:
                    await self.client.queue.nack(
                        self.queue_name,
                        msg.message_id,
                        requeue=True
                    )
                
                await asyncio.sleep(1)
    
    async def process_task(self, msg: QueueMessage):
        task = msg.message
        task_type = task['type']
        
        if task_type == 'transcode':
            await self.transcode_video(task)
        elif task_type == 'thumbnail':
            await self.generate_thumbnail(task)
        elif task_type == 'upload_cdn':
            await self.upload_to_cdn(task)
        else:
            raise ValueError(f'Unknown task type: {task_type}')
    
    async def transcode_video(self, task: dict):
        video_id = task['video_id']
        print(f'Transcoding video {video_id}')
        
        # Simulate transcoding
        await asyncio.sleep(5)
        
        # Update status
        await self.client.kv.set(
            f'video:{video_id}:status',
            {'status': 'transcoded', 'completed_at': time.time()},
            ttl=3600
        )
    
    async def generate_thumbnail(self, task: dict):
        video_id = task['video_id']
        print(f'Generating thumbnail for {video_id}')
        await asyncio.sleep(2)
    
    async def upload_to_cdn(self, task: dict):
        video_id = task['video_id']
        print(f'Uploading {video_id} to CDN')
        await asyncio.sleep(3)
    
    def stop(self):
        self.running = False

# Run multiple workers
async def run_workers(count: int):
    workers = []
    
    for i in range(count):
        worker = VideoWorker(
            'http://localhost:15500',
            os.getenv('SYNAP_API_KEY'),
            'video-processing'
        )
        workers.append(asyncio.create_task(worker.start()))
    
    # Graceful shutdown
    loop = asyncio.get_event_loop()
    
    def signal_handler():
        for worker in workers:
            worker.stop()
    
    loop.add_signal_handler(signal.SIGINT, signal_handler)
    
    await asyncio.gather(*workers)

if __name__ == '__main__':
    asyncio.run(run_workers(count=3))
```

### Rust Worker

```rust
use synap_client::{SynapClient, QueueMessage};
use serde::{Serialize, Deserialize};
use tokio::signal;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum TaskType {
    Transcode,
    Thumbnail,
    UploadCdn,
    NotifyUser,
}

#[derive(Serialize, Deserialize, Debug)]
struct VideoTask {
    #[serde(rename = "type")]
    task_type: TaskType,
    video_id: String,
    user_id: String,
    settings: Option<serde_json::Value>,
}

pub struct VideoWorker {
    client: SynapClient,
    queue_name: String,
    worker_id: String,
    running: Arc<AtomicBool>,
}

impl VideoWorker {
    pub async fn new(synap_url: &str, queue_name: &str) -> Result<Self> {
        Ok(Self {
            client: SynapClient::connect(synap_url).await?,
            queue_name: queue_name.to_string(),
            worker_id: format!("worker-{}", std::process::id()),
            running: Arc::new(AtomicBool::new(false)),
        })
    }
    
    pub async fn start(&self) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);
        tracing::info!("{} started, waiting for tasks...", self.worker_id);
        
        while self.running.load(Ordering::SeqCst) {
            match self.process_next_task().await {
                Ok(processed) => {
                    if processed {
                        tracing::info!("{} completed task", self.worker_id);
                    }
                }
                Err(e) => {
                    etracing::info!("{} error: {}", self.worker_id, e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
        
        Ok(())
    }
    
    async fn process_next_task(&self) -> Result<bool> {
        // Consume message with 30s timeout
        let msg = self.client.queue_consume::<VideoTask>(
            &self.queue_name,
            Some(ConsumeOptions {
                timeout: Some(30),
                ack_deadline: Some(300),
            })
        ).await?;
        
        let Some(msg) = msg else {
            return Ok(false);
        };
        
        tracing::info!("{} processing {}", self.worker_id, msg.message_id);
        
        // Process based on task type
        let result = match msg.message.task_type {
            TaskType::Transcode => self.transcode_video(&msg.message).await,
            TaskType::Thumbnail => self.generate_thumbnail(&msg.message).await,
            TaskType::UploadCdn => self.upload_to_cdn(&msg.message).await,
            TaskType::NotifyUser => self.notify_user(&msg.message).await,
        };
        
        match result {
            Ok(_) => {
                // Acknowledge success
                self.client.queue_ack(&self.queue_name, &msg.message_id).await?;
            }
            Err(e) => {
                etracing::info!("Task processing failed: {}", e);
                
                // NACK (will retry)
                self.client.queue_nack(&self.queue_name, &msg.message_id, true).await?;
            }
        }
        
        Ok(true)
    }
    
    async fn transcode_video(&self, task: &VideoTask) -> Result<()> {
        tracing::info!("Transcoding video {}", task.video_id);
        
        // Simulate transcoding work
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Update status in KV store
        self.client.kv_set(
            &format!("video:{}:status", task.video_id),
            &json!({ "status": "transcoded", "completed_at": chrono::Utc::now().timestamp() }),
            Some(3600)
        ).await?;
        
        Ok(())
    }
    
    async fn generate_thumbnail(&self, task: &VideoTask) -> Result<()> {
        tracing::info!("Generating thumbnail for {}", task.video_id);
        tokio::time::sleep(Duration::from_secs(2)).await;
        Ok(())
    }
    
    async fn upload_to_cdn(&self, task: &VideoTask) -> Result<()> {
        tracing::info!("Uploading {} to CDN", task.video_id);
        tokio::time::sleep(Duration::from_secs(3)).await;
        Ok(())
    }
    
    async fn notify_user(&self, task: &VideoTask) -> Result<()> {
        tracing::info!("Notifying user {} about video {}", task.user_id, task.video_id);
        Ok(())
    }
    
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let worker = VideoWorker::new(
        "http://localhost:15500",
        "video-processing"
    ).await?;
    
    // Setup graceful shutdown
    let running = worker.running.clone();
    tokio::spawn(async move {
        signal::ctrl_c().await.unwrap();
        tracing::info!("\nShutting down gracefully...");
        running.store(false, Ordering::SeqCst);
    });
    
    worker.start().await?;
    Ok(())
}
```

## Retry Logic

### Exponential Backoff

Configure queue with retry settings:

```yaml
queue:
  video-processing:
    max_retries: 3
    ack_deadline_secs: 300
    retry_delays: [10, 60, 300]  # 10s, 1min, 5min
```

### Dead Letter Queue Handling

```typescript
// Monitor dead letter queue
async function monitorDeadLetterQueue() {
  const synap = new SynapClient({ url: 'http://localhost:15500' });
  
  setInterval(async () => {
    const stats = await synap.queue.stats('video-processing');
    
    if (stats.deadLetterCount > 0) {
      console.warn(`${stats.deadLetterCount} tasks in dead letter queue`);
      
      // Alert operations team
      await sendAlert({
        type: 'dead_letter_queue',
        queue: 'video-processing',
        count: stats.deadLetterCount
      });
    }
  }, 60000);  // Check every minute
}
```

## Task Progress Tracking

### Using KV Store

```typescript
// Worker updates progress
async function updateProgress(
  videoId: string,
  progress: number,
  status: string
) {
  await synap.kv.set(
    `video:${videoId}:progress`,
    { progress, status, updatedAt: Date.now() },
    300  // 5 minute TTL
  );
}

// Client checks progress
app.get('/api/videos/:videoId/progress', async (req, res) => {
  const { videoId } = req.params;
  
  const result = await synap.kv.get(`video:${videoId}:progress`);
  
  if (result.found) {
    res.json(result.value);
  } else {
    res.status(404).json({ error: 'Video not found' });
  }
});
```

## Load Balancing

### Multiple Workers

Workers automatically load balance:

```bash
# Start 5 workers
for i in {1..5}; do
  node worker.js &
done
```

Each worker consumes tasks independently, providing parallel processing.

### Priority Queues

High-priority tasks processed first:

```typescript
// Premium user task (priority 9)
await synap.queue.publish('video-processing', task, { priority: 9 });

// Regular user task (priority 5)
await synap.queue.publish('video-processing', task, { priority: 5 });
```

## Monitoring

### Queue Metrics

```typescript
async function monitorQueue(queueName: string) {
  const stats = await synap.queue.stats(queueName);
  
  console.log(`Queue: ${queueName}`);
  console.log(`  Depth: ${stats.depth}`);
  console.log(`  Consumers: ${stats.consumers}`);
  console.log(`  Published: ${stats.publishedTotal}`);
  console.log(`  Consumed: ${stats.consumedTotal}`);
  console.log(`  Acked: ${stats.ackedTotal}`);
  console.log(`  Dead Lettered: ${stats.deadLetteredTotal}`);
  console.log(`  Avg Wait Time: ${stats.avgWaitTimeMs}ms`);
}

setInterval(() => monitorQueue('video-processing'), 10000);
```

### Worker Health

```typescript
// Workers report health via KV store
setInterval(async () => {
  await synap.kv.set(
    `worker:${workerId}:heartbeat`,
    {
      status: 'healthy',
      tasksProcessed: processedCount,
      lastTask: lastTaskTimestamp
    },
    60  // 1 minute TTL
  );
}, 30000);  // Every 30 seconds
```

## Testing

### Integration Test

```typescript
describe('Task Queue', () => {
  it('should process task end-to-end', async () => {
    const synap = new SynapClient({ url: 'http://localhost:15500' });
    
    // Publish task
    const result = await synap.queue.publish('test-queue', {
      type: 'test_task',
      data: 'test'
    });
    
    expect(result.messageId).toBeDefined();
    
    // Consume task
    const msg = await synap.queue.consume('test-queue');
    expect(msg).toBeDefined();
    expect(msg.message.type).toBe('test_task');
    
    // Acknowledge
    await synap.queue.ack('test-queue', msg.messageId);
    
    // Queue should be empty
    const stats = await synap.queue.stats('test-queue');
    expect(stats.depth).toBe(0);
  });
});
```

## See Also

- [QUEUE_SYSTEM.md](../specs/QUEUE_SYSTEM.md) - Queue specification
- [PYTHON.md](../sdks/PYTHON.md) - Python SDK reference
- [TYPESCRIPT.md](../sdks/TYPESCRIPT.md) - TypeScript SDK reference

