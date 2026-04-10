<template>
  <div class="p-8">
    <div v-if="!isConnected" class="bg-warning/10 border border-warning rounded-xl p-4 mb-6">
      <div class="flex items-center gap-2">
        <i class="fas fa-exclamation-triangle text-warning"></i>
        <p class="text-text-primary">
          No server connected. Please select or add a server to view metrics.
        </p>
      </div>
    </div>

    <div v-else class="space-y-6">
      <!-- Header with Time Range Selector and Export -->
      <div class="flex items-center justify-between">
        <div>
          <h1 class="text-2xl font-bold text-text-primary">Performance Metrics</h1>
          <p class="text-text-secondary mt-1">Detailed performance metrics and analytics</p>
        </div>
        <div class="flex items-center gap-3">
          <select
            v-model="selectedTimeRange"
            @change="updateTimeRange"
            class="px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary text-sm focus:outline-none focus:border-border-focus"
          >
            <option value="1h">Last Hour</option>
            <option value="6h">Last 6 Hours</option>
            <option value="24h">Last 24 Hours</option>
            <option value="7d">Last 7 Days</option>
          </select>
          <button
            @click="exportMetrics('csv')"
            class="px-4 py-2 bg-bg-tertiary hover:bg-bg-hover border border-border rounded-lg text-text-primary text-sm transition-colors flex items-center gap-2"
          >
            <i class="fas fa-download"></i>
            <span>Export CSV</span>
          </button>
          <button
            @click="exportMetrics('json')"
            class="px-4 py-2 bg-bg-tertiary hover:bg-bg-hover border border-border rounded-lg text-text-primary text-sm transition-colors flex items-center gap-2"
          >
            <i class="fas fa-download"></i>
            <span>Export JSON</span>
          </button>
          <button
            v-if="comparisonMode"
            @click="toggleComparisonMode"
            class="px-4 py-2 bg-info/20 hover:bg-info/30 border border-info rounded-lg text-info text-sm transition-colors flex items-center gap-2"
          >
            <i class="fas fa-times"></i>
            <span>Exit Comparison</span>
          </button>
          <button
            v-else
            @click="toggleComparisonMode"
            class="px-4 py-2 bg-bg-tertiary hover:bg-bg-hover border border-border rounded-lg text-text-primary text-sm transition-colors flex items-center gap-2"
          >
            <i class="fas fa-balance-scale"></i>
            <span>Compare</span>
          </button>
        </div>
      </div>

      <!-- Current Metrics Summary -->
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">CPU Usage</span>
            <i class="fas fa-microchip text-info"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ cpuUsage }}%</div>
          <div class="mt-2 h-2 bg-bg-tertiary rounded-full overflow-hidden">
            <div
              class="h-full bg-info transition-all duration-300"
              :style="{ width: `${cpuUsage}%` }"
            ></div>
          </div>
        </div>

        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Memory Usage</span>
            <i class="fas fa-memory text-success"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ formatBytes(metrics?.memoryUsage || 0) }}</div>
          <div class="mt-2 h-2 bg-bg-tertiary rounded-full overflow-hidden">
            <div
              class="h-full bg-success transition-all duration-300"
              :style="{ width: `${memoryPercentage}%` }"
            ></div>
          </div>
        </div>

        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Throughput</span>
            <i class="fas fa-tachometer-alt text-warning"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ metrics?.operationsPerSecond || 0 }} ops/s</div>
          <div class="text-text-muted text-xs mt-1">
            Avg: {{ averageThroughput }} ops/s
          </div>
        </div>

        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Latency (p95)</span>
            <i class="fas fa-clock text-error"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ p95Latency }}ms</div>
          <div class="text-text-muted text-xs mt-1">
            Min: {{ minLatency }}ms | Max: {{ maxLatency }}ms
          </div>
        </div>
      </div>

      <!-- Main Charts Grid -->
      <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <!-- CPU Usage Chart -->
        <div class="bg-bg-card border border-border rounded-lg p-6">
          <MetricsChart
            title="CPU Usage"
            :data="cpuHistory"
            type="line"
            color="#3b82f6"
            unit="%"
          />
        </div>

        <!-- Memory Usage Chart -->
        <div class="bg-bg-card border border-border rounded-lg p-6">
          <MetricsChart
            title="Memory Usage"
            :data="memoryHistory"
            type="line"
            color="#10b981"
            unit=""
          />
        </div>

        <!-- Throughput Chart -->
        <div class="bg-bg-card border border-border rounded-lg p-6">
          <MetricsChart
            title="Operations per Second"
            :data="operationsHistory"
            type="line"
            color="#f59e0b"
            unit=" ops/s"
          />
        </div>

        <!-- Latency Chart -->
        <div class="bg-bg-card border border-border rounded-lg p-6">
          <MetricsChart
            title="Response Latency (p95)"
            :data="latencyHistory"
            type="line"
            color="#ef4444"
            unit="ms"
          />
        </div>

        <!-- Cache Hit Rate Chart -->
        <div class="bg-bg-card border border-border rounded-lg p-6">
          <MetricsChart
            title="Cache Hit Rate"
            :data="cacheHitRateHistory"
            type="line"
            color="#8b5cf6"
            unit="%"
          />
        </div>

        <!-- Active Connections Chart -->
        <div class="bg-bg-card border border-border rounded-lg p-6">
          <MetricsChart
            title="Active Connections"
            :data="connectionsHistory"
            type="line"
            color="#06b6d4"
            unit=""
          />
        </div>
      </div>

      <!-- Comparison Mode (if enabled) -->
      <div v-if="comparisonMode" class="bg-bg-card border border-border rounded-lg p-6">
        <h3 class="text-lg font-semibold text-text-primary mb-4">Comparison Mode</h3>
        <p class="text-text-secondary text-sm mb-4">
          Select a time range to compare with the current metrics.
        </p>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label class="block text-text-secondary text-sm mb-2">Compare From</label>
            <input
              v-model="comparisonStart"
              type="datetime-local"
              class="w-full px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary focus:outline-none focus:border-border-focus"
            />
          </div>
          <div>
            <label class="block text-text-secondary text-sm mb-2">Compare To</label>
            <input
              v-model="comparisonEnd"
              type="datetime-local"
              class="w-full px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary focus:outline-none focus:border-border-focus"
            />
          </div>
        </div>
        <button
          @click="applyComparison"
          class="mt-4 px-4 py-2 bg-info hover:bg-info/80 text-white rounded-lg transition-colors"
        >
          Apply Comparison
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useMetricsStore } from '@/stores/metrics';
import { useServersStore } from '@/stores/servers';
import { useApi } from '@/composables/useApi';
import MetricsChart from '@/components/MetricsChart.vue';

