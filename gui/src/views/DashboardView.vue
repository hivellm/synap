<template>
  <div class="p-6">
    <div v-if="!isConnected" class="bg-warning/10 border border-warning rounded-lg p-4 mb-6">
      <div class="flex items-center gap-3">
        <i class="fas fa-exclamation-triangle text-warning"></i>
        <p class="text-text-primary text-sm">
          No server connected. Please select or add a server to view metrics.
        </p>
      </div>
    </div>

    <div v-else class="space-y-6">
      <!-- Server Version Info -->
      <div v-if="metrics?.version" class="flex items-center gap-2 text-text-secondary text-sm">
        <i class="fas fa-server"></i>
        <span>Synap {{ metrics.version }}</span>
        <span class="text-text-muted">|</span>
        <span>Uptime: {{ formatUptime(metrics.uptime) }}</span>
      </div>

      <!-- Stats Cards Row 1 -->
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <StatsCard
          title="Operations/sec"
          :value="metrics?.operationsPerSecond || 0"
          icon="fas fa-bolt"
          variant="info"
        />
        <StatsCard
          title="Memory Usage"
          :value="metrics?.memoryUsageHuman || formatBytes(metrics?.memoryUsage || 0)"
          icon="fas fa-memory"
          variant="success"
        />
        <StatsCard
          title="Cache Hit Rate"
          :value="`${(metrics?.cacheHitRate || 0).toFixed(2)}%`"
          icon="fas fa-bullseye"
          variant="info"
          :subtitle="`${metrics?.keyspaceHits || 0} hits / ${metrics?.keyspaceMisses || 0} misses`"
        />
        <StatsCard
          title="Active Connections"
          :value="metrics?.activeConnections || 0"
          icon="fas fa-plug"
          variant="warning"
        />
      </div>

      <!-- Stats Cards Row 2 -->
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <StatsCard
          title="Total Keys"
          :value="metrics?.totalKeys || 0"
          icon="fas fa-key"
          variant="secondary"
        />
        <StatsCard
          title="Queues"
          :value="metrics?.totalQueues || 0"
          icon="fas fa-inbox"
          variant="info"
        />
        <StatsCard
          title="Streams"
          :value="metrics?.totalStreams || 0"
          icon="fas fa-stream"
          variant="success"
        />
        <StatsCard
          title="Pub/Sub Topics"
          :value="metrics?.totalPubsubTopics || 0"
          icon="fas fa-broadcast-tower"
          variant="warning"
          :subtitle="`${metrics?.totalPubsubSubscribers || 0} subscribers`"
        />
      </div>

      <!-- Charts -->
      <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <div class="bg-bg-card border border-border rounded-lg p-4">
          <MetricsChart
            title="Operations per Second"
            :data="operationsHistory"
            type="line"
            color="#3b82f6"
            unit=" ops"
          />
        </div>
        <div class="bg-bg-card border border-border rounded-lg p-4">
          <MetricsChart
            title="Memory Usage"
            :data="memoryHistory"
            type="line"
            color="#10b981"
            unit=" B"
          />
        </div>
        <div class="bg-bg-card border border-border rounded-lg p-4">
          <MetricsChart
            title="Cache Hit Rate"
            :data="cacheHitRateHistory"
            type="line"
            color="#8b5cf6"
            unit="%"
          />
        </div>
        <div class="bg-bg-card border border-border rounded-lg p-4">
          <MetricsChart
            title="Active Connections"
            :data="connectionsHistory"
            type="line"
            color="#f59e0b"
          />
        </div>
      </div>

      <!-- Server Health and Summary -->
      <div class="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <ServerHealthIndicator
          :metrics="metrics"
          :is-connected="isConnected"
        />
        <TopItemsSummary
          title="Queues"
          icon="fas fa-inbox"
          :items="topQueues"
          :format-value="(value: number | string) => `${value} msgs`"
        />
        <TopItemsSummary
          title="Streams"
          icon="fas fa-stream"
          :items="topStreams"
          :format-value="(value: number | string) => `${value} msgs`"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, watch } from 'vue';
import { useMetricsStore } from '@/stores/metrics';
import StatsCard from '@/components/StatsCard.vue';
import MetricsChart from '@/components/MetricsChart.vue';
import ServerHealthIndicator from '@/components/ServerHealthIndicator.vue';
import TopItemsSummary from '@/components/TopItemsSummary.vue';
import { useApi } from '@/composables/useApi';

const metricsStore = useMetricsStore();
const { isConnected } = useApi();

const metrics = computed(() => metricsStore.currentMetrics);
const operationsHistory = computed(() => metricsStore.operationsHistory);
const memoryHistory = computed(() => metricsStore.memoryHistory);
const cacheHitRateHistory = computed(() => metricsStore.cacheHitRateHistory);
const connectionsHistory = computed(() => metricsStore.connectionsHistory);

// Real data from API - queues sorted by size
const topQueues = computed(() => {
  const queues = metricsStore.queues || [];
  return queues
    .slice()
    .sort((a, b) => (b.size || 0) - (a.size || 0))
    .slice(0, 5)
    .map(q => ({ name: q.name, value: q.size || 0 }));
});

// Real data from API - streams sorted by message count
const topStreams = computed(() => {
  const streams = metricsStore.streams || [];
  return streams
    .slice()
    .sort((a, b) => (b.messages || 0) - (a.messages || 0))
    .slice(0, 5)
    .map(s => ({ name: s.room, value: s.messages || 0 }));
});

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}

function formatUptime(seconds: number): string {
  if (!seconds) return '0s';
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);
  
  if (days > 0) return `${days}d ${hours}h ${minutes}m`;
  if (hours > 0) return `${hours}h ${minutes}m ${secs}s`;
  if (minutes > 0) return `${minutes}m ${secs}s`;
  return `${secs}s`;
}

onMounted(() => {
  if (isConnected.value) {
    metricsStore.startPolling();
  }
});

onUnmounted(() => {
  metricsStore.stopPolling();
});

watch(isConnected, (connected) => {
  if (connected) {
    metricsStore.startPolling();
  } else {
    metricsStore.stopPolling();
  }
});
</script>
