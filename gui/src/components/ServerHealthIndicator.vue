<template>
  <div class="bg-bg-card border border-border rounded-lg p-4">
    <div class="flex items-center justify-between mb-4">
      <h3 class="text-sm font-semibold text-text-primary">Server Health</h3>
      <div class="flex items-center gap-2">
        <div class="w-2 h-2 rounded-full" :class="statusClass"></div>
        <span class="text-xs" :class="statusTextClass">{{ statusText }}</span>
      </div>
    </div>
    
    <div class="space-y-3">
      <!-- Uptime -->
      <div class="flex items-center justify-between">
        <span class="text-xs text-text-secondary">Uptime</span>
        <span class="text-xs text-text-primary font-mono">{{ formattedUptime }}</span>
      </div>

      <!-- Memory Usage -->
      <div>
        <div class="flex items-center justify-between mb-1">
          <span class="text-xs text-text-secondary">Memory</span>
          <span class="text-xs text-text-primary">{{ memoryPercent }}%</span>
        </div>
        <div class="w-full bg-bg-tertiary rounded-full h-1.5">
          <div
            class="h-1.5 rounded-full transition-all"
            :class="memoryColorClass"
            :style="{ width: `${memoryPercent}%` }"
          ></div>
        </div>
      </div>

      <!-- Cache Hit Rate -->
      <div>
        <div class="flex items-center justify-between mb-1">
          <span class="text-xs text-text-secondary">Cache Hit Rate</span>
          <span class="text-xs text-text-primary">{{ cacheHitRate }}%</span>
        </div>
        <div class="w-full bg-bg-tertiary rounded-full h-1.5">
          <div
            class="h-1.5 rounded-full transition-all bg-success"
            :style="{ width: `${cacheHitRate}%` }"
          ></div>
        </div>
      </div>

      <!-- Operations per Second -->
      <div class="flex items-center justify-between">
        <span class="text-xs text-text-secondary">Ops/sec</span>
        <span class="text-xs text-text-primary font-mono">{{ operationsPerSecond }}</span>
      </div>

      <!-- Active Connections -->
      <div class="flex items-center justify-between">
        <span class="text-xs text-text-secondary">Connections</span>
        <span class="text-xs text-text-primary font-mono">{{ activeConnections }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { ServerMetrics } from '@/stores/metrics';

interface Props {
  metrics: ServerMetrics | null;
  isConnected: boolean;
}

const props = defineProps<Props>();

const statusText = computed(() => {
  if (!props.isConnected) return 'Offline';
  if (!props.metrics) return 'Unknown';
  return 'Online';
});

const statusClass = computed(() => {
  if (!props.isConnected) return 'bg-error';
  if (!props.metrics) return 'bg-warning';
  return 'bg-success';
});

const statusTextClass = computed(() => {
  if (!props.isConnected) return 'text-error';
  if (!props.metrics) return 'text-warning';
  return 'text-success';
});

const formattedUptime = computed(() => {
  if (!props.metrics?.uptime) return '--';
  const seconds = props.metrics.uptime;
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);

  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
});

const memoryPercent = computed(() => {
  if (!props.metrics?.memoryUsage) return 0;
  return Math.min(100, Math.round((props.metrics.memoryUsage / (1024 * 1024 * 1024)) * 10));
});

const memoryColorClass = computed(() => {
  const percent = memoryPercent.value;
  if (percent < 50) return 'bg-success';
  if (percent < 80) return 'bg-warning';
  return 'bg-error';
});

const cacheHitRate = computed(() => {
  return parseFloat((props.metrics?.cacheHitRate || 0).toFixed(2));
});

const operationsPerSecond = computed(() => {
  return props.metrics?.operationsPerSecond || 0;
});

const activeConnections = computed(() => {
  return props.metrics?.activeConnections || 0;
});
</script>
