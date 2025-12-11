import { ref, Ref } from 'vue';

export interface WebSocketConfig {
  url: string;
  port?: number;
  protocol?: string;
  reconnectInterval?: number;
  maxReconnectAttempts?: number;
}

export type WebSocketMessage = {
  type: string;
  data?: any;
  timestamp?: number;
};

export type WebSocketEventHandler = (message: WebSocketMessage) => void;

export class SynapWebSocketClient {
  private ws: WebSocket | null = null;
  private config: WebSocketConfig;
  private reconnectAttempts = 0;
  private reconnectTimer: NodeJS.Timeout | null = null;
  private eventHandlers: Map<string, Set<WebSocketEventHandler>> = new Map();
  private isManualClose = false;

  public isConnected: Ref<boolean> = ref(false);
  public isConnecting: Ref<boolean> = ref(false);

  constructor(config: WebSocketConfig) {
    this.config = {
      reconnectInterval: 3000,
      maxReconnectAttempts: 10,
      ...config,
    };
  }

  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      if (this.ws?.readyState === WebSocket.OPEN) {
        resolve();
        return;
      }

      this.isConnecting.value = true;
      this.isManualClose = false;

      const wsUrl = this.config.port
        ? `${this.config.url}:${this.config.port}`
        : this.config.url;

      const protocol = this.config.protocol || (wsUrl.startsWith('https') ? 'wss' : 'ws');
      const url = wsUrl.replace(/^https?/, protocol);

      try {
        this.ws = new WebSocket(url);

        this.ws.onopen = () => {
          this.isConnected.value = true;
          this.isConnecting.value = false;
          this.reconnectAttempts = 0;
          console.log('WebSocket connected');
          resolve();
        };

        this.ws.onmessage = (event) => {
          try {
            const message: WebSocketMessage = JSON.parse(event.data);
            this.handleMessage(message);
          } catch (error) {
            console.error('Failed to parse WebSocket message:', error);
          }
        };

        this.ws.onerror = (error) => {
          console.error('WebSocket error:', error);
          this.isConnecting.value = false;
          reject(error);
        };

        this.ws.onclose = () => {
          this.isConnected.value = false;
          this.isConnecting.value = false;
          console.log('WebSocket disconnected');

          if (!this.isManualClose && this.reconnectAttempts < (this.config.maxReconnectAttempts || 10)) {
            this.scheduleReconnect();
          }
        };
      } catch (error) {
        this.isConnecting.value = false;
        reject(error);
      }
    });
  }

  disconnect(): void {
    this.isManualClose = true;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.isConnected.value = false;
  }

  send(message: WebSocketMessage): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
    } else {
      console.warn('WebSocket is not connected. Message not sent:', message);
    }
  }

  // Subscribe to a specific message type
  on(eventType: string, handler: WebSocketEventHandler): () => void {
    if (!this.eventHandlers.has(eventType)) {
      this.eventHandlers.set(eventType, new Set());
    }
    this.eventHandlers.get(eventType)!.add(handler);

    // Return unsubscribe function
    return () => {
      const handlers = this.eventHandlers.get(eventType);
      if (handlers) {
        handlers.delete(handler);
        if (handlers.size === 0) {
          this.eventHandlers.delete(eventType);
        }
      }
    };
  }

  // Unsubscribe from a specific event type
  off(eventType: string, handler?: WebSocketEventHandler): void {
    if (handler) {
      const handlers = this.eventHandlers.get(eventType);
      if (handlers) {
        handlers.delete(handler);
        if (handlers.size === 0) {
          this.eventHandlers.delete(eventType);
        }
      }
    } else {
      this.eventHandlers.delete(eventType);
    }
  }

  private handleMessage(message: WebSocketMessage): void {
    // Call handlers for specific message type
    const typeHandlers = this.eventHandlers.get(message.type);
    if (typeHandlers) {
      typeHandlers.forEach((handler) => {
        try {
          handler(message);
        } catch (error) {
          console.error('Error in WebSocket event handler:', error);
        }
      });
    }

    // Call handlers for '*' (all messages)
    const allHandlers = this.eventHandlers.get('*');
    if (allHandlers) {
      allHandlers.forEach((handler) => {
        try {
          handler(message);
        } catch (error) {
          console.error('Error in WebSocket event handler:', error);
        }
      });
    }
  }

  private scheduleReconnect(): void {
    this.reconnectAttempts++;
    const delay = this.config.reconnectInterval || 3000;

    this.reconnectTimer = setTimeout(() => {
      console.log(`Attempting to reconnect (${this.reconnectAttempts}/${this.config.maxReconnectAttempts})...`);
      this.connect().catch((error) => {
        console.error('Reconnection failed:', error);
      });
    }, delay);
  }

  // Pub/Sub subscribe
  subscribe(topic: string, handler: (data: any) => void): () => void {
    this.send({
      type: 'pubsub.subscribe',
      data: { topic },
    });

    return this.on(`pubsub.${topic}`, (message) => {
      handler(message.data);
    });
  }

  // Pub/Sub unsubscribe
  unsubscribe(topic: string): void {
    this.send({
      type: 'pubsub.unsubscribe',
      data: { topic },
    });
    this.off(`pubsub.${topic}`);
  }

  // Stream subscribe
  subscribeStream(room: string, partition: number, consumerGroup: string, handler: (data: any) => void): () => void {
    this.send({
      type: 'stream.subscribe',
      data: { room, partition, consumer_group: consumerGroup },
    });

    return this.on(`stream.${room}.${partition}`, (message) => {
      handler(message.data);
    });
  }
}

// Factory function
export function createWebSocketClient(config: WebSocketConfig): SynapWebSocketClient {
  return new SynapWebSocketClient(config);
}

