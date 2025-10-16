# Real-Time Chat Sample

## Overview

This example demonstrates how to build a multi-room chat application using Synap's Event Stream system.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                 Chat Clients                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐      │
│  │  Alice   │  │   Bob    │  │  Carol   │      │
│  └──────────┘  └──────────┘  └──────────┘      │
└─────────────────────────────────────────────────┘
         │              │              │
         └──────────────┼──────────────┘
                        │ WebSocket
         ┌──────────────────────────────────┐
         │        Synap Server              │
         │                                  │
         │  Room: chat-room-1               │
         │  ├─ Event History (ring buffer)  │
         │  └─ Active Subscribers: 3        │
         └──────────────────────────────────┘
```

## Features

- **Multi-room Support**: Users can join multiple chat rooms
- **Message History**: New users see last 50 messages
- **Real-time Delivery**: Messages broadcast instantly to all room members
- **Presence**: Join/leave notifications
- **Typing Indicators**: Real-time typing status
- **User List**: Active users in room

## Implementation

### Backend (TypeScript)

```typescript
import { SynapClient } from '@hivellm/synap-client';
import express from 'express';
import http from 'http';

const app = express();
const server = http.createServer(app);

const synap = new SynapClient({
  url: 'http://localhost:15500',
  apiKey: process.env.SYNAP_API_KEY
});

// REST API for HTTP requests
app.use(express.json());

// Create or join room
app.post('/api/rooms/:roomId/join', async (req, res) => {
  const { roomId } = req.params;
  const { userId, username } = req.body;
  
  // Store user in room
  await synap.kv.set(
    `room:${roomId}:user:${userId}`,
    { username, joinedAt: Date.now() },
    3600  // 1 hour TTL
  );
  
  // Publish join event
  await synap.stream.publish(
    `chat-${roomId}`,
    'join',
    { userId, username, timestamp: Date.now() }
  );
  
  // Get message history
  const history = await synap.stream.history(
    `chat-${roomId}`,
    { fromOffset: -50, limit: 50 }
  );
  
  res.json({
    success: true,
    history: history.events
  });
});

// Send message
app.post('/api/rooms/:roomId/messages', async (req, res) => {
  const { roomId } = req.params;
  const { userId, username, text } = req.body;
  
  const result = await synap.stream.publish(
    `chat-${roomId}`,
    'message',
    {
      userId,
      username,
      text,
      timestamp: Date.now()
    }
  );
  
  res.json({
    success: true,
    eventId: result.eventId,
    offset: result.offset
  });
});

// Leave room
app.post('/api/rooms/:roomId/leave', async (req, res) => {
  const { roomId } = req.params;
  const { userId, username } = req.body;
  
  // Remove user from room
  await synap.kv.del(`room:${roomId}:user:${userId}`);
  
  // Publish leave event
  await synap.stream.publish(
    `chat-${roomId}`,
    'leave',
    { userId, username, timestamp: Date.now() }
  );
  
  res.json({ success: true });
});

// Get active users
app.get('/api/rooms/:roomId/users', async (req, res) => {
  const { roomId } = req.params;
  
  const result = await synap.kv.scan({
    prefix: `room:${roomId}:user:`,
    count: 100
  });
  
  const users = [];
  for (const key of result.keys) {
    const user = await synap.kv.get(key);
    if (user.found) {
      users.push(user.value);
    }
  }
  
  res.json({ users });
});

server.listen(3000, () => {
  console.log('Chat server running on port 3000');
});
```

### Frontend (TypeScript)

```typescript
import { SynapClient } from '@hivellm/synap-client';

class ChatClient {
  private synap: SynapClient;
  private currentRoom?: string;
  private subscription?: Subscription;
  
  constructor() {
    this.synap = new SynapClient({
      url: 'http://localhost:15500',
      apiKey: 'client_key'
    });
  }
  
