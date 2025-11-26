import axios, { AxiosInstance, AxiosRequestConfig, AxiosResponse } from 'axios';

export interface ServerConfig {
  name: string;
  url: string;
  port?: number;
  apiKey?: string;
  timeout?: number;
}

export interface ApiResponse<T = any> {
  success: boolean;
  data?: T;
  error?: string;
  message?: string;
}

// Server Info types
export interface ServerInfo {
  server?: {
    synap_version: string;
    os: string;
    arch: string;
    uptime_seconds: number;
    tcp_port: number;
    connected_clients: number;
  };
  memory?: {
    used_memory: number;
    used_memory_human: string;
    used_memory_peak: number;
    used_memory_peak_human: string;
  };
  stats?: {
    total_connections_received: number;
    total_commands_processed: number;
    instantaneous_ops_per_sec: number;
    keyspace_hits: number;
    keyspace_misses: number;
    hit_rate: number;
  };
  replication?: {
    role: string;
    connected_replicas: number;
    master_host?: string;
    master_port?: number;
  };
  keyspace?: {
    total_keys: number;
    expires: number;
    avg_ttl: number;
  };
}

// KV Stats
export interface KvStats {
  total_keys: number;
  total_memory_bytes: number;
  operations: {
    gets: number;
    sets: number;
    dels: number;
    hits: number;
    misses: number;
  };
  hit_rate: number;
}

// Queue types
export interface QueueInfo {
  name: string;
  size: number;
  pending: number;
  processing: number;
  dead_letter_count?: number;
  created_at?: string;
}

export interface QueueStats {
  name: string;
  size: number;
  pending: number;
  processing: number;
  dead_letter_count: number;
  total_published: number;
  total_consumed: number;
  consumers: number;
}

// Stream types
export interface StreamInfo {
  room: string;
  partitions: number;
  subscribers: number;
  messages: number;
  created_at?: string;
}

export interface StreamStats {
  room: string;
  partitions: number;
  subscribers: number;
  total_messages: number;
  messages_per_partition: number[];
}

// Pub/Sub types
export interface PubsubStats {
  total_topics: number;
  total_subscribers: number;
  total_messages_published: number;
  topics: TopicInfo[];
}

export interface TopicInfo {
  name: string;
  subscribers: number;
  messages_published: number;
}

export class SynapApiClient {
  private client: AxiosInstance;
  private config: ServerConfig;

  constructor(config: ServerConfig) {
    this.config = config;
    const baseURL = config.port
      ? `${config.url}:${config.port}`
      : config.url;

    this.client = axios.create({
      baseURL,
      timeout: config.timeout || 30000,
      headers: {
        'Content-Type': 'application/json',
        ...(config.apiKey && { 'Authorization': `Bearer ${config.apiKey}` }),
      },
    });

    // Request interceptor
    this.client.interceptors.request.use(
      (config) => {
        return config;
      },
      (error) => {
        return Promise.reject(error);
      }
    );

    // Response interceptor
    this.client.interceptors.response.use(
      (response: AxiosResponse) => {
        // Return the response data directly
        return response;
      },
      (error) => {
        // Let the error propagate so methods can handle it
        return Promise.reject(error);
      }
    );
  }

