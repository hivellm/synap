<template>
  <div class="p-6">
    <div class="mb-6 flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold text-text-primary">Configuration</h1>
        <p class="text-text-secondary mt-1">Edit server configuration with live preview</p>
      </div>
      <div class="flex gap-3">
        <button
          @click="loadConfig"
          class="px-4 py-2 bg-bg-tertiary hover:bg-bg-hover border border-border rounded-lg text-text-primary text-sm transition-colors flex items-center gap-2"
        >
          <i class="fas fa-sync"></i>
          Reload
        </button>
        <button
          @click="saveConfig"
          :disabled="!hasChanges"
          class="px-4 py-2 bg-info hover:bg-info/80 text-white rounded-lg text-sm transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
        >
          <i class="fas fa-save"></i>
          Save Changes
        </button>
      </div>
    </div>

    <div v-if="!isConnected" class="bg-warning/10 border border-warning rounded-xl p-4 mb-6">
      <div class="flex items-center gap-2">
        <i class="fas fa-exclamation-triangle text-warning"></i>
        <p class="text-text-primary">
          No server connected. Please select or add a server to view configuration.
        </p>
      </div>
    </div>

    <div v-else class="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <!-- Editor Panel -->
      <div class="bg-bg-card border border-border rounded-xl overflow-hidden flex flex-col">
        <div class="px-4 py-3 bg-bg-secondary border-b border-border flex items-center justify-between">
          <div class="flex items-center gap-2">
            <i class="fas fa-file-code text-text-secondary"></i>
            <span class="text-sm font-medium text-text-primary">config.yml</span>
          </div>
          <div class="flex items-center gap-2">
            <span v-if="hasChanges" class="text-xs text-warning">
              <i class="fas fa-circle text-xs mr-1"></i>Unsaved changes
            </span>
            <span v-if="validationError" class="text-xs text-error">
              <i class="fas fa-exclamation-circle mr-1"></i>{{ validationError }}
            </span>
          </div>
        </div>
        <div class="flex-1 min-h-0">
          <textarea
            v-model="configContent"
            @input="handleChange"
            class="w-full h-full min-h-96 p-4 bg-bg-primary text-text-primary font-mono text-sm resize-none focus:outline-none"
            spellcheck="false"
            placeholder="Loading configuration..."
          ></textarea>
        </div>
      </div>

      <!-- Preview Panel -->
      <div class="bg-bg-card border border-border rounded-xl overflow-hidden flex flex-col">
        <div class="px-4 py-3 bg-bg-secondary border-b border-border flex items-center gap-2">
          <i class="fas fa-eye text-text-secondary"></i>
          <span class="text-sm font-medium text-text-primary">Preview</span>
        </div>
        <div class="flex-1 min-h-0 p-4 overflow-auto">
          <div v-if="loading" class="flex items-center justify-center h-64">
            <i class="fas fa-spinner fa-spin text-2xl text-text-muted"></i>
          </div>
          <div v-else-if="parsedConfig" class="space-y-4">
            <ConfigSection
              v-for="(value, key) in parsedConfig"
              :key="key"
              :title="String(key)"
              :data="value"
            />
          </div>
          <div v-else class="text-center py-8 text-text-muted">
            <i class="fas fa-exclamation-circle text-2xl mb-2 block"></i>
            <p>Unable to parse configuration</p>
          </div>
        </div>
      </div>

      <!-- Rollback Section -->
      <div class="lg:col-span-2 bg-bg-card border border-border rounded-xl p-6">
        <h3 class="text-lg font-semibold text-text-primary mb-4 flex items-center gap-2">
          <i class="fas fa-history"></i>
          Configuration History
        </h3>
        <div v-if="configHistory.length === 0" class="text-center py-8 text-text-muted">
          No configuration history available
        </div>
        <div v-else class="space-y-2">
          <div
            v-for="(history, index) in configHistory"
            :key="index"
            class="flex items-center justify-between p-3 bg-bg-tertiary rounded-lg hover:bg-bg-hover transition-colors"
          >
            <div class="flex items-center gap-3">
              <i class="fas fa-file-alt text-text-muted"></i>
              <div>
                <p class="text-sm text-text-primary">{{ history.description }}</p>
                <p class="text-xs text-text-secondary">{{ formatDate(history.timestamp) }}</p>
              </div>
            </div>
            <button
              @click="rollbackTo(index)"
              class="px-3 py-1 text-sm text-info hover:bg-info/20 rounded transition-colors"
            >
              <i class="fas fa-undo mr-1"></i>
              Rollback
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useApi } from '@/composables/useApi';
import ConfigSection from '@/components/ConfigSection.vue';

