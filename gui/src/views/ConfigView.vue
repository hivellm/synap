<template>
  <div class="p-6">
    <div class="mb-6 flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold text-text-primary">Configuration</h1>
        <p class="text-text-secondary mt-1">
          The configuration the server is running with, secrets redacted. Read-only: edit the
          server's config file and restart it to change a setting.
        </p>
      </div>
      <div class="flex gap-3">
        <button
          @click="loadConfig"
          :disabled="!isConnected || loading"
          class="px-4 py-2 bg-bg-tertiary hover:bg-bg-hover border border-border rounded-lg text-text-primary text-sm transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
        >
          <i class="fas fa-sync"></i>
          Reload
        </button>
        <button
          @click="copyConfig"
          :disabled="!snapshot"
          class="px-4 py-2 bg-bg-tertiary hover:bg-bg-hover border border-border rounded-lg text-text-primary text-sm transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
        >
          <i class="fas fa-copy"></i>
          Copy YAML
        </button>
        <button
          @click="downloadConfig"
          :disabled="!snapshot"
          class="px-4 py-2 bg-info hover:bg-info/80 text-white rounded-lg text-sm transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
        >
          <i class="fas fa-download"></i>
          Download YAML
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

    <template v-else>
      <div v-if="copyStatus" class="mb-4 text-sm text-text-secondary">{{ copyStatus }}</div>

      <div v-if="loading" class="flex items-center justify-center h-64">
        <i class="fas fa-spinner fa-spin text-2xl text-text-muted"></i>
      </div>

      <div v-else-if="loadError" class="bg-error/10 border border-error rounded-xl p-4">
        <div class="flex items-start gap-2">
          <i class="fas fa-exclamation-circle text-error mt-1"></i>
          <div>
            <p class="text-text-primary font-medium">Could not load the server configuration</p>
            <p class="text-text-secondary text-sm mt-1 font-mono break-all">{{ loadError }}</p>
            <p class="text-text-secondary text-sm mt-2">
              Viewing the configuration requires an API key with admin permission.
            </p>
          </div>
        </div>
      </div>

      <div v-else-if="snapshot" class="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <!-- YAML Panel -->
        <div class="bg-bg-card border border-border rounded-xl overflow-hidden flex flex-col">
          <div class="px-4 py-3 bg-bg-secondary border-b border-border flex items-center gap-2">
            <i class="fas fa-file-code text-text-secondary"></i>
            <span class="text-sm font-medium text-text-primary">Effective configuration (YAML)</span>
            <span class="text-xs text-text-muted ml-auto">
              <i class="fas fa-lock mr-1"></i>read-only
            </span>
          </div>
          <pre
            class="flex-1 min-h-96 m-0 p-4 bg-bg-primary text-text-primary font-mono text-sm overflow-auto whitespace-pre"
          >{{ snapshot.yaml }}</pre>
        </div>

        <!-- Structured Panel -->
        <div class="bg-bg-card border border-border rounded-xl overflow-hidden flex flex-col">
          <div class="px-4 py-3 bg-bg-secondary border-b border-border flex items-center gap-2">
            <i class="fas fa-eye text-text-secondary"></i>
            <span class="text-sm font-medium text-text-primary">Sections</span>
          </div>
          <div class="flex-1 min-h-0 p-4 overflow-auto space-y-4">
            <ConfigSection
              v-for="(value, key) in snapshot.config"
              :key="key"
              :title="String(key)"
              :data="value"
            />
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted } from 'vue';
import { useApi } from '@/composables/useApi';
import ConfigSection from '@/components/ConfigSection.vue';
import type { ServerConfigSnapshot } from '@/services/api';

const { apiClient, isConnected } = useApi();

const snapshot = ref<ServerConfigSnapshot | null>(null);
const loading = ref(false);
const loadError = ref('');
const copyStatus = ref('');

/** Load the server's effective configuration; on failure show the error and nothing else. */
async function loadConfig(): Promise<void> {
  const client = apiClient.value;
  if (!client) return;

  loading.value = true;
  loadError.value = '';
  copyStatus.value = '';
  snapshot.value = null;
  try {
    const response = await client.getServerConfig();
    if (response.success && response.data) {
      snapshot.value = response.data;
    } else {
      loadError.value = response.error || 'The server returned no configuration';
    }
  } catch (error) {
    loadError.value = error instanceof Error ? error.message : String(error);
  } finally {
    loading.value = false;
  }
}

async function copyConfig(): Promise<void> {
  if (!snapshot.value) return;
  try {
    await navigator.clipboard.writeText(snapshot.value.yaml);
    copyStatus.value = 'Configuration copied to the clipboard.';
  } catch (error) {
    copyStatus.value = `Copy failed: ${error instanceof Error ? error.message : String(error)}`;
  }
}

function downloadConfig(): void {
  if (!snapshot.value) return;
  const blob = new Blob([snapshot.value.yaml], { type: 'application/yaml' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `synap-config-${new Date().toISOString().slice(0, 10)}.yml`;
  a.click();
  URL.revokeObjectURL(url);
}

// A different server, or a reconnection, has its own configuration.
watch([apiClient, isConnected], () => {
  snapshot.value = null;
  loadError.value = '';
  if (isConnected.value) {
    loadConfig();
  }
});

onMounted(() => {
  if (isConnected.value) {
    loadConfig();
  }
});
</script>