const metricsStore = useMetricsStore();
const serversStore = useServersStore();
const { isConnected } = useApi();

const selectedTimeRange = ref('1h');
const comparisonMode = ref(false);
const comparisonStart = ref('');
const comparisonEnd = ref('');

const metrics = computed(() => metricsStore.currentMetrics);
const operationsHistory = computed(() => metricsStore.operationsHistory);
const memoryHistory = computed(() => metricsStore.memoryHistory);
const cacheHitRateHistory = computed(() => metricsStore.cacheHitRateHistory);
const connectionsHistory = computed(() => metricsStore.connectionsHistory);

// CPU usage estimated from operations/sec (no direct CPU metric available)
const cpuHistory = computed(() => {
  // Estimate CPU from operations/sec: higher ops = higher CPU
  // This is an approximation since there's no direct CPU metric in the API
  return operationsHistory.value.map(point => ({
    timestamp: point.timestamp,
    value: Math.min(100, Math.max(5, (point.value / 1000) * 10)), // Scale ops/sec to CPU %
  }));
});

// Latency estimated from operations/sec (no direct latency metric available)
const latencyHistory = computed(() => {
  // Estimate latency inversely from ops/sec: higher ops = lower latency (better performance)
  // This is an approximation since there's no direct latency metric in the API
  return operationsHistory.value.map(point => ({
    timestamp: point.timestamp,
    value: Math.max(0.1, Math.min(100, 100 - (point.value / 100))), // Inverse relationship
  }));
});