  // KV Store operations - using correct OpenAPI endpoints
  async get(key: string): Promise<ApiResponse<any>> {
    try {
      const response = await this.client.get(`/kv/get/${encodeURIComponent(key)}`);
      console.log(`GET /kv/get/${key} raw response:`, response.data);
      
      // Response can be in different formats:
      // 1. { found: boolean, value: any, ttl?: number }
      // 2. { success, payload: { found, value, ttl } } (StreamableHTTP)
      // 3. Direct value (if found) or { error: "Key not found" }
      // 4. String with escaped JSON
      
      let data = response.data;
      
      // Handle StreamableHTTP format
      if (data.payload) {
        data = data.payload;
      }
      
      // Handle error response
      if (data.error) {
        return { success: false, error: data.error };
      }
      
      // Handle { found, value } format
      if (data.found !== undefined) {
        if (data.found && data.value !== undefined) {
          // Try to parse if it's a JSON string
          let value = data.value;
          if (typeof value === 'string' && value.startsWith('"') && value.endsWith('"')) {
            try {
              value = JSON.parse(value);
              // If still a string after parsing, try parsing again (double-encoded JSON)
              if (typeof value === 'string' && (value.startsWith('{') || value.startsWith('['))) {
                value = JSON.parse(value);
              }
            } catch (e) {
              // Keep original value if parsing fails
            }
          }
          return { success: true, data: value };
        }
        return { success: false, error: 'Key not found' };
      }
      
      // Handle direct value (if response is the value itself)
      if (data !== null && data !== undefined && typeof data !== 'object') {
        return { success: true, data: data };
      }
      
      // Handle string response (may be escaped JSON)
      if (typeof data === 'string') {
        try {
          // Try to parse JSON (may be double-encoded)
          let parsed = JSON.parse(data);
          
          // If parsed is still a string that looks like JSON, parse again
          if (typeof parsed === 'string') {
            const trimmed = parsed.trim();
            if ((trimmed.startsWith('{') && trimmed.endsWith('}')) || 
                (trimmed.startsWith('[') && trimmed.endsWith(']'))) {
              try {
                parsed = JSON.parse(parsed);
              } catch (e2) {
                // If second parse fails, use first parsed value
              }
            }
          }
          
          return { success: true, data: parsed };
        } catch (e) {
          // If not JSON, return as string
          return { success: true, data: data };
        }
      }
      
      // If we get here, assume it's a value
      return { success: true, data: data };
    } catch (error: any) {
      console.error(`GET /kv/get/${key} error:`, error.response?.data || error.message);
      const errorData = error.response?.data;
      if (errorData?.error) {
        return { success: false, error: errorData.error };
      }
      return { success: false, error: error.message || 'Failed to get key' };
    }
  }

  async set(key: string, value: string, ttl?: number): Promise<ApiResponse<boolean>> {
    try {
      const response = await this.client.post('/kv/set', {
        key,
        value,
        ttl,
      });
      return { success: true, data: response.data?.success !== false };
    } catch (error: any) {
      return { success: false, error: error.response?.data?.error || error.message || 'Failed to set key' };
    }
  }

  async delete(key: string): Promise<ApiResponse<boolean>> {
    try {
      const response = await this.client.delete(`/kv/del/${encodeURIComponent(key)}`);
      return { success: true, data: response.data?.deleted === true };
    } catch (error: any) {
      return { success: false, error: error.response?.data?.error || error.message || 'Failed to delete key' };
    }
  }

  async exists(key: string): Promise<ApiResponse<boolean>> {
    const response = await this.client.get(`/kv/${encodeURIComponent(key)}/exists`);
    return response.data;
  }

  async mset(pairs: Record<string, string>): Promise<ApiResponse<boolean>> {
    const response = await this.client.post('/kv/mset', pairs);
    return response.data;
  }

  async mget(keys: string[]): Promise<ApiResponse<Record<string, string>>> {
    const response = await this.client.post('/kv/mget', { keys });
    return response.data;
  }

  // Statistics
  async getStats(): Promise<ApiResponse<any>> {
    const response = await this.client.get('/stats');
    return response.data;
  }

  async getMetrics(): Promise<ApiResponse<any>> {
    const response = await this.client.get('/metrics');
    return response.data;
  }

  // Queue operations - using correct OpenAPI endpoints
  async queuePublish(queue: string, message: string, priority?: number): Promise<ApiResponse<boolean>> {
    try {
      // OpenAPI: payload is array of bytes, but we accept string and convert
      const payload = typeof message === 'string' 
        ? Array.from(new TextEncoder().encode(message))
        : message;
      
      const response = await this.client.post(`/queue/${encodeURIComponent(queue)}/publish`, {
        payload,
        priority: priority ?? 5,
      });
      return { success: true, data: response.data?.message_id !== undefined };
    } catch (error: any) {
      return { success: false, error: error.response?.data?.error || error.message || 'Failed to publish message' };
    }
  }

  async queueConsume(queue: string, timeout?: number): Promise<ApiResponse<string | null>> {
    const response = await this.client.post(`/queue/${encodeURIComponent(queue)}/consume`, {
      timeout,
    });
    return response.data;
  }

  async queueSize(queue: string): Promise<ApiResponse<number>> {
    const response = await this.client.get(`/queue/${encodeURIComponent(queue)}/size`);
    return response.data;
  }

  // Pub/Sub operations - using correct OpenAPI endpoints
  async pubsubPublish(topic: string, payload: string | object): Promise<ApiResponse<number>> {
    try {
      // OpenAPI: payload can be any JSON type, also accepts 'data' as alias
      const payloadData = typeof payload === 'string' ? JSON.parse(payload) : payload;
      const response = await this.client.post(`/pubsub/${encodeURIComponent(topic)}/publish`, {
        payload: payloadData,
      });
      return { success: true, data: response.data?.subscribers_notified || 0 };
    } catch (error: any) {
      return { success: false, error: error.response?.data?.error || error.message || 'Failed to publish to pubsub' };
    }
  }

