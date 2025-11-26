<template>
  <div class="p-6">
    <div v-if="!isConnected" class="bg-warning/10 border border-warning rounded-lg p-4 mb-6">
      <div class="flex items-center gap-3">
        <i class="fas fa-exclamation-triangle text-warning"></i>
        <p class="text-text-primary text-sm">
          No server connected. Please select or add a server to view queues.
        </p>
      </div>
    </div>

    <div v-else class="space-y-6">
      <!-- Header -->
      <div class="flex items-center justify-between">
        <div>
          <h1 class="text-2xl font-bold text-text-primary">Queue System</h1>
          <p class="text-text-secondary mt-1">Manage message queues</p>
        </div>
        <div class="flex items-center gap-3">
          <button
            @click="refreshQueues"
            class="px-4 py-2 bg-bg-tertiary hover:bg-bg-hover border border-border rounded-lg text-text-primary text-sm transition-colors flex items-center gap-2"
            :disabled="loading"
          >
            <i class="fas fa-sync-alt" :class="{ 'fa-spin': loading }"></i>
            <span>Refresh</span>
          </button>
          <button
            @click="showCreateModal = true"
            class="px-4 py-2 bg-info hover:bg-info/80 text-white rounded-lg transition-colors flex items-center gap-2"
          >
            <i class="fas fa-plus"></i>
            <span>Create Queue</span>
          </button>
        </div>
      </div>

      <!-- Statistics -->
      <div class="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Total Queues</span>
            <i class="fas fa-inbox text-info"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ queues.length }}</div>
        </div>

        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Total Messages</span>
            <i class="fas fa-envelope text-success"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ totalMessages }}</div>
        </div>

        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Pending</span>
            <i class="fas fa-clock text-warning"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ totalPending }}</div>
        </div>

        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Dead Letters</span>
            <i class="fas fa-skull-crossbones text-error"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ totalDeadLetters }}</div>
        </div>
      </div>

      <!-- Queues List -->
      <div class="bg-bg-card border border-border rounded-lg overflow-hidden">
        <div class="overflow-x-auto">
          <table class="w-full">
            <thead class="bg-bg-secondary border-b border-border">
              <tr>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">Queue Name</th>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">Size</th>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">Pending</th>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">Processing</th>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">Dead Letters</th>
                <th class="px-4 py-3 text-right text-sm font-semibold text-text-primary">Actions</th>
              </tr>
            </thead>
            <tbody>
              <tr v-if="loading" class="border-b border-border">
                <td colspan="6" class="px-4 py-8 text-center text-text-secondary">
                  <i class="fas fa-spinner fa-spin mr-2"></i>
                  Loading queues...
                </td>
              </tr>
              <tr v-else-if="queues.length === 0" class="border-b border-border">
                <td colspan="6" class="px-4 py-8 text-center text-text-secondary">
                  No queues found. Create a new queue to get started.
                </td>
              </tr>
              <tr
                v-else
                v-for="queue in queues"
                :key="queue.name"
                class="border-b border-border hover:bg-bg-hover transition-colors"
              >
                <td class="px-4 py-3">
                  <div class="flex items-center gap-2">
                    <i class="fas fa-inbox text-text-muted"></i>
                    <span class="text-text-primary font-mono text-sm">{{ queue.name }}</span>
                  </div>
                </td>
                <td class="px-4 py-3 text-text-primary text-sm font-mono">
                  {{ queue.size || 0 }}
                </td>
                <td class="px-4 py-3">
                  <span class="px-2 py-1 bg-warning/20 text-warning text-xs rounded font-mono">
                    {{ queue.pending || 0 }}
                  </span>
                </td>
                <td class="px-4 py-3">
                  <span class="px-2 py-1 bg-info/20 text-info text-xs rounded font-mono">
                    {{ queue.processing || 0 }}
                  </span>
                </td>
                <td class="px-4 py-3">
                  <span 
                    class="px-2 py-1 text-xs rounded font-mono"
                    :class="(queue.dead_letter_count || 0) > 0 ? 'bg-error/20 text-error' : 'bg-bg-tertiary text-text-muted'"
                  >
                    {{ queue.dead_letter_count || 0 }}
                  </span>
                </td>
                <td class="px-4 py-3">
                  <div class="flex items-center justify-end gap-2">
                    <button
                      @click="viewQueueStats(queue.name)"
                      class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary hover:text-text-primary"
                      title="View Stats"
                    >
                      <i class="fas fa-chart-bar"></i>
                    </button>
                    <button
                      @click="publishToQueue(queue.name)"
                      class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary hover:text-success"
                      title="Publish Message"
                    >
                      <i class="fas fa-paper-plane"></i>
                    </button>
                    <button
                      @click="purgeQueue(queue.name)"
                      class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary hover:text-warning"
                      title="Purge Queue"
                    >
                      <i class="fas fa-broom"></i>
                    </button>
                    <button
                      @click="deleteQueue(queue.name)"
                      class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary hover:text-error"
                      title="Delete Queue"
                    >
                      <i class="fas fa-trash"></i>
                    </button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>

    <!-- Create Queue Modal -->
    <div
      v-if="showCreateModal"
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      @click.self="showCreateModal = false"
    >
      <div class="bg-bg-card border border-border rounded-lg p-6 w-full max-w-md">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-xl font-bold text-text-primary">Create Queue</h2>
          <button
            @click="showCreateModal = false"
            class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary"
          >
            <i class="fas fa-times"></i>
          </button>
        </div>

        <div class="space-y-4">
          <div>
            <label class="block text-text-secondary text-sm mb-2">Queue Name</label>
            <input
              v-model="newQueueName"
              type="text"
              class="w-full px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary focus:outline-none focus:border-border-focus"
              placeholder="e.g., email-notifications"
            />
          </div>

          <div class="flex items-center justify-end gap-3 pt-4">
            <button
              @click="showCreateModal = false"
              class="px-4 py-2 bg-bg-tertiary hover:bg-bg-hover border border-border rounded-lg text-text-primary transition-colors"
            >
              Cancel
            </button>
            <button
              @click="createQueue"
              class="px-4 py-2 bg-info hover:bg-info/80 text-white rounded-lg transition-colors"
              :disabled="!newQueueName.trim()"
            >
              Create
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Publish Message Modal -->
    <div
      v-if="publishingToQueue"
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      @click.self="publishingToQueue = null"
    >
      <div class="bg-bg-card border border-border rounded-lg p-6 w-full max-w-lg">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-xl font-bold text-text-primary">Publish to {{ publishingToQueue }}</h2>
          <button
            @click="publishingToQueue = null"
            class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary"
          >
            <i class="fas fa-times"></i>
          </button>
        </div>

        <div class="space-y-4">
          <div>
            <label class="block text-text-secondary text-sm mb-2">Message</label>
            <textarea
              v-model="publishMessage"
              rows="5"
              class="w-full px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary focus:outline-none focus:border-border-focus font-mono text-sm"
              placeholder="Enter message content..."
            ></textarea>
          </div>

          <div>
            <label class="block text-text-secondary text-sm mb-2">Priority (0-9)</label>
            <input
              v-model.number="publishPriority"
              type="number"
              min="0"
              max="9"
              class="w-full px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary focus:outline-none focus:border-border-focus"
            />
          </div>

          <div class="flex items-center justify-end gap-3 pt-4">
            <button
              @click="publishingToQueue = null"
              class="px-4 py-2 bg-bg-tertiary hover:bg-bg-hover border border-border rounded-lg text-text-primary transition-colors"
            >
              Cancel
            </button>
            <button
              @click="doPublish"
              class="px-4 py-2 bg-success hover:bg-success/80 text-white rounded-lg transition-colors"
              :disabled="!publishMessage.trim()"
            >
              Publish
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Queue Stats Modal -->
    <div
      v-if="viewingQueueStats"
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      @click.self="viewingQueueStats = null"
    >
      <div class="bg-bg-card border border-border rounded-lg p-6 w-full max-w-lg">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-xl font-bold text-text-primary">{{ viewingQueueStats }} Stats</h2>
          <button
            @click="viewingQueueStats = null"
            class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary"
          >
            <i class="fas fa-times"></i>
          </button>
        </div>

        <div v-if="queueStatsData" class="space-y-4">
          <div class="grid grid-cols-2 gap-4">
            <div class="p-3 bg-bg-tertiary rounded-lg">
              <div class="text-text-muted text-sm">Size</div>
              <div class="text-xl font-bold text-text-primary">{{ queueStatsData.size || 0 }}</div>
            </div>
            <div class="p-3 bg-bg-tertiary rounded-lg">
              <div class="text-text-muted text-sm">Pending</div>
              <div class="text-xl font-bold text-warning">{{ queueStatsData.pending || 0 }}</div>
            </div>
            <div class="p-3 bg-bg-tertiary rounded-lg">
              <div class="text-text-muted text-sm">Processing</div>
              <div class="text-xl font-bold text-info">{{ queueStatsData.processing || 0 }}</div>
            </div>
            <div class="p-3 bg-bg-tertiary rounded-lg">
              <div class="text-text-muted text-sm">Dead Letters</div>
              <div class="text-xl font-bold text-error">{{ queueStatsData.dead_letter_count || 0 }}</div>
            </div>
            <div class="p-3 bg-bg-tertiary rounded-lg">
              <div class="text-text-muted text-sm">Total Published</div>
              <div class="text-xl font-bold text-text-primary">{{ queueStatsData.total_published || 0 }}</div>
            </div>
            <div class="p-3 bg-bg-tertiary rounded-lg">
              <div class="text-text-muted text-sm">Total Consumed</div>
              <div class="text-xl font-bold text-text-primary">{{ queueStatsData.total_consumed || 0 }}</div>
            </div>
            <div class="p-3 bg-bg-tertiary rounded-lg col-span-2">
              <div class="text-text-muted text-sm">Active Consumers</div>
              <div class="text-xl font-bold text-success">{{ queueStatsData.consumers || 0 }}</div>
            </div>
          </div>
        </div>
        <div v-else class="text-center py-8 text-text-secondary">
          <i class="fas fa-spinner fa-spin mr-2"></i>
          Loading stats...
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { useApi } from '@/composables/useApi';
import type { QueueInfo, QueueStats } from '@/services/api';

