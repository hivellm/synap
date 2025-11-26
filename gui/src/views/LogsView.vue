<template>
  <div class="p-6 flex flex-col h-full">
    <div class="mb-6 flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold text-text-primary">Logs</h1>
        <p class="text-text-secondary mt-1">Real-time log streaming and search</p>
      </div>
      <div class="flex gap-3">
        <button
          @click="toggleStreaming"
          :class="[
            'px-4 py-2 rounded-lg text-sm transition-colors flex items-center gap-2',
            isStreaming
              ? 'bg-error hover:bg-error/80 text-white'
              : 'bg-success hover:bg-success/80 text-white'
          ]"
        >
          <i :class="['fas', isStreaming ? 'fa-pause' : 'fa-play']"></i>
          {{ isStreaming ? 'Pause' : 'Resume' }}
        </button>
        <button
          @click="clearLogs"
          class="px-4 py-2 bg-bg-tertiary hover:bg-bg-hover border border-border rounded-lg text-text-primary text-sm transition-colors flex items-center gap-2"
        >
          <i class="fas fa-trash"></i>
          Clear
        </button>
        <button
          @click="exportLogs"
          class="px-4 py-2 bg-info hover:bg-info/80 text-white rounded-lg text-sm transition-colors flex items-center gap-2"
        >
          <i class="fas fa-download"></i>
          Export
        </button>
      </div>
    </div>

    <div v-if="!isConnected" class="bg-warning/10 border border-warning rounded-xl p-4 mb-6">
      <div class="flex items-center gap-2">
        <i class="fas fa-exclamation-triangle text-warning"></i>
        <p class="text-text-primary">
          No server connected. Please select or add a server to view logs.
        </p>
      </div>
    </div>

    <div v-else class="flex-1 flex flex-col min-h-0 space-y-4">
      <!-- Filters -->
      <div class="flex flex-wrap gap-4 p-4 bg-bg-card border border-border rounded-xl">
        <!-- Log Level Filter -->
        <div class="flex items-center gap-2">
          <span class="text-sm text-text-secondary">Level:</span>
          <div class="flex gap-1">
            <button
              v-for="level in logLevels"
              :key="level.value"
              @click="toggleLevel(level.value)"
              :class="[
                'px-3 py-1 text-xs rounded-full transition-colors flex items-center gap-1',
                selectedLevels.includes(level.value)
                  ? level.activeClass
                  : 'bg-bg-tertiary text-text-muted hover:bg-bg-hover'
              ]"
            >
              <i :class="level.icon"></i>
              {{ level.label }}
            </button>
          </div>
        </div>

        <!-- Search -->
        <div class="flex-1 min-w-64">
          <div class="relative">
            <i class="fas fa-search absolute left-3 top-1/2 -translate-y-1/2 text-text-muted"></i>
            <input
              v-model="searchQuery"
              type="text"
              placeholder="Search logs..."
              class="w-full pl-10 pr-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary placeholder-text-muted focus:outline-none focus:border-border-focus text-sm"
            />
          </div>
        </div>

        <!-- Auto-scroll toggle -->
        <label class="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            v-model="autoScroll"
            class="w-4 h-4 rounded border-border bg-bg-tertiary"
          />
          <span class="text-sm text-text-secondary">Auto-scroll</span>
        </label>
      </div>

      <!-- Log entries -->
      <div
        ref="logContainer"
        class="flex-1 min-h-0 bg-bg-card border border-border rounded-xl overflow-auto font-mono text-sm"
      >
        <div v-if="filteredLogs.length === 0" class="flex items-center justify-center h-64 text-text-muted">
          <div class="text-center">
            <i class="fas fa-file-alt text-3xl mb-2 block"></i>
            <p>No log entries</p>
          </div>
        </div>
        <div v-else class="divide-y divide-border">
          <div
            v-for="(log, index) in filteredLogs"
            :key="index"
            :class="[
              'p-3 hover:bg-bg-hover transition-colors',
              getLevelBgClass(log.level)
            ]"
          >
            <div class="flex items-start gap-3">
              <span class="text-text-muted text-xs w-20 flex-shrink-0">
                {{ formatTime(log.timestamp) }}
              </span>
              <span
                :class="[
                  'px-2 py-0.5 text-xs rounded font-medium w-16 text-center flex-shrink-0',
                  getLevelClass(log.level)
                ]"
              >
                {{ log.level.toUpperCase() }}
              </span>
              <span class="text-text-secondary flex-1 break-all">
                <span v-if="log.source" class="text-text-muted">[{{ log.source }}]</span>
                {{ log.message }}
              </span>
            </div>
          </div>
        </div>
      </div>

      <!-- Stats bar -->
      <div class="flex items-center justify-between p-3 bg-bg-card border border-border rounded-xl text-sm">
        <div class="flex items-center gap-4">
          <span class="text-text-secondary">
            Showing <strong class="text-text-primary">{{ filteredLogs.length }}</strong> of
            <strong class="text-text-primary">{{ logs.length }}</strong> entries
          </span>
        </div>
        <div class="flex items-center gap-4 text-xs">
          <span class="text-error">{{ logCounts.error }} errors</span>
          <span class="text-warning">{{ logCounts.warn }} warnings</span>
          <span class="text-info">{{ logCounts.info }} info</span>
          <span class="text-text-muted">{{ logCounts.debug }} debug</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue';
