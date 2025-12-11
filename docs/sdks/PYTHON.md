# Python SDK Documentation

## Overview

The Synap Python SDK provides both async and sync clients for Python 3.8+ applications.

## Installation

```bash
pip install synap-client
```

## Quick Start

### Async Client (Recommended)

```python
import asyncio
from synap import AsyncSynapClient

async def main():
    client = AsyncSynapClient(
        url='http://localhost:15500',
        api_key='synap_your_api_key'
    )
    
    # Key-Value operations
    await client.kv.set('user:1', {'name': 'Alice'}, ttl=3600)
    value = await client.kv.get('user:1')
    
    # Queue operations
    await client.queue.publish('tasks', {'type': 'send_email'})
    message = await client.queue.consume('tasks')
    await client.queue.ack('tasks', message.message_id)
    
    await client.close()

if __name__ == '__main__':
    asyncio.run(main())
```

### Sync Client

```python
from synap import SynapClient

client = SynapClient(
    url='http://localhost:15500',
    api_key='synap_your_api_key'
)

# Same API as async but without await
client.kv.set('user:1', {'name': 'Alice'}, ttl=3600)
value = client.kv.get('user:1')

client.close()
```

## Client Configuration

```python
from synap import AsyncSynapClient, ClientConfig

config = ClientConfig(
    url='http://localhost:15500',
    api_key='synap_key',
    timeout=30.0,             # Request timeout in seconds
    retries=3,                # Retry attempts
    pool_size=10,             # Connection pool size
    keep_alive=True,          # Keep-alive connections
    compression=False,        # Enable gzip compression
    format='json'             # 'json' or 'msgpack'
)

client = AsyncSynapClient(config)
```

## Key-Value API

### SET - Store Value

```python
await client.kv.set(
    key: str,
    value: Any,
    ttl: Optional[int] = None,
    nx: bool = False,
    xx: bool = False
) -> SetResult
```

**Example**:
```python
# Simple set
await client.kv.set('session:abc', {'user_id': 123})

# With TTL (1 hour)
await client.kv.set('cache:data', result, ttl=3600)

# Conditional set
result = await client.kv.set('lock:resource', True, ttl=60, nx=True)
if result.success:
    print('Lock acquired')
else:
    print('Lock already exists')
```

### GET - Retrieve Value

```python
await client.kv.get(key: str) -> GetResult
```

**Return Type**:
```python
@dataclass
class GetResult:
    found: bool
    value: Optional[Any]
    ttl: Optional[int]
```

**Example**:
```python
result = await client.kv.get('user:1001')

if result.found:
    print('Value:', result.value)
    print('TTL:', result.ttl)
else:
    print('Key not found')
```

### DEL - Delete Keys

```python
await client.kv.delete(*keys: str) -> int
```

**Example**:
```python
# Delete single key
await client.kv.delete('user:1001')

# Delete multiple keys
deleted = await client.kv.delete('user:1001', 'user:1002', 'user:1003')
print(f'Deleted {deleted} keys')
```

### INCR/DECR - Atomic Increment

```python
await client.kv.incr(key: str, amount: int = 1) -> int
await client.kv.decr(key: str, amount: int = 1) -> int
```

**Example**:
```python
# Increment page views
views = await client.kv.incr('article:123:views')

# Increment by custom amount
count = await client.kv.incr('counter', amount=5)

# Decrement inventory
remaining = await client.kv.decr('inventory:item:42')
```

### SCAN - Scan Keys

```python
await client.kv.scan(
    prefix: Optional[str] = None,
    cursor: Optional[str] = None,
    count: int = 100
) -> ScanResult
```

**Return Type**:
```python
@dataclass
class ScanResult:
    keys: List[str]
    cursor: Optional[str]
    has_more: bool
```

**Example**:
```python
# Scan all user keys
cursor = None
all_keys = []

while True:
    result = await client.kv.scan(prefix='user:', cursor=cursor, count=100)
    all_keys.extend(result.keys)
    
    if not result.has_more:
        break
    
    cursor = result.cursor

print(f'Found {len(all_keys)} user keys')
```

## Queue API

### PUBLISH - Add Message

```python
await client.queue.publish(
    queue: str,
    message: Any,
    priority: int = 5,
    headers: Optional[Dict[str, str]] = None
) -> PublishResult
```

**Return Type**:
```python
@dataclass
class PublishResult:
    message_id: str
    position: int
```

**Example**:
```python
result = await client.queue.publish(
    'tasks',
    {
        'type': 'send_email',
        'to': 'user@example.com',
        'subject': 'Welcome!'
    },
    priority=8,
    headers={'source': 'signup-flow'}
)

print(f'Published message {result.message_id} at position {result.position}')
```