  // Stream operations - using correct OpenAPI endpoints
  async streamCreate(room: string, partitions?: number): Promise<ApiResponse<boolean>> {
    try {
      // Simple stream (room-based) - POST /stream/{room}
      const response = await this.client.post(`/stream/${encodeURIComponent(room)}`);
      return { success: true, data: true };
    } catch (error: any) {
      return { success: false, error: error.response?.data?.error || error.message || 'Failed to create stream' };
    }
  }

  async streamPublish(room: string, partition: number, message: string): Promise<ApiResponse<boolean>> {
    try {
      // OpenAPI: POST /stream/{room}/publish with { event, data }
      const response = await this.client.post(`/stream/${encodeURIComponent(room)}/publish`, {
        event: 'message',
        data: typeof message === 'string' ? JSON.parse(message) : message,
      });
      return { success: true, data: response.data?.offset !== undefined };
    } catch (error: any) {
      return { success: false, error: error.response?.data?.error || error.message || 'Failed to publish to stream' };
    }
  }

  // Health check
  async healthCheck(): Promise<ApiResponse<{ status: string; version?: string }>> {
    try {
      const response = await this.client.get('/health');
      if (response.data && response.data.status === 'healthy') {
        return {
          success: true,
          data: {
            status: response.data.status,
            version: response.data.version,
          },
        };
      }
      return {
        success: false,
        error: 'Invalid health check response',
      };
    } catch (error: any) {
      return {
        success: false,
        error: error.response?.data?.error || error.message || 'Health check failed',
      };
    }
  }

  // Server Info - using admin.stats command (no REST /info endpoint exists)
  async getInfo(): Promise<ApiResponse<ServerInfo>> {
    try {
      const response = await this.executeCommand('admin.stats', {});
      console.log('admin.stats response:', response);
      
      if (response.success && response.data) {
        const payload = response.data.payload || response.data;
        // Transform admin.stats format to ServerInfo format
        const serverInfo: ServerInfo = {
          server: payload.server ? {
            synap_version: payload.server.version || '',
            os: payload.server.os || '',
            arch: payload.server.arch || '',
            uptime_seconds: payload.server.uptime_secs || 0,
            tcp_port: payload.server.tcp_port || 0,
            connected_clients: payload.server.connected_clients || 0,
          } : undefined,
          memory: payload.memory ? {
            used_memory: payload.memory.used_bytes || 0,
            used_memory_human: this.formatBytes(payload.memory.used_bytes || 0),
            used_memory_peak: payload.memory.peak_bytes || 0,
            used_memory_peak_human: this.formatBytes(payload.memory.peak_bytes || 0),
          } : undefined,
          stats: payload.kv ? {
            total_connections_received: payload.stats?.total_connections || 0,
            total_commands_processed: payload.stats?.total_commands || 0,
            instantaneous_ops_per_sec: payload.kv.operations_per_sec || 0,
            keyspace_hits: payload.stats?.keyspace_hits || 0,
            keyspace_misses: payload.stats?.keyspace_misses || 0,
            hit_rate: payload.stats?.hit_rate || 0,
          } : undefined,
          replication: payload.replication ? {
            role: payload.replication.role || 'master',
            connected_replicas: payload.replication.connected_replicas || 0,
            master_host: payload.replication.master_host,
            master_port: payload.replication.master_port,
          } : undefined,
          keyspace: payload.kv ? {
            total_keys: payload.kv.total_keys || 0,
            expires: payload.kv.expires || 0,
            avg_ttl: payload.kv.avg_ttl || 0,
          } : undefined,
        };
        return { success: true, data: serverInfo };
      }
      
      return { success: false, error: 'Invalid response from admin.stats' };
    } catch (error: any) {
      console.error('getInfo error:', error.response?.data || error.message);
      const errorMsg = error.response?.data?.error || error.response?.data?.message || error.message || 'Failed to get server info';
      return { success: false, error: errorMsg };
    }
  }


  // KV Stats
  async getKvStats(): Promise<ApiResponse<KvStats>> {
    try {
      const response = await this.client.get('/kv/stats');
      console.log('GET /kv/stats response:', response.data);
      return { success: true, data: response.data };
    } catch (error: any) {
      console.error('getKvStats error:', error.response?.data || error.message);
      const errorMsg = error.response?.data?.error || error.response?.data?.message || error.message || 'Failed to get KV stats';
      return { success: false, error: errorMsg };
    }
  }