  async joinRoom(roomId: string, userId: string, username: string) {
    // Get history via REST API
    const response = await fetch(`/api/rooms/${roomId}/join`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ userId, username })
    });
    
    const { history } = await response.json();
    
    // Display history
    history.forEach(event => this.displayEvent(event));
    
    // Subscribe to new events
    this.subscription = await this.synap.stream.subscribe(
      `chat-${roomId}`,
      (event) => this.displayEvent(event)
    );
    
    this.currentRoom = roomId;
  }
  
  async sendMessage(text: string, userId: string, username: string) {
    if (!this.currentRoom) return;
    
    await fetch(`/api/rooms/${this.currentRoom}/messages`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ userId, username, text })
    });
  }
  
  async leaveRoom(userId: string, username: string) {
    if (!this.currentRoom) return;
    
    await fetch(`/api/rooms/${this.currentRoom}/leave`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ userId, username })
    });
    
    if (this.subscription) {
      await this.subscription.unsubscribe();
    }
    
    this.currentRoom = undefined;
  }
  
  private displayEvent(event: any) {
    const { event_type, data } = event;
    
    switch (event_type) {
      case 'message':
        this.addMessage(data.username, data.text, data.timestamp);
        break;
      case 'join':
        this.addSystemMessage(`${data.username} joined the room`);
        break;
      case 'leave':
        this.addSystemMessage(`${data.username} left the room`);
        break;
    }
  }
  
  private addMessage(username: string, text: string, timestamp: number) {
    const messageDiv = document.createElement('div');
    messageDiv.className = 'message';
    messageDiv.innerHTML = `
      <span class="username">${username}</span>
      <span class="text">${text}</span>
      <span class="time">${new Date(timestamp).toLocaleTimeString()}</span>
    `;
    document.getElementById('messages').appendChild(messageDiv);
  }
  
  private addSystemMessage(text: string) {
    const div = document.createElement('div');
    div.className = 'system-message';
    div.textContent = text;
    document.getElementById('messages').appendChild(div);
  }
}

// Usage
const chat = new ChatClient();

document.getElementById('join-btn').onclick = async () => {
  const roomId = document.getElementById('room-input').value;
  const userId = localStorage.getItem('userId');
  const username = localStorage.getItem('username');
  
  await chat.joinRoom(roomId, userId, username);
};

document.getElementById('send-btn').onclick = async () => {
  const text = document.getElementById('message-input').value;
  const userId = localStorage.getItem('userId');
  const username = localStorage.getItem('username');
  
  await chat.sendMessage(text, userId, username);
  document.getElementById('message-input').value = '';
};
```

### Python Client

```python
import asyncio
from synap import AsyncSynapClient
from dataclasses import dataclass
from typing import Callable

@dataclass
class ChatMessage:
    username: str
    text: str
    timestamp: int

class ChatClient:
    def __init__(self, synap_url: str, api_key: str):
        self.client = AsyncSynapClient(url=synap_url, api_key=api_key)
        self.current_room = None
        self.subscription = None
    
    async def join_room(
        self,
        room_id: str,
        user_id: str,
        username: str,
        on_message: Callable
    ):
        # Get message history
        history = await self.client.stream.history(
            f'chat-{room_id}',
            from_offset=-50,
            limit=50
        )
        
        # Display history
        for event in history.events:
            if event.event_type == 'message':
                on_message(ChatMessage(**event.data))
        
        # Subscribe to new messages
        def event_handler(event):
            if event.event_type == 'message':
                on_message(ChatMessage(**event.data))
            elif event.event_type == 'join':
                print(f"{event.data['username']} joined")
            elif event.event_type == 'leave':
                print(f"{event.data['username']} left")
        
        self.subscription = await self.client.stream.subscribe(
            f'chat-{room_id}',
            event_handler
        )
        
        # Publish join event
        await self.client.stream.publish(
            f'chat-{room_id}',
            'join',
            {'user_id': user_id, 'username': username}
        )
        
        self.current_room = room_id
    
    async def send_message(self, username: str, text: str):
        if not self.current_room:
            return
        
        await self.client.stream.publish(
            f'chat-{self.current_room}',
            'message',
            {
                'username': username,
                'text': text,
                'timestamp': int(time.time() * 1000)
            }
        )
    
    async def leave_room(self, username: str):
        if not self.current_room:
            return
        
        # Publish leave event
        await self.client.stream.publish(
            f'chat-{self.current_room}',
            'leave',
            {'username': username}
        )
        
        # Unsubscribe
        if self.subscription:
            await self.subscription.unsubscribe()
        
        self.current_room = None