### CONSUME - Get Message

```python
await client.queue.consume(
    queue: str,
    timeout: int = 0,
    ack_deadline: int = 30
) -> Optional[QueueMessage]
```

**Return Type**:
```python
@dataclass
class QueueMessage:
    message_id: str
    message: Any
    priority: int
    retry_count: int
    headers: Dict[str, str]
```

**Example**:
```python
# Poll for message (return immediately if none)
msg = await client.queue.consume('tasks')

# Wait up to 30 seconds
msg = await client.queue.consume('tasks', timeout=30)

if msg:
    try:
        # Process message
        await process_task(msg.message)
        
        # Acknowledge success
        await client.queue.ack('tasks', msg.message_id)
        
    except Exception as e:
        # Negative acknowledge (requeue)
        await client.queue.nack('tasks', msg.message_id, requeue=True)
```

### ACK - Acknowledge Message

```python
await client.queue.ack(queue: str, message_id: str) -> None
```

### NACK - Negative Acknowledge

```python
await client.queue.nack(
    queue: str,
    message_id: str,
    requeue: bool = True
) -> NackResult

@dataclass
class NackResult:
    success: bool
    action: str  # 'requeued' or 'dead_lettered'
```

## Event Stream API

### PUBLISH - Publish Event

```python
await client.stream.publish(
    room: str,
    event_type: str,
    data: Any,
    metadata: Optional[Dict[str, str]] = None
) -> PublishEventResult

@dataclass
class PublishEventResult:
    event_id: str
    offset: int
    subscribers_notified: int
```

**Example**:
```python
result = await client.stream.publish(
    'chat-room-1',
    'message',
    {
        'user': 'alice',
        'text': 'Hello everyone!',
        'timestamp': datetime.now().isoformat()
    }
)

print(f'Event {result.offset} sent to {result.subscribers_notified} subscribers')
```

### SUBSCRIBE - Subscribe to Room

```python
await client.stream.subscribe(
    room: str,
    callback: Callable[[StreamEvent], None],
    from_offset: Optional[int] = None,
    replay: bool = False
) -> Subscription
```

**Return Type**:
```python
@dataclass
class StreamEvent:
    event_id: str
    offset: int
    event_type: str
    data: Any
    timestamp: int

class Subscription:
    async def unsubscribe(self) -> None: ...
    @property
    def room(self) -> str: ...
```

**Example**:
```python
def on_event(event: StreamEvent):
    if event.event_type == 'message':
        print(f"{event.data['user']}: {event.data['text']}")

# Subscribe with history
subscription = await client.stream.subscribe(
    'chat-room-1',
    on_event,
    from_offset=-50,  # Last 50 events
    replay=True
)

# Later: unsubscribe
await subscription.unsubscribe()
```

### HISTORY - Get Event History

```python
await client.stream.history(
    room: str,
    from_offset: Optional[int] = None,
    to_offset: Optional[int] = None,
    limit: int = 100
) -> HistoryResult

@dataclass
class HistoryResult:
    events: List[StreamEvent]
    oldest_offset: int
    newest_offset: int
```

## Pub/Sub API

### PUBLISH - Publish Message

```python
await client.pubsub.publish(
    topic: str,
    message: Any,
    metadata: Optional[Dict[str, str]] = None
) -> PublishResult

@dataclass
class PublishResult:
    message_id: str
    topic: str
    subscribers_matched: int
```

**Example**:
```python
result = await client.pubsub.publish(
    'notifications.email.user',
    {
        'to': 'alice@example.com',
        'subject': 'Welcome!',
        'body': 'Thanks for signing up'
    }
)

print(f'Delivered to {result.subscribers_matched} subscribers')
```

### SUBSCRIBE - Subscribe to Topics

```python
await client.pubsub.subscribe(
    topics: List[str],
    callback: Callable[[str, Any], None]
) -> Subscription
```

**Example**:
```python
def on_message(topic: str, message: Any):
    print(f'[{topic}] {message}')

# Subscribe with wildcards
subscription = await client.pubsub.subscribe(
    ['notifications.email.*', 'events.user.#'],
    on_message
)

# Unsubscribe
await subscription.unsubscribe()
```

## Error Handling

### Exception Hierarchy

```python
class SynapError(Exception):
    """Base exception for all Synap errors"""
    def __init__(self, code: str, message: str, details: Optional[Dict] = None):
        self.code = code
        self.message = message
        self.details = details or {}

class KeyNotFoundError(SynapError):
    """Raised when key doesn't exist"""

class QueueNotFoundError(SynapError):
    """Raised when queue doesn't exist"""

class RoomNotFoundError(SynapError):
    """Raised when room doesn't exist"""

class RateLimitError(SynapError):
    """Raised when rate limit exceeded"""

class ConnectionError(SynapError):
    """Raised on network/connection errors"""
```