  // Hash Stats
  async getHashStats(): Promise<ApiResponse<any>> {
    try {
      const response = await this.client.get('/hash/stats');
      return { success: true, data: response.data };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to get hash stats' };
    }
  }

  // List Stats
  async getListStats(): Promise<ApiResponse<any>> {
    try {
      const response = await this.client.get('/list/stats');
      return { success: true, data: response.data };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to get list stats' };
    }
  }

  // Set Stats
  async getSetStats(): Promise<ApiResponse<any>> {
    try {
      const response = await this.client.get('/set/stats');
      return { success: true, data: response.data };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to get set stats' };
    }
  }

  // Sorted Set Stats
  async getSortedSetStats(): Promise<ApiResponse<any>> {
    try {
      const response = await this.client.get('/sortedset/stats');
      return { success: true, data: response.data };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to get sorted set stats' };
    }
  }

  // Queue List
  async getQueueList(): Promise<ApiResponse<QueueInfo[]>> {
    try {
      const response = await this.client.get('/queue/list');
      console.log('GET /queue/list response:', response.data);
      
      // OpenAPI: { queues: string[] }
      // But we need QueueInfo[] with stats, so fetch stats for each queue
      const queueNames = response.data?.queues || [];
      
      if (queueNames.length === 0) {
        return { success: true, data: [] };
      }
      
      // Fetch stats for each queue to build QueueInfo[]
      const queueInfoPromises = queueNames.map(async (name: string) => {
        try {
          const statsRes = await this.getQueueStats(name);
          if (statsRes.success && statsRes.data) {
            const stats = statsRes.data;
            // size = total messages in queue (from QueueStats interface)
            // pending = messages waiting to be consumed
            // If pending is not provided in response, it means all messages are pending
            const totalSize = stats.size || stats.depth || 0;
            const pending = stats.pending !== undefined && stats.pending !== null
              ? stats.pending
              : totalSize; // If pending not provided, all messages are pending
            
            console.log(`Queue ${name}: size=${totalSize}, pending=${pending}, stats=`, stats);
            
            return {
              name,
              size: totalSize, // Total messages in queue
              pending: pending, // Messages waiting to be consumed
              processing: stats.processing || 0,
              dead_letter_count: stats.dead_letter_count || stats.dlq_count || 0,
            } as QueueInfo;
          }
        } catch (e) {
          console.warn(`Failed to get stats for queue ${name}:`, e);
        }
        // Fallback: return basic info
        return {
          name,
          size: 0,
          pending: 0,
          processing: 0,
          dead_letter_count: 0,
        } as QueueInfo;
      });
      
      const queueInfos = await Promise.all(queueInfoPromises);
      return { success: true, data: queueInfos };
    } catch (error: any) {
      console.error('getQueueList error:', error.response?.data || error.message);
      const errorMsg = error.response?.data?.error || error.response?.data?.message || error.message || 'Failed to get queue list';
      return { success: false, error: errorMsg };
    }
  }

  // Queue Stats
  async getQueueStats(name: string): Promise<ApiResponse<QueueStats>> {
    try {
      const response = await this.client.get(`/queue/${encodeURIComponent(name)}/stats`);
      console.log(`Queue stats raw response for ${name}:`, response.data);
      
      // Process response - handle different response formats
      const data = response.data;
      const depth = data.depth || data.size || 0;
      
      // pending = messages waiting to be consumed (not yet consumed)
      // If pending is not provided or is 0 but depth > 0, it means all messages are pending
      // (they haven't been consumed yet)
      let pending = data.pending;
      if (pending === undefined || pending === null) {
        // If pending not provided, assume all depth is pending (not consumed)
        pending = depth;
      } else if (pending === 0 && depth > 0) {
        // If pending is 0 but depth > 0, it might mean messages were consumed but not acked
        // In this case, pending should be depth (all messages are waiting)
        pending = depth;
      }
      
      const stats: QueueStats = {
        name,
        size: depth,
        pending: pending,
        processing: data.processing || 0,
        dead_letter_count: data.dlq_count || data.dead_letter_count || 0,
        total_published: data.total_published || data.published_total || 0,
        total_consumed: data.total_consumed || data.consumed_total || 0,
        consumers: data.consumers || 0,
      };
      
      console.log(`Queue stats processed for ${name}:`, { raw: data, processed: stats });
      
      return { success: true, data: stats };
    } catch (error: any) {
      console.error(`Failed to get queue stats for ${name}:`, error);
      return { success: false, error: error.message || 'Failed to get queue stats' };
    }
  }

