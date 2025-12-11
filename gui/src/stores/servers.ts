import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import type { ServerConfig } from '@/services/api';
import { SynapApiClient } from '@/services/api';

export interface Server extends ServerConfig {
  id: string;
  connected: boolean;
  lastConnected?: Date;
  status?: 'online' | 'offline' | 'connecting' | 'error';
  error?: string;
}

interface StoredServer {
  id: string;
  name: string;
  url: string;
  port?: number;
  apiKey?: string;
  timeout?: number;
}

const STORAGE_KEY = 'synap-desktop-servers';
const ACTIVE_SERVER_KEY = 'synap-desktop-active-server';

export const useServersStore = defineStore('servers', () => {
  const servers = ref<Map<string, Server>>(new Map());
  const activeServerId = ref<string | null>(null);
  const clients = ref<Map<string, SynapApiClient>>(new Map());
  let healthCheckInterval: NodeJS.Timeout | null = null;

  // Load from localStorage on initialization
  function loadFromStorage(): void {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        const serverList: StoredServer[] = JSON.parse(stored);
        serverList.forEach((storedServer) => {
          const server: Server = {
            ...storedServer,
            connected: false,
            status: 'offline',
          };
          servers.value.set(server.id, server);
          const client = new SynapApiClient({
            name: server.name,
            url: server.url,
            port: server.port,
            apiKey: server.apiKey,
            timeout: server.timeout,
          });
          clients.value.set(server.id, client);
        });
      }

      const activeId = localStorage.getItem(ACTIVE_SERVER_KEY);
      if (activeId && servers.value.has(activeId)) {
        activeServerId.value = activeId;
      }
    } catch (error) {
      console.error('Failed to load servers from storage:', error);
    }
  }

  // Save to localStorage
  function saveToStorage(): void {
    try {
      const serverList: StoredServer[] = Array.from(servers.value.values()).map((server) => ({
        id: server.id,
        name: server.name,
        url: server.url,
        port: server.port,
        apiKey: server.apiKey,
        timeout: server.timeout,
      }));
      localStorage.setItem(STORAGE_KEY, JSON.stringify(serverList));
      
      if (activeServerId.value) {
        localStorage.setItem(ACTIVE_SERVER_KEY, activeServerId.value);
      } else {
        localStorage.removeItem(ACTIVE_SERVER_KEY);
      }
    } catch (error) {
      console.error('Failed to save servers to storage:', error);
    }
  }

  // Initialize from storage
  loadFromStorage();

  const activeServer = computed(() => {
    if (!activeServerId.value) return null;
    return servers.value.get(activeServerId.value) || null;
  });

  const activeClient = computed((): SynapApiClient | null => {
    if (!activeServerId.value) return null;
    return (clients.value.get(activeServerId.value) as SynapApiClient | undefined) || null;
  });

  const serverList = computed(() => Array.from(servers.value.values()));

  function addServer(config: ServerConfig): string {
    const id = `server-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
    const server: Server = {
      ...config,
      id,
      connected: false,
      status: 'offline',
    };

    servers.value.set(id, server);
    const client = new SynapApiClient(config);
    clients.value.set(id, client);

    saveToStorage();
    return id;
  }

  function removeServer(id: string): void {
    servers.value.delete(id);
    clients.value.delete(id);
    if (activeServerId.value === id) {
      activeServerId.value = null;
    }
    saveToStorage();
  }

  function updateServer(id: string, updates: Partial<ServerConfig>): void {
    const server = servers.value.get(id);
    if (!server) return;

    const updated = { ...server, ...updates };
    servers.value.set(id, updated);

    const client = clients.value.get(id);
    if (client) {
      client.updateConfig(updates);
    }
    saveToStorage();
  }

  function setActiveServer(id: string | null): void {
    activeServerId.value = id;
    saveToStorage();
    // Start/stop health check polling
    if (id) {
      startHealthCheckPolling();
      // Immediately check connection
      connectServer(id);
    } else {
      stopHealthCheckPolling();
    }
  }

  async function connectServer(id: string): Promise<boolean> {
    const server = servers.value.get(id);
    if (!server) return false;

    server.status = 'connecting';
    server.error = undefined;

    try {
      const client = clients.value.get(id);
      if (!client) {
        throw new Error('Client not found');
      }

      const response = await client.healthCheck();
      if (response.success && response.data) {
        server.connected = true;
        server.status = 'online';
        server.lastConnected = new Date();
        server.error = undefined;
        return true;
      } else {
        const errorMsg = response.error || 'Health check failed';
        server.connected = false;
        server.status = 'offline';
        server.error = errorMsg;
        return false;
      }
    } catch (error: any) {
      server.connected = false;
      server.status = 'offline';
      server.error = error.message || error.error || 'Connection failed';
      return false;
    }
  }

  async function disconnectServer(id: string): Promise<void> {
    const server = servers.value.get(id);
    if (server) {
      server.connected = false;
      server.status = 'offline';
    }
    if (activeServerId.value === id) {
      stopHealthCheckPolling();
    }
  }

  async function testConnection(config: ServerConfig): Promise<boolean> {
    try {
      const client = new SynapApiClient(config);
      const response = await client.healthCheck();
      return response.success;
    } catch {
      return false;
    }
  }

  function getServer(id: string): Server | undefined {
    return servers.value.get(id);
  }

  function getClient(id: string): SynapApiClient | undefined {
    return clients.value.get(id) as SynapApiClient | undefined;
  }

  async function checkServerHealth(id: string): Promise<void> {
    const server = servers.value.get(id);
    if (!server) return;

    try {
      const client = clients.value.get(id);
      if (!client) {
        server.connected = false;
        server.status = 'offline';
        return;
      }

      const response = await client.healthCheck();
      if (response.success && response.data?.status === 'healthy') {
        if (!server.connected || server.status !== 'online') {
          server.connected = true;
          server.status = 'online';
          server.lastConnected = new Date();
          server.error = undefined;
        }
      } else {
        if (server.connected || server.status === 'online') {
          server.connected = false;
          server.status = 'offline';
          server.error = response.error || 'Health check failed';
        }
      }
    } catch (error: any) {
      if (server.connected || server.status === 'online') {
        server.connected = false;
        server.status = 'offline';
        server.error = error.message || error.error || 'Health check failed';
      }
    }
  }

  function startHealthCheckPolling(): void {
    stopHealthCheckPolling();
    healthCheckInterval = setInterval(() => {
      if (activeServerId.value) {
        checkServerHealth(activeServerId.value);
      }
    }, 10000); // Check every 10 seconds
  }

  function stopHealthCheckPolling(): void {
    if (healthCheckInterval) {
      clearInterval(healthCheckInterval);
      healthCheckInterval = null;
    }
  }

  return {
    servers,
    activeServerId,
    activeServer,
    activeClient,
    serverList,
    addServer,
    removeServer,
    updateServer,
    setActiveServer,
    connectServer,
    disconnectServer,
    testConnection,
    getServer,
    getClient,
    checkServerHealth,
    startHealthCheckPolling,
    stopHealthCheckPolling,
  };
});