import { useApi } from '@/composables/useApi';

interface LogEntry {
  timestamp: number;
  level: 'debug' | 'info' | 'warn' | 'error';
  message: string;
  source?: string;
}

const { apiClient, isConnected } = useApi();

const logs = ref<LogEntry[]>([]);
const searchQuery = ref('');
const selectedLevels = ref<string[]>(['debug', 'info', 'warn', 'error']);
const isStreaming = ref(true);
const autoScroll = ref(true);
const logContainer = ref<HTMLElement | null>(null);

const logLevels = [
  { value: 'error', label: 'Error', icon: 'fas fa-times-circle', activeClass: 'bg-error text-white' },
  { value: 'warn', label: 'Warn', icon: 'fas fa-exclamation-triangle', activeClass: 'bg-warning text-white' },
  { value: 'info', label: 'Info', icon: 'fas fa-info-circle', activeClass: 'bg-info text-white' },
  { value: 'debug', label: 'Debug', icon: 'fas fa-bug', activeClass: 'bg-bg-tertiary text-text-primary border border-border' },
];

const filteredLogs = computed(() => {
  return logs.value.filter((log) => {
    // Filter by level
    if (!selectedLevels.value.includes(log.level)) return false;

    // Filter by search query
    if (searchQuery.value) {
      const query = searchQuery.value.toLowerCase();
      return (
        log.message.toLowerCase().includes(query) ||
        (log.source && log.source.toLowerCase().includes(query))
      );
    }

    return true;
  });
});

const logCounts = computed(() => {
  const counts = { error: 0, warn: 0, info: 0, debug: 0 };
  for (const log of logs.value) {
    counts[log.level]++;
  }
  return counts;
});

let streamInterval: NodeJS.Timeout | null = null;
let eventSource: EventSource | null = null;

function toggleLevel(level: string) {
  const index = selectedLevels.value.indexOf(level);
  if (index === -1) {
    selectedLevels.value.push(level);
  } else {
    selectedLevels.value.splice(index, 1);
  }
}

function toggleStreaming() {
  isStreaming.value = !isStreaming.value;
  if (isStreaming.value) {
    startStreaming();
  } else {
    stopStreaming();
  }
}

function startStreaming() {
  if (eventSource || streamInterval) return;
  
  if (!apiClient.value) {
    console.error('No API client available');
    return;
  }

  // Try to use Server-Sent Events (SSE) for streaming logs
  // If SSE is not available, fall back to polling
  const serverUrl = apiClient.value.getConfig().url;
  const apiKey = apiClient.value.getConfig().apiKey;
  
  try {
    // Try SSE endpoint first (if server supports it)
    const sseUrl = `${serverUrl}/api/v1/logs/stream${apiKey ? `?api_key=${apiKey}` : ''}`;
    eventSource = new EventSource(sseUrl);
    
    eventSource.onmessage = (event) => {
      try {
        const logData = JSON.parse(event.data);
        addLog({
          timestamp: logData.timestamp || Date.now(),
          level: logData.level || 'info',
          message: logData.message || logData.text || '',
          source: logData.source || logData.component || 'server',
        });
      } catch (e) {
        // If not JSON, treat as plain text
        addLog({
          timestamp: Date.now(),
          level: 'info',
          message: event.data,
          source: 'server',
        });
      }
    };
    
    eventSource.onerror = (error) => {
      console.warn('SSE connection error, falling back to polling:', error);
      eventSource?.close();
      eventSource = null;
      // Fall back to polling
      startPolling();
    };
    
    eventSource.onopen = () => {
      console.log('SSE connection opened for logs');
    };
  } catch (error) {
    console.warn('SSE not supported, using polling:', error);
    // Fall back to polling if SSE is not available
    startPolling();
  }
}