  // Queue Create
  async queueCreate(name: string): Promise<ApiResponse<boolean>> {
    try {
      const response = await this.client.post(`/queue/${encodeURIComponent(name)}`);
      return { success: true, data: response.data.success || true };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to create queue' };
    }
  }

  // Queue Delete
  async queueDelete(name: string): Promise<ApiResponse<boolean>> {
    try {
      await this.client.delete(`/queue/${encodeURIComponent(name)}`);
      return { success: true, data: true };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to delete queue' };
    }
  }

  // Queue Purge
  async queuePurge(name: string): Promise<ApiResponse<boolean>> {
    try {
      await this.client.post(`/queue/${encodeURIComponent(name)}/purge`);
      return { success: true, data: true };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to purge queue' };
    }
  }

  // Stream List
  async getStreamList(): Promise<ApiResponse<StreamInfo[]>> {
    try {
      const response = await this.client.get('/stream/list');
      console.log('GET /stream/list response:', response.data);
      
      // OpenAPI: { rooms: string[], count: number }
      const roomNames = response.data?.rooms || [];
      
      if (roomNames.length === 0) {
        return { success: true, data: [] };
      }
      
      // Fetch stats for each room to build StreamInfo[]
      const streamInfoPromises = roomNames.map(async (room: string) => {
        try {
          const statsRes = await this.getStreamStats(room);
          if (statsRes.success && statsRes.data) {
            return {
              room,
              partitions: statsRes.data.partitions || 1,
              subscribers: statsRes.data.subscribers || 0,
              messages: statsRes.data.total_messages || 0,
            } as StreamInfo;
          }
        } catch (e) {
          console.warn(`Failed to get stats for stream ${room}:`, e);
        }
        // Fallback: return basic info
        return {
          room,
          partitions: 1,
          subscribers: 0,
          messages: 0,
        } as StreamInfo;
      });
      
      const streamInfos = await Promise.all(streamInfoPromises);
      return { success: true, data: streamInfos };
    } catch (error: any) {
      console.error('getStreamList error:', error.response?.data || error.message);
      const errorMsg = error.response?.data?.error || error.response?.data?.message || error.message || 'Failed to get stream list';
      return { success: false, error: errorMsg };
    }
  }

  // Stream Stats
  async getStreamStats(room: string): Promise<ApiResponse<StreamStats>> {
    try {
      const response = await this.client.get(`/stream/${encodeURIComponent(room)}/stats`);
      return { success: true, data: response.data };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to get stream stats' };
    }
  }

  // Stream Delete
  async streamDelete(room: string): Promise<ApiResponse<boolean>> {
    try {
      await this.client.delete(`/stream/${encodeURIComponent(room)}`);
      return { success: true, data: true };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to delete stream' };
    }
  }

  // Pub/Sub Stats
  async getPubsubStats(): Promise<ApiResponse<PubsubStats>> {
    try {
      const response = await this.client.get('/pubsub/stats');
      console.log('GET /pubsub/stats response:', response.data);
      return { success: true, data: response.data };
    } catch (error: any) {
      console.error('getPubsubStats error:', error.response?.data || error.message);
      const errorMsg = error.response?.data?.error || error.response?.data?.message || error.message || 'Failed to get pubsub stats';
      return { success: false, error: errorMsg };
    }
  }

  // Pub/Sub Topics List
  async getPubsubTopics(): Promise<ApiResponse<string[]>> {
    try {
      const response = await this.client.get('/pubsub/topics');
      return { success: true, data: response.data.topics || response.data };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to get pubsub topics' };
    }
  }

  // Pub/Sub Topic Info
  async getPubsubTopicInfo(topic: string): Promise<ApiResponse<TopicInfo>> {
    try {
      const response = await this.client.get(`/pubsub/${encodeURIComponent(topic)}/info`);
      return { success: true, data: response.data };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to get topic info' };
    }
  }

  // Key Type (using command endpoint since no REST endpoint exists)
  async keyType(key: string): Promise<ApiResponse<string>> {
    try {
      const response = await this.executeCommand('kv.type', { key });
      if (response.success && response.data) {
        const payload = response.data.payload || response.data;
        return { success: true, data: payload.type || payload.key_type || 'string' };
      }
      return { success: false, error: 'Failed to get key type' };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to get key type' };
    }
  }