# Usage
async def main():
    chat = ChatClient(
        synap_url='http://localhost:15500',
        api_key='client_key'
    )
    
    def print_message(msg: ChatMessage):
        print(f'[{msg.timestamp}] {msg.username}: {msg.text}')
    
    await chat.join_room('room-1', 'user-123', 'Alice', print_message)
    
    # Send messages
    await chat.send_message('Alice', 'Hello everyone!')
    await asyncio.sleep(1)
    await chat.send_message('Alice', 'How is everyone doing?')
    
    # Keep running
    await asyncio.sleep(3600)
    
    await chat.leave_room('Alice')

if __name__ == '__main__':
    asyncio.run(main())
```

### Rust Client

```rust
use synap_client::{SynapClient, StreamEvent};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
struct ChatMessage {
    username: String,
    text: String,
    timestamp: u64,
}

pub struct ChatClient {
    client: SynapClient,
    current_room: Option<String>,
}

impl ChatClient {
    pub async fn new(url: &str, api_key: &str) -> Result<Self> {
        let client = SynapClient::connect(url).await?;
        Ok(Self { client, current_room: None })
    }
    
    pub async fn join_room(&mut self, room_id: &str, username: &str) -> Result<()> {
        // Get message history
        let history = self.client.stream_history(
            &format!("chat-{}", room_id),
            Some(-50),
            None,
            Some(50)
        ).await?;
        
        // Display history
        for event in history.events {
            if event.event_type == "message" {
                let msg: ChatMessage = serde_json::from_value(event.data)?;
                println!("[{}] {}: {}", event.offset, msg.username, msg.text);
            }
        }
        
        // Subscribe to new events
        self.client.stream_subscribe(
            &format!("chat-{}", room_id),
            |event: StreamEvent| async move {
                if event.event_type == "message" {
                    if let Ok(msg) = serde_json::from_value::<ChatMessage>(event.data) {
                        println!("{}: {}", msg.username, msg.text);
                    }
                }
            },
            None
        ).await?;
        
        // Publish join event
        self.client.stream_publish(
            &format!("chat-{}", room_id),
            "join",
            &json!({"username": username}),
            None
        ).await?;
        
        self.current_room = Some(room_id.to_string());
        Ok(())
    }
    
    pub async fn send_message(&self, username: &str, text: &str) -> Result<()> {
        if let Some(room) = &self.current_room {
            self.client.stream_publish(
                &format!("chat-{}", room),
                "message",
                &ChatMessage {
                    username: username.to_string(),
                    text: text.to_string(),
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                },
                None
            ).await?;
        }
        Ok(())
    }
    