function startPolling() {
  if (streamInterval) return;
  
  // Poll for logs using StreamableHTTP command
  const pollLogs = async () => {
    if (!isStreaming.value || !apiClient.value) return;
    
    try {
      // Try to get logs using admin command
      // Since there's no direct logs endpoint, we'll simulate activity logs
      // based on API activity
      
      // Option 1: Try admin.logs command (if exists)
      const response = await apiClient.value.executeCommand('admin.logs', {
        limit: 20,
        since: logs.value.length > 0 ? logs.value[logs.value.length - 1].timestamp : Date.now() - 60000,
      });
      
      if (response.success && response.data) {
        const payload = response.data.payload || response.data;
        const newLogs = Array.isArray(payload.logs) 
          ? payload.logs 
          : Array.isArray(payload) 
            ? payload 
            : Array.isArray(response.data.logs)
              ? response.data.logs
              : [];
        
        newLogs.forEach((log: any) => {
          addLog({
            timestamp: log.timestamp || log.time || Date.now(),
            level: (log.level || log.severity || 'info').toLowerCase(),
            message: log.message || log.text || log.msg || JSON.stringify(log),
            source: log.source || log.component || log.module || 'server',
          });
        });
      } else {
        // Fallback: Generate activity logs based on server stats
        // This simulates logs when the real endpoint doesn't exist
        const statsResponse = await apiClient.value.getInfo();
        if (statsResponse.success && statsResponse.data) {
          const info = statsResponse.data;
          const now = Date.now();
          
          // Only add log if we haven't added one recently (avoid spam)
          if (logs.value.length === 0 || (now - logs.value[logs.value.length - 1].timestamp) > 5000) {
            addLog({
              timestamp: now,
              level: 'info',
              message: `Server stats: ${info.server?.connected_clients || 0} clients, ${info.keyspace?.total_keys || 0} keys, ${info.stats?.instantaneous_ops_per_sec || 0} ops/sec`,
              source: 'monitor',
            });
          }
        }
      }
    } catch (error: any) {
      // If admin.logs doesn't exist, that's OK - just continue polling
      console.debug('Log polling (admin.logs may not exist):', error.message);
    }
  };
  
  // Poll every 3 seconds
  streamInterval = setInterval(pollLogs, 3000);
  pollLogs(); // Initial poll
}

function stopStreaming() {
  if (eventSource) {
    eventSource.close();
    eventSource = null;
  }
  if (streamInterval) {
    clearInterval(streamInterval);
    streamInterval = null;
  }
}

function addLog(log: LogEntry) {
  logs.value.push(log);
  
  // Keep only last 1000 logs
  if (logs.value.length > 1000) {
    logs.value.shift();
  }
  
  if (autoScroll.value) {
    nextTick(() => {
      if (logContainer.value) {
        logContainer.value.scrollTop = logContainer.value.scrollHeight;
      }
    });
  }
}

function clearLogs() {
  if (confirm('Clear all logs?')) {
    logs.value = [];
  }
}

function exportLogs() {
  const data = filteredLogs.value.map((log) => ({
    timestamp: new Date(log.timestamp).toISOString(),
    level: log.level,
    source: log.source || '',
    message: log.message,
  }));
  
  const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `synap-logs-${new Date().toISOString().slice(0, 10)}.json`;
  a.click();
  URL.revokeObjectURL(url);
}

function formatTime(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}

function getLevelClass(level: string): string {
  switch (level) {
    case 'error':
      return 'bg-error/20 text-error';
    case 'warn':
      return 'bg-warning/20 text-warning';
    case 'info':
      return 'bg-info/20 text-info';
    case 'debug':
      return 'bg-bg-tertiary text-text-muted';
    default:
      return 'bg-bg-tertiary text-text-muted';
  }
}

function getLevelBgClass(level: string): string {
  switch (level) {
    case 'error':
      return 'bg-error/5';
    case 'warn':
      return 'bg-warning/5';
    default:
      return '';
  }
}

// Watch for auto-scroll changes
watch(filteredLogs, () => {
  if (autoScroll.value) {
    nextTick(() => {
      if (logContainer.value) {
        logContainer.value.scrollTop = logContainer.value.scrollHeight;
      }
    });
  }
});

onMounted(() => {
  if (isConnected.value) {
    startStreaming();
  }
});

onUnmounted(() => {
  stopStreaming();
});
</script>
