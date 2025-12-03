---
title: Real-Time Chat
module: use-cases
id: real-time-chat
order: 3
description: Kafka replacement for real-time chat applications
tags: [use-cases, chat, kafka, streams, websocket]
---

# Real-Time Chat

Using Synap streams as a Kafka replacement for real-time chat applications.

## Overview

Synap streams provide:
- Offset-based consumption
- WebSocket support
- Partitioning for scale
- Event history

## Architecture

### Backend Server

```python
from synap_sdk import SynapClient
from fastapi import FastAPI, WebSocket
import json

app = FastAPI()
client = SynapClient("http://localhost:15500")
stream_name = "chat-room-1"

@app.on_event("startup")
async def startup():
    # Create stream if not exists
    try:
        client.stream.create(stream_name, partitions=1, retention_hours=24)
    except:
        pass  # Stream already exists

@app.websocket("/ws/chat/{room_id}")
async def websocket_endpoint(websocket: WebSocket, room_id: str):
    await websocket.accept()
    
    # Connect to Synap stream
    synap_ws = client.stream.websocket(stream_name, f"server-{room_id}", from_offset=0)
    
    # Forward messages from Synap to client
    async def forward_messages():
        async for event in synap_ws:
            await websocket.send_json({
                "type": event.event,
                "data": json.loads(event.data),
                "offset": event.offset,
                "timestamp": event.timestamp
            })
    
    # Forward messages from client to Synap
    async def forward_to_synap():
        while True:
            data = await websocket.receive_json()
            
            # Publish to stream
            client.stream.publish(
                stream_name,
                "message",
                json.dumps({
                    "user_id": data["user_id"],
                    "message": data["message"],
                    "timestamp": time.time()
                })
            )
    
    # Run both tasks
    await asyncio.gather(forward_messages(), forward_to_synap())
```

### Frontend Client

```javascript
// Connect to chat room
const ws = new WebSocket('ws://localhost:8000/ws/chat/room-1');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  
  if (msg.type === 'message') {
    const data = msg.data;
    displayMessage(data.user_id, data.message, msg.timestamp);
  }
};

// Send message
function sendMessage(userId, message) {
  ws.send(JSON.stringify({
    user_id: userId,
    message: message
  }));
}
```

## Direct WebSocket Connection

### Connect to Stream

```javascript
// Connect directly to Synap stream
const ws = new WebSocket('ws://localhost:15500/stream/chat-room-1/ws/user-123?from_offset=0');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  console.log('Event:', msg.event);
  console.log('Data:', msg.data);
  console.log('Offset:', msg.offset);
  
  // Display message
  displayMessage(msg.data);
};

// Send message via REST API
function sendMessage(message) {
  fetch('http://localhost:15500/stream/chat-room-1/publish', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      event: 'message',
      data: JSON.stringify({
        user_id: currentUserId,
        message: message,
        timestamp: Date.now()
      })
    })
  });
}
```

## Message History

### Load Recent Messages

```python
# Load last 50 messages
events = client.stream.consume(
    "chat-room-1",
    "user-123",
    from_offset=0,  # Start from beginning
    limit=50
)

# Display messages
for event in reversed(events):  # Most recent last
    data = json.loads(event.data)
    display_message(data["user_id"], data["message"], event.timestamp)
```

### Continue from Last Position

```python
# Store last offset
last_offset = get_last_offset("user-123")

# Continue from last position
events = client.stream.consume(
    "chat-room-1",
    "user-123",
    from_offset=last_offset + 1,
    limit=10
)

# Update last offset
for event in events:
    display_message(event)
    last_offset = event.offset
save_last_offset("user-123", last_offset)
```

## Multiple Chat Rooms

### Room Management

```python
def create_chat_room(room_id):
    """Create a new chat room"""
    stream_name = f"chat-room-{room_id}"
    client.stream.create(stream_name, partitions=1, retention_hours=24)
    return stream_name

def send_message(room_id, user_id, message):
    """Send message to room"""
    stream_name = f"chat-room-{room_id}"
    client.stream.publish(
        stream_name,
        "message",
        json.dumps({
            "user_id": user_id,
            "message": message,
            "timestamp": time.time()
        })
    )
```

## Typing Indicators

### Publish Typing Event

```python
def send_typing_indicator(room_id, user_id, is_typing):
    """Send typing indicator"""
    stream_name = f"chat-room-{room_id}"
    client.stream.publish(
        stream_name,
        "typing",
        json.dumps({
            "user_id": user_id,
            "is_typing": is_typing,
            "timestamp": time.time()
        })
    )
```

### Subscribe to Typing Events

```javascript
ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  
  if (msg.event === 'typing') {
    const data = JSON.parse(msg.data);
    showTypingIndicator(data.user_id, data.is_typing);
  } else if (msg.event === 'message') {
    const data = JSON.parse(msg.data);
    displayMessage(data.user_id, data.message);
  }
};
```

## User Presence

### User Join/Leave

```python
def user_joined(room_id, user_id):
    """User joined room"""
    stream_name = f"chat-room-{room_id}"
    client.stream.publish(
        stream_name,
        "user_joined",
        json.dumps({
            "user_id": user_id,
            "timestamp": time.time()
        })
    )

def user_left(room_id, user_id):
    """User left room"""
    stream_name = f"chat-room-{room_id}"
    client.stream.publish(
        stream_name,
        "user_left",
        json.dumps({
            "user_id": user_id,
            "timestamp": time.time()
        })
    )
```

## Best Practices

### Use Partition Keys for Ordering

```python
# Messages for same user go to same partition
client.stream.publish(
    "chat-room-1",
    "message",
    message_data,
    partition_key=f"user-{user_id}"  # Maintain order per user
)
```

### Handle Reconnection

```javascript
let lastOffset = 0;

function connect() {
  const ws = new WebSocket(`ws://localhost:15500/stream/chat-room-1/ws/user-123?from_offset=${lastOffset}`);
  
  ws.onmessage = (event) => {
    const msg = JSON.parse(event.data);
    lastOffset = msg.offset + 1;
    handleMessage(msg);
  };
  
  ws.onclose = () => {
    // Reconnect after delay
    setTimeout(connect, 1000);
  };
}

connect();
```

### Monitor Stream

```python
# Check stream statistics
stats = client.stream.stats("chat-room-1")
print(f"Messages: {stats.message_count}")
print(f"Subscribers: {stats.subscribers}")
```

## Related Topics

- [Creating Streams](../streams/CREATING.md) - Stream creation
- [Publishing Events](../streams/PUBLISHING.md) - Publishing events
- [Consuming Events](../streams/CONSUMING.md) - Consuming events