### Example Usage

```python
from synap import AsyncSynapClient, KeyNotFoundError, RateLimitError

client = AsyncSynapClient(url='http://localhost:15500')

try:
    value = await client.kv.get('nonexistent-key')
except KeyNotFoundError as e:
    print(f'Key not found: {e.details}')
except RateLimitError as e:
    print('Rate limited, retry after:', e.details.get('retry_after'))
except SynapError as e:
    print(f'Synap error [{e.code}]: {e.message}')
```

## Context Manager Support

```python
async with AsyncSynapClient(url='http://localhost:15500') as client:
    await client.kv.set('key', 'value')
    result = await client.kv.get('key')
# Automatically closes connection
```

## Type Hints

Full type hint support with Python 3.8+:

```python
from typing import Optional, Dict, Any, List
from synap import AsyncSynapClient, GetResult

async def get_user(client: AsyncSynapClient, user_id: int) -> Optional[Dict[str, Any]]:
    result: GetResult = await client.kv.get(f'user:{user_id}')
    return result.value if result.found else None
```

## Examples

### Cache Decorator

```python
from functools import wraps

def cached(ttl: int = 3600):
    def decorator(func):
        @wraps(func)
        async def wrapper(self, *args, **kwargs):
            cache_key = f'{func.__name__}:{args}:{kwargs}'
            
            # Try cache
            result = await self.client.kv.get(cache_key)
            if result.found:
                return result.value
            
            # Compute value
            value = await func(self, *args, **kwargs)
            
            # Store in cache
            await self.client.kv.set(cache_key, value, ttl=ttl)
            
            return value
        return wrapper
    return decorator

class UserService:
    def __init__(self, client: AsyncSynapClient):
        self.client = client
    
    @cached(ttl=300)
    async def get_user_profile(self, user_id: int) -> Dict:
        # Expensive database query
        return await fetch_from_database(user_id)
```

### Background Task Worker

```python
import asyncio
from synap import AsyncSynapClient

class TaskWorker:
    def __init__(self, client: AsyncSynapClient, queue_name: str):
        self.client = client
        self.queue_name = queue_name
        self.running = False
    
    async def start(self):
        self.running = True
        
        while self.running:
            try:
                # Wait for task (30 second timeout)
                msg = await self.client.queue.consume(
                    self.queue_name,
                    timeout=30,
                    ack_deadline=300
                )
                
                if not msg:
                    continue
                
                # Process task
                await self.process_task(msg.message)
                
                # Acknowledge completion
                await self.client.queue.ack(
                    self.queue_name,
                    msg.message_id
                )
                
            except Exception as e:
                print(f'Worker error: {e}')
                
                # NACK on failure
                if msg:
                    await self.client.queue.nack(
                        self.queue_name,
                        msg.message_id,
                        requeue=True
                    )
    
    async def process_task(self, task: Dict):
        # Task processing logic
        print(f'Processing task: {task}')
    
    def stop(self):
        self.running = False
```

### Event Stream Listener

```python
from synap import AsyncSynapClient, StreamEvent

class ChatListener:
    def __init__(self, client: AsyncSynapClient):
        self.client = client
        self.subscription = None
    
    async def join_room(self, room_id: str):
        def on_event(event: StreamEvent):
            if event.event_type == 'message':
                print(f"{event.data['user']}: {event.data['text']}")
            elif event.event_type == 'join':
                print(f"{event.data['user']} joined the room")
            elif event.event_type == 'leave':
                print(f"{event.data['user']} left the room")
        
        self.subscription = await self.client.stream.subscribe(
            f'chat-{room_id}',
            on_event,
            from_offset=-50,  # Last 50 messages
            replay=True
        )
    
    async def send_message(self, room_id: str, text: str):
        await self.client.stream.publish(
            f'chat-{room_id}',
            'message',
            {
                'user': self.get_username(),
                'text': text,
                'timestamp': datetime.now().isoformat()
            }
        )
    
    async def leave_room(self):
        if self.subscription:
            await self.subscription.unsubscribe()
```

### Pub/Sub Notifications