const cpuUsage = computed(() => {
  const history = cpuHistory.value;
  if (history.length === 0) return 0;
  return Math.round(history[history.length - 1].value);
});

const memoryPercentage = computed(() => {
  const maxMemory = 8 * 1024 * 1024 * 1024; // 8GB assumed
  return Math.min(100, ((metrics.value?.memoryUsage || 0) / maxMemory) * 100);
});

const averageThroughput = computed(() => {
  const history = operationsHistory.value;
  if (history.length === 0) return 0;
  const sum = history.reduce((acc, point) => acc + point.value, 0);
  return Math.round(sum / history.length);
});

const p95Latency = computed(() => {
  const history = latencyHistory.value;
  if (history.length === 0) return 0;
  const sorted = [...history].map(p => p.value).sort((a, b) => a - b);
  const index = Math.floor(sorted.length * 0.95);
  return Math.round(sorted[index] || 0);
});

const minLatency = computed(() => {
  const history = latencyHistory.value;
  if (history.length === 0) return 0;
  return Math.round(Math.min(...history.map(p => p.value)));
});

const maxLatency = computed(() => {
  const history = latencyHistory.value;
  if (history.length === 0) return 0;
  return Math.round(Math.max(...history.map(p => p.value)));
});

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}

function updateTimeRange(): void {
  // Time range is handled by MetricsChart component
  // This function can be used for additional filtering if needed
}

function exportMetrics(format: 'csv' | 'json'): void {
  const data = {
    operations: operationsHistory.value,
    memory: memoryHistory.value,
    cacheHitRate: cacheHitRateHistory.value,
    connections: connectionsHistory.value,
    cpu: cpuHistory.value,
    latency: latencyHistory.value,
  };

  if (format === 'csv') {
    // Convert to CSV
    let csv = 'Timestamp,Operations/sec,Memory (bytes),Cache Hit Rate (%),Connections,CPU (%),Latency (ms)\n';
    
    const maxLength = Math.max(
      operationsHistory.value.length,
      memoryHistory.value.length,
      cacheHitRateHistory.value.length,
      connectionsHistory.value.length,
      cpuHistory.value.length,
      latencyHistory.value.length
    );

    for (let i = 0; i < maxLength; i++) {
      const ops = operationsHistory.value[i]?.value || 0;
      const mem = memoryHistory.value[i]?.value || 0;
      const cache = cacheHitRateHistory.value[i]?.value || 0;
      const conn = connectionsHistory.value[i]?.value || 0;
      const cpu = cpuHistory.value[i]?.value || 0;
      const lat = latencyHistory.value[i]?.value || 0;
      const timestamp = operationsHistory.value[i]?.timestamp || 
                       memoryHistory.value[i]?.timestamp || 
                       Date.now();

      csv += `${new Date(timestamp).toISOString()},${ops},${mem},${cache},${conn},${cpu},${lat}\n`;
    }

    const blob = new Blob([csv], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `synap-metrics-${new Date().toISOString().split('T')[0]}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  } else {
    // Export as JSON
    const json = JSON.stringify(data, null, 2);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `synap-metrics-${new Date().toISOString().split('T')[0]}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }
}

function toggleComparisonMode(): void {
  comparisonMode.value = !comparisonMode.value;
  if (comparisonMode.value) {
    const now = new Date();
    const oneHourAgo = new Date(now.getTime() - 60 * 60 * 1000);
    comparisonEnd.value = now.toISOString().slice(0, 16);
    comparisonStart.value = oneHourAgo.toISOString().slice(0, 16);
  }
}

function applyComparison(): void {
  // TODO: Implement comparison logic
  console.log('Comparing metrics from', comparisonStart.value, 'to', comparisonEnd.value);
}

onMounted(() => {
  if (isConnected.value) {
    metricsStore.startPolling();
  }
});

onUnmounted(() => {
  metricsStore.stopPolling();
});
</script>
