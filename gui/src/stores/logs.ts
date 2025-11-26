import { defineStore } from 'pinia';
import { ref, computed } from 'vue';

export type LogLevel = 'DEBUG' | 'INFO' | 'WARN' | 'ERROR';

export interface LogEntry {
  id: string;
  timestamp: number;
  level: LogLevel;
  message: string;
  source?: string;
  metadata?: Record<string, any>;
}

export const useLogsStore = defineStore('logs', () => {
  const logs = ref<LogEntry[]>([]);
  const maxLogs = ref<number>(10000);
  const filters = ref<{
    level?: LogLevel[];
    search?: string;
    source?: string;
  }>({});

  const filteredLogs = computed(() => {
    let result = logs.value;

    // Filter by level
    if (filters.value.level && filters.value.level.length > 0) {
      result = result.filter((log) => filters.value.level!.includes(log.level));
    }

    // Filter by search term
    if (filters.value.search) {
      const search = filters.value.search.toLowerCase();
      result = result.filter(
        (log) =>
          log.message.toLowerCase().includes(search) ||
          log.source?.toLowerCase().includes(search)
      );
    }

    // Filter by source
    if (filters.value.source) {
      result = result.filter((log) => log.source === filters.value.source);
    }

    return result;
  });

  const logCounts = computed(() => {
    const counts = {
      DEBUG: 0,
      INFO: 0,
      WARN: 0,
      ERROR: 0,
    };

    logs.value.forEach((log) => {
      counts[log.level]++;
    });

    return counts;
  });

  function addLog(entry: Omit<LogEntry, 'id' | 'timestamp'>): void {
    const log: LogEntry = {
      ...entry,
      id: `log-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
      timestamp: Date.now(),
    };

    logs.value.push(log);

    // Trim logs if exceeding max
    if (logs.value.length > maxLogs.value) {
      logs.value.shift();
    }
  }

  function addLogs(entries: Omit<LogEntry, 'id' | 'timestamp'>[]): void {
    entries.forEach((entry) => addLog(entry));
  }

  function clearLogs(): void {
    logs.value = [];
  }

  function removeLog(id: string): void {
    const index = logs.value.findIndex((log) => log.id === id);
    if (index !== -1) {
      logs.value.splice(index, 1);
    }
  }

  function setFilters(newFilters: typeof filters.value): void {
    filters.value = { ...filters.value, ...newFilters };
  }

  function clearFilters(): void {
    filters.value = {};
  }

  function setMaxLogs(max: number): void {
    maxLogs.value = max;
    // Trim existing logs if necessary
    if (logs.value.length > maxLogs.value) {
      logs.value = logs.value.slice(-maxLogs.value);
    }
  }

  function exportLogs(format: 'json' | 'csv' | 'txt' = 'json'): string {
    const data = filteredLogs.value;

    switch (format) {
      case 'json':
        return JSON.stringify(data, null, 2);
      case 'csv':
        const headers = ['timestamp', 'level', 'source', 'message'];
        const rows = data.map((log) => [
          new Date(log.timestamp).toISOString(),
          log.level,
          log.source || '',
          JSON.stringify(log.message),
        ]);
        return [headers.join(','), ...rows.map((row) => row.join(','))].join('\n');
      case 'txt':
        return data
          .map(
            (log) =>
              `[${new Date(log.timestamp).toISOString()}] [${log.level}] ${log.source || 'unknown'}: ${log.message}`
          )
          .join('\n');
      default:
        return '';
    }
  }

  return {
    logs,
    filteredLogs,
    logCounts,
    maxLogs,
    filters,
    addLog,
    addLogs,
    clearLogs,
    removeLog,
    setFilters,
    clearFilters,
    setMaxLogs,
    exportLogs,
  };
});