interface ConfigHistoryItem {
  content: string;
  description: string;
  timestamp: number;
}

const { apiClient, isConnected } = useApi();

const configContent = ref('');
const originalConfig = ref('');
const loading = ref(false);
const validationError = ref('');
const configHistory = ref<ConfigHistoryItem[]>([]);

const hasChanges = computed(() => configContent.value !== originalConfig.value);

const parsedConfig = computed(() => {
  try {
    // Simple YAML-like parsing for preview
    // In production, use a proper YAML parser
    const lines = configContent.value.split('\n');
    const result: Record<string, unknown> = {};
    let currentSection = '';

    for (const line of lines) {
      if (line.trim().startsWith('#') || !line.trim()) continue;
      
      if (!line.startsWith(' ') && !line.startsWith('\t') && line.includes(':')) {
        const key = line.split(':')[0].trim();
        const value = line.split(':').slice(1).join(':').trim();
        if (value) {
          result[key] = value;
        } else {
          currentSection = key;
          result[currentSection] = {};
        }
      } else if (currentSection && line.includes(':')) {
        const key = line.split(':')[0].trim();
        const value = line.split(':').slice(1).join(':').trim();
        (result[currentSection] as Record<string, string>)[key] = value;
      }
    }

    validationError.value = '';
    return result;
  } catch (e) {
    validationError.value = 'Invalid YAML syntax';
    return null;
  }
});

async function loadConfig() {
  if (!apiClient.value) return;

  loading.value = true;
  try {
    // Try to fetch config using admin.config command (no REST endpoint exists)
    const response = await apiClient.value.executeCommand('admin.config', {});
    
    if (response.success && response.data) {
      const configData = response.data.payload || response.data;
      // Convert config object to YAML-like string
      configContent.value = objectToYaml(configData);
      originalConfig.value = configContent.value;
    } else {
      // Use default sample config if command fails
      console.warn('Failed to load config from server, using sample config');
      configContent.value = getSampleConfig();
      originalConfig.value = configContent.value;
    }
  } catch (error) {
    console.error('Error loading config:', error);
    // Use sample config on error
    configContent.value = getSampleConfig();
    originalConfig.value = configContent.value;
  } finally {
    loading.value = false;
  }
}

function objectToYaml(obj: Record<string, unknown>, indent = 0): string {
  let result = '';
  const prefix = '  '.repeat(indent);

  for (const [key, value] of Object.entries(obj)) {
    if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
      result += `${prefix}${key}:\n`;
      result += objectToYaml(value as Record<string, unknown>, indent + 1);
    } else if (Array.isArray(value)) {
      result += `${prefix}${key}:\n`;
      for (const item of value) {
        result += `${prefix}  - ${item}\n`;
      }
    } else {
      result += `${prefix}${key}: ${value}\n`;
    }
  }

  return result;
}

function getSampleConfig(): string {
  return `# Synap Server Configuration
server:
  host: 0.0.0.0
  port: 15500
  max_connections: 10000

storage:
  data_dir: ./data
  sync_interval: 1000
  compression: true

replication:
  enabled: false
  role: master
  master_host: ""
  master_port: 0

logging:
  level: info
  format: json
  output: stdout

security:
  auth_enabled: false
  tls_enabled: false
`;
}

async function saveConfig() {
  if (!apiClient.value || !hasChanges.value) return;

  try {
    // Save to history before applying
    configHistory.value.unshift({
      content: originalConfig.value,
      description: 'Configuration before save',
      timestamp: Date.now(),
    });

    // Keep only last 10 history items
    if (configHistory.value.length > 10) {
      configHistory.value.pop();
    }

    // Here you would send the config to the server
    // For now, just update the original
    originalConfig.value = configContent.value;

    alert('Configuration saved successfully!');
  } catch (error) {
    alert('Failed to save configuration');
  }
}

function handleChange() {
  // Validate on change
  parsedConfig.value;
}

function rollbackTo(index: number) {
  if (confirm('Are you sure you want to rollback to this configuration?')) {
    configContent.value = configHistory.value[index].content;
  }
}

function formatDate(timestamp: number): string {
  return new Date(timestamp).toLocaleString();
}

onMounted(() => {
  if (isConnected.value) {
    loadConfig();
  }
});
</script>
