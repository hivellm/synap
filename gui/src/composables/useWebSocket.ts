import { onUnmounted, ref, Ref } from 'vue';
import { SynapWebSocketClient, createWebSocketClient, type WebSocketConfig } from '@/services/websocket';
import { useServersStore } from '@/stores/servers';

export function useWebSocket(config?: WebSocketConfig) {
  const serversStore = useServersStore();
  const wsClient = ref<SynapWebSocketClient | null>(null);
  const isConnected = ref(false);
  const isConnecting = ref(false);

  function connect(serverConfig?: WebSocketConfig): Promise<void> {
    const activeServer = serversStore.activeServer;
    if (!activeServer && !config && !serverConfig) {
      return Promise.reject(new Error('No server configuration provided'));
    }

    const wsConfig: WebSocketConfig = serverConfig || config || {
      url: activeServer!.url,
      port: activeServer!.port,
    };

    if (wsClient.value) {
      wsClient.value.disconnect();
    }
    
    const client = createWebSocketClient(wsConfig);
    wsClient.value = client;

    // Sync reactive state
    isConnected.value = client.isConnected.value;
    isConnecting.value = client.isConnecting.value;

    // Watch for connection state changes
    const updateState = () => {
      isConnected.value = client.isConnected.value;
      isConnecting.value = client.isConnecting.value;
    };

    // Poll connection state (simple approach)
    const stateCheckInterval = setInterval(updateState, 100);

    return client.connect().then(() => {
      isConnected.value = true;
      isConnecting.value = false;
      clearInterval(stateCheckInterval);
    }).catch((error) => {
      isConnecting.value = false;
      clearInterval(stateCheckInterval);
      throw error;
    });
  }

  function disconnect(): void {
    if (wsClient.value) {
      wsClient.value.disconnect();
      isConnected.value = false;
      isConnecting.value = false;
    }
  }

  function send(message: { type: string; data?: any }): void {
    if (wsClient.value) {
      wsClient.value.send(message);
    }
  }

  function on(eventType: string, handler: (data: any) => void): () => void {
    if (!wsClient.value) {
      throw new Error('WebSocket client not initialized. Call connect() first.');
    }
    return wsClient.value.on(eventType, handler);
  }

  function off(eventType: string, handler?: (data: any) => void): void {
    if (wsClient.value) {
      wsClient.value.off(eventType, handler);
    }
  }

  // Pub/Sub helpers
  function subscribe(topic: string, handler: (data: any) => void): () => void {
    if (!wsClient.value) {
      throw new Error('WebSocket client not initialized. Call connect() first.');
    }
    return wsClient.value.subscribe(topic, handler);
  }

  function unsubscribe(topic: string): void {
    if (wsClient.value) {
      wsClient.value.unsubscribe(topic);
    }
  }

  // Stream helpers
  function subscribeStream(
    room: string,
    partition: number,
    consumerGroup: string,
    handler: (data: any) => void
  ): () => void {
    if (!wsClient.value) {
      throw new Error('WebSocket client not initialized. Call connect() first.');
    }
    return wsClient.value.subscribeStream(room, partition, consumerGroup, handler);
  }

  // Cleanup on unmount
  onUnmounted(() => {
    disconnect();
  });

  return {
    wsClient: wsClient as Ref<SynapWebSocketClient | null>,
    isConnected,
    isConnecting,
    connect,
    disconnect,
    send,
    on,
    off,
    subscribe,
    unsubscribe,
    subscribeStream,
  };
}