```python
class NotificationService:
    def __init__(self, client: AsyncSynapClient):
        self.client = client
    
    async def start_listening(self):
        await self.client.pubsub.subscribe(
            ['notifications.#'],  # All notifications
            self.handle_notification
        )
    
    def handle_notification(self, topic: str, message: Dict):
        if topic.startswith('notifications.email'):
            self.send_email(message)
        elif topic.startswith('notifications.sms'):
            self.send_sms(message)
        elif topic.startswith('notifications.push'):
            self.send_push(message)
    
    async def notify_email(self, to: str, subject: str, body: str):
        await self.client.pubsub.publish(
            'notifications.email.user',
            {
                'to': to,
                'subject': subject,
                'body': body,
                'timestamp': datetime.now().isoformat()
            }
        )
```

## Sync Client

### Usage

```python
from synap import SynapClient  # Note: not Async

client = SynapClient(url='http://localhost:15500', api_key='key')

# Same methods without async/await
client.kv.set('key', 'value')
value = client.kv.get('key')
client.queue.publish('tasks', {'data': 'value'})
```

### When to Use Sync

- Simple scripts
- Jupyter notebooks
- Integration with sync frameworks (Flask, Django)
- One-off administrative tasks

### When to Use Async

- High-performance servers (FastAPI, aiohttp)
- Concurrent operations
- WebSocket/streaming features
- Production applications

## Testing

### Fixtures

```python
import pytest
from synap import AsyncSynapClient

@pytest.fixture
async def synap_client():
    client = AsyncSynapClient(
        url='http://localhost:15500',
        api_key='test_key'
    )
    yield client
    await client.close()

@pytest.mark.asyncio
async def test_kv_operations(synap_client):
    # Test set and get
    await synap_client.kv.set('test-key', 'test-value')
    result = await synap_client.kv.get('test-key')
    
    assert result.found
    assert result.value == 'test-value'
```

### Mock Client

```python
from unittest.mock import AsyncMock
from synap import AsyncSynapClient

# Create mock
mock_client = AsyncMock(spec=AsyncSynapClient)

# Setup mock responses
mock_client.kv.get.return_value = GetResult(
    found=True,
    value={'name': 'Alice'},
    ttl=3600
)

# Use in tests
result = await mock_client.kv.get('user:1')
assert result.value['name'] == 'Alice'
```

## Connection Management

### Connection Pool

```python
# Pool is managed automatically
client = AsyncSynapClient(pool_size=20)

# Multiple concurrent requests use pool
await asyncio.gather(
    client.kv.get('key1'),
    client.kv.get('key2'),
    client.kv.get('key3')
)
```

### Explicit Close

```python
client = AsyncSynapClient(url='http://localhost:15500')

try:
    await client.kv.set('key', 'value')
finally:
    await client.close()  # Clean shutdown
```

### Reconnection

Automatic reconnection on connection failure:

```python
client = AsyncSynapClient(
    url='http://localhost:15500',
    retries=5,
    retry_delay=1.0,      # Initial delay (seconds)
    retry_backoff=2.0,    # Exponential backoff
    retry_max_delay=30.0  # Max delay
)
```

## Advanced Features

### Batch Operations

```python
results = await client.batch([
    {'command': 'kv.set', 'payload': {'key': 'user:1', 'value': 'Alice'}},
    {'command': 'kv.set', 'payload': {'key': 'user:2', 'value': 'Bob'}},
    {'command': 'kv.get', 'payload': {'key': 'user:1'}}
])

for result in results:
    if result.status == 'success':
        print('Success:', result.payload)
    else:
        print('Error:', result.error)
```

### Custom Serialization

```python
import msgpack

client = AsyncSynapClient(
    url='http://localhost:15500',
    format='msgpack'
)

# Values are serialized with MessagePack
await client.kv.set('binary-data', large_object)
```

### Event Handlers

```python
client = AsyncSynapClient(url='http://localhost:15500')

@client.on('connected')
async def on_connected():
    print('Connected to Synap')

@client.on('disconnected')
async def on_disconnected(reason: str):
    print(f'Disconnected: {reason}')

@client.on('error')
async def on_error(error: Exception):
    print(f'Error: {error}')
```

## Type Stubs

Full type hints available:

```python
from synap import (
    AsyncSynapClient,
    SynapClient,
    ClientConfig,
    SetResult,
    GetResult,
    ScanResult,
    QueueMessage,
    PublishResult,
    StreamEvent,
    Subscription,
    SynapError,
    KeyNotFoundError,
    QueueNotFoundError
)
```

## See Also

- [REST_API.md](../api/REST_API.md) - Complete API reference
- [TYPESCRIPT.md](TYPESCRIPT.md) - TypeScript SDK
- [RUST.md](RUST.md) - Rust SDK
- [TASK_QUEUE.md](../examples/TASK_QUEUE.md) - Task queue example