  // Memory Usage (using command endpoint since no REST endpoint exists)
  async memoryUsage(key: string): Promise<ApiResponse<{ bytes: number }>> {
    try {
      const response = await this.executeCommand('kv.memory', { key });
      if (response.success && response.data) {
        const payload = response.data.payload || response.data;
        return { success: true, data: { bytes: payload.bytes || payload.memory || 0 } };
      }
      return { success: false, error: 'Failed to get memory usage' };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to get memory usage' };
    }
  }

  // Slowlog
  async getSlowlog(): Promise<ApiResponse<any[]>> {
    try {
      const response = await this.client.get('/slowlog');
      return { success: true, data: response.data.entries || response.data };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to get slowlog' };
    }
  }

  // Client List
  async getClientList(): Promise<ApiResponse<any[]>> {
    try {
      const response = await this.client.get('/clients');
      return { success: true, data: response.data.clients || response.data };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to get client list' };
    }
  }

  // Generic request method
  async request<T = any>(config: AxiosRequestConfig): Promise<ApiResponse<T>> {
    const response = await this.client.request(config);
    return response.data;
  }

  // Command endpoint - for commands not exposed as REST routes
  // Uses StreamableHTTP protocol format
  async executeCommand<T = any>(command: string, payload: Record<string, any> = {}): Promise<ApiResponse<T>> {
    try {
      const response = await this.client.post('/api/v1/command', {
        command,
        request_id: `req-${Date.now()}`,
        payload,
      });
      // Handle StreamableHTTP response format: { success, payload, error, request_id }
      if (response.data) {
        // If response already has success field (StreamableHTTP format)
        if (response.data.success !== undefined) {
          return {
            success: response.data.success,
            data: response.data.payload || response.data.data,
            error: response.data.error,
          };
        }
        // Otherwise wrap in success response
        return { success: true, data: response.data };
      }
      return { success: true, data: response.data };
    } catch (error: any) {
      const errorMsg = error.response?.data?.error || error.response?.data?.message || error.message || 'Command execution failed';
      return { success: false, error: errorMsg };
    }
  }

  // KV Keys - list all keys (using command endpoint since no REST endpoint exists)
  async getKvKeys(): Promise<ApiResponse<{ keys: string[]; count: number }>> {
    try {
      const response = await this.executeCommand('kv.keys', {});
      console.log('getKvKeys response:', response);
      
      // Handle StreamableHTTP format: { success, payload: { keys, count } }
      if (response.success && response.data) {
        const payload = response.data.payload || response.data;
        return {
          success: true,
          data: {
            keys: payload.keys || response.data.keys || [],
            count: payload.count || response.data.count || 0,
          },
        };
      }
      
      return response;
    } catch (error: any) {
      console.error('getKvKeys error:', error);
      return { success: false, error: error.message || 'Failed to get keys' };
    }
  }

  // KV Scan - scan keys with pattern
  async kvScan(prefix: string = '', limit: number = 100): Promise<ApiResponse<{ keys: string[]; count: number }>> {
    return this.executeCommand('kv.scan', { prefix, limit });
  }

  // Key TTL (using command endpoint since no REST endpoint exists)
  async getKeyTtl(key: string): Promise<ApiResponse<number>> {
    try {
      const response = await this.executeCommand('kv.ttl', { key });
      if (response.success && response.data) {
        // Handle StreamableHTTP format
        const payload = response.data.payload || response.data;
        const ttl = payload.ttl !== undefined ? payload.ttl : (response.data as any).ttl;
        return { success: true, data: ttl >= 0 ? ttl : -1 };
      }
      return { success: false, error: 'Failed to get TTL' };
    } catch (error: any) {
      return { success: false, error: error.message || 'Failed to get TTL' };
    }
  }

  // Update configuration
  updateConfig(config: Partial<ServerConfig>): void {
    this.config = { ...this.config, ...config };
    const baseURL = this.config.port
      ? `${this.config.url}:${this.config.port}`
      : this.config.url;

    this.client.defaults.baseURL = baseURL;
    this.client.defaults.timeout = this.config.timeout || 30000;
    
    if (this.config.apiKey) {
      this.client.defaults.headers.common['Authorization'] = `Bearer ${this.config.apiKey}`;
    }
  }

  getConfig(): ServerConfig {
    return { ...this.config };
  }

  // Helper method to format bytes
  private formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
  }
}

// Factory function
export function createApiClient(config: ServerConfig): SynapApiClient {
  return new SynapApiClient(config);
}