const { apiClient, isConnected } = useApi();

const loading = ref(false);
const queues = ref<QueueInfo[]>([]);
const lastUpdate = ref<number | null>(null);
const showCreateModal = ref(false);
const newQueueName = ref('');
const publishingToQueue = ref<string | null>(null);
const publishMessage = ref('');
const publishPriority = ref(5);
const viewingQueueStats = ref<string | null>(null);
const queueStatsData = ref<QueueStats | null>(null);

const totalMessages = computed(() => 
  queues.value.reduce((sum, q) => sum + (q.size || 0), 0)
);

const totalPending = computed(() => 
  queues.value.reduce((sum, q) => sum + (q.pending || 0), 0)
);

const totalDeadLetters = computed(() => 
  queues.value.reduce((sum, q) => sum + (q.dead_letter_count || 0), 0)
);

function formatTime(timestamp: number): string {
  const date = new Date(timestamp);
  return date.toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

async function refreshQueues(): Promise<void> {
  if (!apiClient.value) return;

  loading.value = true;
  try {
    const response = await apiClient.value.getQueueList();
    if (response.success && response.data) {
      const oldCount = queues.value.length;
      queues.value = response.data;
      lastUpdate.value = Date.now();
      
      // Notificar se houve mudança
      if (oldCount !== queues.value.length) {
        console.log(`Queues atualizadas: ${oldCount} → ${queues.value.length}`);
      }
    } else {
      queues.value = [];
      lastUpdate.value = Date.now();
    }
  } catch (error) {
    console.error('Failed to load queues:', error);
    queues.value = [];
    lastUpdate.value = Date.now();
  } finally {
    loading.value = false;
  }
}

async function createQueue(): Promise<void> {
  if (!apiClient.value || !newQueueName.value.trim()) return;

  try {
    const response = await apiClient.value.queueCreate(newQueueName.value.trim());
    if (response.success) {
      showCreateModal.value = false;
      newQueueName.value = '';
      await refreshQueues();
    }
  } catch (error) {
    console.error('Failed to create queue:', error);
  }
}

function publishToQueue(queueName: string): void {
  publishingToQueue.value = queueName;
  publishMessage.value = '';
  publishPriority.value = 5;
}

async function doPublish(): Promise<void> {
  if (!apiClient.value || !publishingToQueue.value || !publishMessage.value.trim()) return;

  try {
    const response = await apiClient.value.queuePublish(
      publishingToQueue.value,
      publishMessage.value,
      publishPriority.value
    );
    if (response.success) {
      publishingToQueue.value = null;
      await refreshQueues();
    }
  } catch (error) {
    console.error('Failed to publish message:', error);
  }
}

async function viewQueueStats(queueName: string): Promise<void> {
  viewingQueueStats.value = queueName;
  queueStatsData.value = null;

  if (!apiClient.value) return;

  try {
    const response = await apiClient.value.getQueueStats(queueName);
    if (response.success && response.data) {
      queueStatsData.value = response.data;
    }
  } catch (error) {
    console.error('Failed to load queue stats:', error);
  }
}

async function purgeQueue(queueName: string): Promise<void> {
  if (!confirm(`Are you sure you want to purge all messages from "${queueName}"?`)) return;
  if (!apiClient.value) return;

  try {
    const response = await apiClient.value.queuePurge(queueName);
    if (response.success) {
      await refreshQueues();
    }
  } catch (error) {
    console.error('Failed to purge queue:', error);
  }
}

async function deleteQueue(queueName: string): Promise<void> {
  if (!confirm(`Are you sure you want to delete queue "${queueName}"?`)) return;
  if (!apiClient.value) return;

  try {
    const response = await apiClient.value.queueDelete(queueName);
    if (response.success) {
      await refreshQueues();
    }
  } catch (error) {
    console.error('Failed to delete queue:', error);
  }
}

let refreshInterval: NodeJS.Timeout | null = null;

onMounted(() => {
  if (isConnected.value) {
    refreshQueues();
    // Auto-refresh a cada 5 segundos
    refreshInterval = setInterval(() => {
      if (isConnected.value) {
        refreshQueues();
      }
    }, 5000);
  }
});

onUnmounted(() => {
  if (refreshInterval) {
    clearInterval(refreshInterval);
    refreshInterval = null;
  }
});

watch(isConnected, (connected) => {
  if (connected) {
    refreshQueues();
    if (!refreshInterval) {
      refreshInterval = setInterval(() => {
        if (isConnected.value) {
          refreshQueues();
        }
      }, 5000);
    }
  } else {
    if (refreshInterval) {
      clearInterval(refreshInterval);
      refreshInterval = null;
    }
  }
});
</script>
