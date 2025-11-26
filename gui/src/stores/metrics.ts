import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { useServersStore } from './servers';
import type { ServerInfo, KvStats, QueueInfo, StreamInfo, PubsubStats } from '@/services/api';

export interface MetricPoint {
  timestamp: number;
  value: number;
}

export interface ServerMetrics {
  operationsPerSecond: number;
  memoryUsage: number;
  memoryUsageHuman: string;
  cacheHitRate: number;
  activeConnections: number;
  totalKeys: number;
  totalQueues: number;
  totalStreams: number;
  totalPubsubTopics: number;
  totalPubsubSubscribers: number;
  uptime: number;
  version: string;
  totalCommands: number;
  keyspaceHits: number;
  keyspaceMisses: number;
}

export interface TimeSeriesData {
  label: string;
  data: MetricPoint[];
  color?: string;
}

export const useMetricsStore = defineStore('metrics', () => {
  const currentMetrics = ref<ServerMetrics | null>(null);
  const metricsHistory = ref<Map<string, MetricPoint[]>>(new Map());
  const updateInterval = ref<number>(5000); // 5 seconds default
  const isPolling = ref<boolean>(false);
  let pollingTimer: NodeJS.Timeout | null = null;

  // Raw data from API
  const serverInfo = ref<ServerInfo | null>(null);
  const kvStats = ref<KvStats | null>(null);
  const queues = ref<QueueInfo[]>([]);
  const streams = ref<StreamInfo[]>([]);
  const pubsubStats = ref<PubsubStats | null>(null);

  const serversStore = useServersStore();

  const operationsHistory = computed(() => {
    return metricsHistory.value.get('operationsPerSecond') || [];
  });

  const memoryHistory = computed(() => {
    return metricsHistory.value.get('memoryUsage') || [];
  });

  const cacheHitRateHistory = computed(() => {
    return metricsHistory.value.get('cacheHitRate') || [];
  });

  const connectionsHistory = computed(() => {
    return metricsHistory.value.get('activeConnections') || [];
  });

  function startPolling(): void {
    if (isPolling.value) return;

    isPolling.value = true;
    fetchMetrics();

    pollingTimer = setInterval(() => {
      fetchMetrics();
    }, updateInterval.value);
  }

  function stopPolling(): void {
    isPolling.value = false;
    if (pollingTimer) {
      clearInterval(pollingTimer);
      pollingTimer = null;
    }
  }

  async function fetchMetrics(): Promise<void> {
    const client = serversStore.activeClient;
    if (!client) {
      currentMetrics.value = null;
      return;
    }

    try {
      // Fetch all data in parallel
      const [infoRes, kvRes, queueRes, streamRes, pubsubRes] = await Promise.all([
        client.getInfo(),
        client.getKvStats(),
        client.getQueueList(),
        client.getStreamList(),
        client.getPubsubStats(),
      ]);

      // Store raw data
      if (infoRes.success) {
        serverInfo.value = infoRes.data || null;
        console.log('Server Info:', serverInfo.value);
      } else {
        console.error('Failed to get server info:', infoRes.error);
      }
      
      if (kvRes.success) {
        kvStats.value = kvRes.data || null;
        console.log('KV Stats:', kvStats.value);
      } else {
        console.error('Failed to get KV stats:', kvRes.error);
      }
      
      if (queueRes.success) {
        queues.value = queueRes.data || [];
        console.log('Queues:', queues.value);
      } else {
        console.error('Failed to get queues:', queueRes.error);
      }
      
      if (streamRes.success) {
        streams.value = streamRes.data || [];
        console.log('Streams:', streams.value);
      } else {
        console.error('Failed to get streams:', streamRes.error);
      }
      
      if (pubsubRes.success) {
        pubsubStats.value = pubsubRes.data || null;
        console.log('PubSub Stats:', pubsubStats.value);
      } else {
        console.error('Failed to get pubsub stats:', pubsubRes.error);
      }

      // Build metrics from real data
      const info = serverInfo.value;
      const kv = kvStats.value;

      currentMetrics.value = {
        operationsPerSecond: info?.stats?.instantaneous_ops_per_sec || 0,
        memoryUsage: info?.memory?.used_memory || kv?.total_memory_bytes || 0,
        memoryUsageHuman: info?.memory?.used_memory_human || formatBytes(info?.memory?.used_memory || 0),
        cacheHitRate: info?.stats?.hit_rate || kv?.hit_rate || 0,
        activeConnections: info?.server?.connected_clients || 0,
        totalKeys: info?.keyspace?.total_keys || kv?.total_keys || 0,
        totalQueues: queues.value.length,
        totalStreams: streams.value.length,
        totalPubsubTopics: pubsubStats.value?.total_topics || 0,
        totalPubsubSubscribers: pubsubStats.value?.total_subscribers || 0,
        uptime: info?.server?.uptime_seconds || 0,
        version: info?.server?.synap_version || 'unknown',
        totalCommands: info?.stats?.total_commands_processed || 0,
        keyspaceHits: info?.stats?.keyspace_hits || kv?.operations?.hits || 0,
        keyspaceMisses: info?.stats?.keyspace_misses || kv?.operations?.misses || 0,
      };

      // Add to history
      const now = Date.now();
      addMetricPoint('operationsPerSecond', now, currentMetrics.value.operationsPerSecond);
      addMetricPoint('memoryUsage', now, currentMetrics.value.memoryUsage);
      addMetricPoint('cacheHitRate', now, currentMetrics.value.cacheHitRate);
      addMetricPoint('activeConnections', now, currentMetrics.value.activeConnections);
    } catch (error) {
      console.error('Failed to fetch metrics:', error);
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
  }

  function addMetricPoint(metric: string, timestamp: number, value: number): void {
    if (!metricsHistory.value.has(metric)) {
      metricsHistory.value.set(metric, []);
    }

    const history = metricsHistory.value.get(metric)!;
    history.push({ timestamp, value });

    // Keep only last 1000 points (or ~1.4 hours at 5s interval)
    if (history.length > 1000) {
      history.shift();
    }
  }

  function clearHistory(): void {
    metricsHistory.value.clear();
  }

  function clearHistoryForMetric(metric: string): void {
    metricsHistory.value.delete(metric);
  }

  function getTimeSeriesData(metric: string, maxPoints: number = 100): TimeSeriesData | null {
    const history = metricsHistory.value.get(metric);
    if (!history || history.length === 0) return null;

    // Get last N points
    const data = history.slice(-maxPoints);

    return {
      label: metric,
      data,
    };
  }

  function setUpdateInterval(interval: number): void {
    updateInterval.value = interval;
    if (isPolling.value) {
      stopPolling();
      startPolling();
    }
  }

  return {
    // State
    currentMetrics,
    metricsHistory,
    updateInterval,
    isPolling,
    // Raw data from API
    serverInfo,
    kvStats,
    queues,
    streams,
    pubsubStats,
    // Computed history
    operationsHistory,
    memoryHistory,
    cacheHitRateHistory,
    connectionsHistory,
    // Actions
    startPolling,
    stopPolling,
    fetchMetrics,
    clearHistory,
    clearHistoryForMetric,
    getTimeSeriesData,
    setUpdateInterval,
  };
});