    pub async fn leave_room(&mut self, username: &str) -> Result<()> {
        if let Some(room) = &self.current_room {
            // Publish leave event
            self.client.stream_publish(
                &format!("chat-{}", room),
                "leave",
                &json!({"username": username}),
                None
            ).await?;
            
            self.current_room = None;
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut chat = ChatClient::new(
        "http://localhost:15500",
        "client_key"
    ).await?;
    
    // Join room
    chat.join_room("general", "Alice").await?;
    
    // Send messages
    chat.send_message("Alice", "Hello everyone!").await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    chat.send_message("Alice", "How is everyone?").await?;
    
    // Keep running
    tokio::time::sleep(Duration::from_secs(3600)).await;
    
    // Leave room
    chat.leave_room("Alice").await?;
    
    Ok(())
}
```

## Advanced Features

### Typing Indicators

```typescript
// Publish typing status
async function setTyping(roomId: string, username: string, isTyping: boolean) {
  await synap.stream.publish(
    `chat-${roomId}`,
    'typing',
    {
      username,
      isTyping,
      timestamp: Date.now()
    }
  );
}

// Subscribe and handle typing
subscription = await synap.stream.subscribe(
  `chat-${roomId}`,
  (event) => {
    if (event.eventType === 'typing') {
      updateTypingIndicator(event.data.username, event.data.isTyping);
    }
  }
);
```

### User Presence

```typescript
// Store user status
await synap.kv.set(
  `user:${userId}:status`,
  { status: 'online', lastSeen: Date.now() },
  300  // 5 minute TTL
);

// Heartbeat to maintain presence
setInterval(async () => {
  await synap.kv.set(
    `user:${userId}:status`,
    { status: 'online', lastSeen: Date.now() },
    300
  );
}, 60000);  // Every minute
```

### Private Messages

```typescript
// Direct message between users
async function sendPrivateMessage(fromUser: string, toUser: string, text: string) {
  const roomId = [fromUser, toUser].sort().join('-');
  
  await synap.stream.publish(
    `dm-${roomId}`,
    'message',
    {
      from: fromUser,
      to: toUser,
      text,
      timestamp: Date.now()
    }
  );
}
```

### Message Reactions

```typescript
// Add reaction to message
await synap.stream.publish(
  `chat-${roomId}`,
  'reaction',
  {
    messageOffset: 42,  // Offset of message being reacted to
    userId: 'user-123',
    reaction: '👍'
  }
);
```

## Testing

### Unit Tests

```typescript
import { SynapClient } from '@hivellm/synap-client';
import { createMockClient } from '@hivellm/synap-client/testing';

describe('ChatClient', () => {
  it('should join room and receive history', async () => {
    const mock = createMockClient();
    
    mock.stream.history.mockResolvedValue({
      events: [
        { offset: 1, eventType: 'message', data: { text: 'Hi' } }
      ],
      oldestOffset: 1,
      newestOffset: 1
    });
    
    const chat = new ChatClient(mock);
    await chat.joinRoom('test-room', 'user-1', 'Alice');
    
    expect(mock.stream.history).toHaveBeenCalledWith('chat-test-room', expect.any(Object));
  });
});
```

### Integration Test

```python
import pytest
from synap import AsyncSynapClient

@pytest.mark.asyncio
async def test_chat_flow():
    client = AsyncSynapClient(url='http://localhost:15500')
    
    # Publish message
    result = await client.stream.publish(
        'chat-test',
        'message',
        {'username': 'test', 'text': 'hello'}
    )
    
    assert result.offset > 0
    
    # Get history
    history = await client.stream.history('chat-test', from_offset=0)
    assert len(history.events) >= 1
    assert history.events[-1].data['text'] == 'hello'
```

## Performance Considerations

### Message Batching

For high-frequency updates (typing indicators), batch events:

```typescript
const typingBuffer: string[] = [];

function onUserTyping(username: string) {
  typingBuffer.push(username);
}

// Flush every 500ms
setInterval(async () => {
  if (typingBuffer.length > 0) {
    await synap.stream.publish(
      `chat-${roomId}`,
      'typing_batch',
      { users: [...new Set(typingBuffer)] }
    );
    typingBuffer.length = 0;
  }
}, 500);
```

### Connection Limits

For large rooms (1000+ users):

```yaml
# server config
event_stream:
  max_subscribers_per_room: 10000
  broadcast_timeout_ms: 1000
```

## Scaling

### Multiple Rooms

- Each room is independent
- Rooms auto-cleanup when inactive
- 100K+ concurrent rooms supported

### Read Scaling

Deploy read replicas for message history:

```
Writes (publish) → Master
Reads (history)  → Replicas (load balanced)
Subscriptions    → Any node (WebSocket)
```

## See Also

- [EVENT_STREAM.md](../specs/EVENT_STREAM.md) - Event stream specification
- [EVENT_BROADCAST.md](EVENT_BROADCAST.md) - Broadcasting example
- [TYPESCRIPT.md](../sdks/TYPESCRIPT.md) - TypeScript SDK reference

