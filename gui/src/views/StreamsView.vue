<template>
  <div class="p-6">
    <div v-if="!isConnected" class="bg-warning/10 border border-warning rounded-lg p-4 mb-6">
      <div class="flex items-center gap-3">
        <i class="fas fa-exclamation-triangle text-warning"></i>
        <p class="text-text-primary text-sm">
          No server connected. Please select or add a server to view streams.
        </p>
      </div>
    </div>

    <div v-else class="space-y-6">
      <!-- Header -->
      <div class="flex items-center justify-between">
        <div>
          <h1 class="text-2xl font-bold text-text-primary">Event Streams</h1>
          <p class="text-text-secondary mt-1">
            Kafka-style partitioned topics
            <span v-if="lastUpdate" class="text-xs text-text-muted ml-2">
              (Atualizado: {{ formatTime(lastUpdate) }})
            </span>
          </p>
        </div>
        <div class="flex items-center gap-3">
          <button
            @click="refreshStreams"
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
            <span>Create Stream</span>
          </button>
        </div>
      </div>

      <!-- Statistics -->
      <div class="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Total Streams</span>
            <i class="fas fa-stream text-info"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ streams.length }}</div>
        </div>

        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Total Partitions</span>
            <i class="fas fa-layer-group text-success"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ totalPartitions }}</div>
        </div>

        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Total Messages</span>
            <i class="fas fa-envelope text-warning"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ totalMessages }}</div>
        </div>

        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Subscribers</span>
            <i class="fas fa-users text-info"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ totalSubscribers }}</div>
        </div>
      </div>

      <!-- Streams List -->
      <div class="bg-bg-card border border-border rounded-lg overflow-hidden">
        <div class="overflow-x-auto">
          <table class="w-full">
            <thead class="bg-bg-secondary border-b border-border">
              <tr>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">Stream Name</th>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">Partitions</th>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">Messages</th>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">Subscribers</th>
                <th class="px-4 py-3 text-right text-sm font-semibold text-text-primary">Actions</th>
              </tr>
            </thead>
            <tbody>
              <tr v-if="loading" class="border-b border-border">
                <td colspan="5" class="px-4 py-8 text-center text-text-secondary">
                  <i class="fas fa-spinner fa-spin mr-2"></i>
                  Loading streams...
                </td>
              </tr>
              <tr v-else-if="streams.length === 0" class="border-b border-border">
                <td colspan="5" class="px-4 py-8 text-center text-text-secondary">
                  No streams found. Create a new stream to get started.
                </td>
              </tr>
              <tr
                v-else
                v-for="stream in streams"
                :key="stream.room"
                class="border-b border-border hover:bg-bg-hover transition-colors"
              >
                <td class="px-4 py-3">
                  <div class="flex items-center gap-2">
                    <i class="fas fa-stream text-text-muted"></i>
                    <span class="text-text-primary font-mono text-sm">{{ stream.room }}</span>
                  </div>
                </td>
                <td class="px-4 py-3">
                  <span class="px-2 py-1 bg-info/20 text-info text-xs rounded font-mono">
                    {{ stream.partitions || 1 }}
                  </span>
                </td>
                <td class="px-4 py-3 text-text-primary text-sm font-mono">
                  {{ stream.messages || 0 }}
                </td>
                <td class="px-4 py-3">
                  <span class="px-2 py-1 bg-success/20 text-success text-xs rounded font-mono">
                    {{ stream.subscribers || 0 }}
                  </span>
                </td>
                <td class="px-4 py-3">
                  <div class="flex items-center justify-end gap-2">
                    <button
                      @click="viewStreamStats(stream.room)"
                      class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary hover:text-text-primary"
                      title="View Stats"
                    >
                      <i class="fas fa-chart-bar"></i>
                    </button>
                    <button
                      @click="publishToStream(stream)"
                      class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary hover:text-success"
                      title="Publish Message"
                    >
                      <i class="fas fa-paper-plane"></i>
                    </button>
                    <button
                      @click="deleteStream(stream.room)"
                      class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary hover:text-error"
                      title="Delete Stream"
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

    <!-- Create Stream Modal -->
    <div
      v-if="showCreateModal"
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      @click.self="showCreateModal = false"
    >
      <div class="bg-bg-card border border-border rounded-lg p-6 w-full max-w-md">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-xl font-bold text-text-primary">Create Stream</h2>
          <button
            @click="showCreateModal = false"
            class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary"
          >
            <i class="fas fa-times"></i>
          </button>
        </div>

        <div class="space-y-4">
          <div>
            <label class="block text-text-secondary text-sm mb-2">Stream Name (Room)</label>
            <input
              v-model="newStreamName"
              type="text"
              class="w-full px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary focus:outline-none focus:border-border-focus"
              placeholder="e.g., events-room"
            />
          </div>

          <div>
            <label class="block text-text-secondary text-sm mb-2">Number of Partitions</label>
            <input
              v-model.number="newStreamPartitions"
              type="number"
              min="1"
              max="100"
              class="w-full px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary focus:outline-none focus:border-border-focus"
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
              @click="createStream"
              class="px-4 py-2 bg-info hover:bg-info/80 text-white rounded-lg transition-colors"
              :disabled="!newStreamName.trim()"
            >
              Create
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Publish Message Modal -->
    <div
      v-if="publishingToStream"
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      @click.self="publishingToStream = null"
    >
      <div class="bg-bg-card border border-border rounded-lg p-6 w-full max-w-lg">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-xl font-bold text-text-primary">Publish to {{ publishingToStream.room }}</h2>
          <button
            @click="publishingToStream = null"
            class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary"
          >
            <i class="fas fa-times"></i>
          </button>
        </div>

        <div class="space-y-4">
          <div>
            <label class="block text-text-secondary text-sm mb-2">Partition (0 - {{ (publishingToStream.partitions || 1) - 1 }})</label>
            <input
              v-model.number="publishPartition"
              type="number"
              :min="0"
              :max="(publishingToStream.partitions || 1) - 1"
              class="w-full px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary focus:outline-none focus:border-border-focus"
            />
          </div>

          <div>
            <label class="block text-text-secondary text-sm mb-2">Message</label>
            <textarea
              v-model="publishMessage"
              rows="5"
              class="w-full px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary focus:outline-none focus:border-border-focus font-mono text-sm"
              placeholder="Enter message content..."
            ></textarea>
          </div>

          <div class="flex items-center justify-end gap-3 pt-4">
            <button
              @click="publishingToStream = null"
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

    <!-- Stream Stats Modal -->
    <div
      v-if="viewingStreamStats"
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      @click.self="viewingStreamStats = null"
    >
      <div class="bg-bg-card border border-border rounded-lg p-6 w-full max-w-lg">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-xl font-bold text-text-primary">{{ viewingStreamStats }} Stats</h2>
          <button
            @click="viewingStreamStats = null"
            class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary"
          >
            <i class="fas fa-times"></i>
          </button>
        </div>

        <div v-if="streamStatsData" class="space-y-4">
          <div class="grid grid-cols-2 gap-4">
            <div class="p-3 bg-bg-tertiary rounded-lg">
              <div class="text-text-muted text-sm">Partitions</div>
              <div class="text-xl font-bold text-text-primary">{{ streamStatsData.partitions || 1 }}</div>
            </div>
            <div class="p-3 bg-bg-tertiary rounded-lg">
              <div class="text-text-muted text-sm">Subscribers</div>
              <div class="text-xl font-bold text-success">{{ streamStatsData.subscribers || 0 }}</div>
            </div>
            <div class="p-3 bg-bg-tertiary rounded-lg col-span-2">
              <div class="text-text-muted text-sm">Total Messages</div>
              <div class="text-xl font-bold text-text-primary">{{ streamStatsData.total_messages || 0 }}</div>
            </div>
          </div>

          <div v-if="streamStatsData.messages_per_partition?.length" class="mt-4">
            <div class="text-text-secondary text-sm mb-2">Messages per Partition</div>
            <div class="grid grid-cols-4 gap-2">
              <div
                v-for="(count, index) in streamStatsData.messages_per_partition"
                :key="index"
                class="p-2 bg-bg-tertiary rounded text-center"
              >
                <div class="text-text-muted text-xs">P{{ index }}</div>
                <div class="text-text-primary font-mono">{{ count }}</div>
              </div>
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
import type { StreamInfo, StreamStats } from '@/services/api';

const { apiClient, isConnected } = useApi();

const loading = ref(false);
const streams = ref<StreamInfo[]>([]);
const lastUpdate = ref<number | null>(null);
const showCreateModal = ref(false);
const newStreamName = ref('');
const newStreamPartitions = ref(1);
const publishingToStream = ref<StreamInfo | null>(null);
const publishMessage = ref('');
const publishPartition = ref(0);
const viewingStreamStats = ref<string | null>(null);
const streamStatsData = ref<StreamStats | null>(null);

const totalPartitions = computed(() => 
  streams.value.reduce((sum, s) => sum + (s.partitions || 1), 0)
);

const totalMessages = computed(() => 
  streams.value.reduce((sum, s) => sum + (s.messages || 0), 0)
);

const totalSubscribers = computed(() => 
  streams.value.reduce((sum, s) => sum + (s.subscribers || 0), 0)
);

function formatTime(timestamp: number): string {
  const date = new Date(timestamp);
  return date.toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

async function refreshStreams(): Promise<void> {
  if (!apiClient.value) return;

  loading.value = true;
  try {
    const response = await apiClient.value.getStreamList();
    if (response.success && response.data) {
      const oldCount = streams.value.length;
      streams.value = response.data;
      lastUpdate.value = Date.now();
      
      // Notificar se houve mudança
      if (oldCount !== streams.value.length) {
        console.log(`Streams atualizados: ${oldCount} → ${streams.value.length}`);
      }
    } else {
      streams.value = [];
      lastUpdate.value = Date.now();
    }
  } catch (error) {
    console.error('Failed to load streams:', error);
    streams.value = [];
    lastUpdate.value = Date.now();
  } finally {
    loading.value = false;
  }
}

async function createStream(): Promise<void> {
  if (!apiClient.value || !newStreamName.value.trim()) return;

  try {
    const response = await apiClient.value.streamCreate(
      newStreamName.value.trim(),
      newStreamPartitions.value
    );
    if (response.success) {
      showCreateModal.value = false;
      newStreamName.value = '';
      newStreamPartitions.value = 1;
      await refreshStreams();
    }
  } catch (error) {
    console.error('Failed to create stream:', error);
  }
}

function publishToStream(stream: StreamInfo): void {
  publishingToStream.value = stream;
  publishMessage.value = '';
  publishPartition.value = 0;
}

async function doPublish(): Promise<void> {
  if (!apiClient.value || !publishingToStream.value || !publishMessage.value.trim()) return;

  try {
    const response = await apiClient.value.streamPublish(
      publishingToStream.value.room,
      publishPartition.value,
      publishMessage.value
    );
    if (response.success) {
      publishingToStream.value = null;
      await refreshStreams();
    }
  } catch (error) {
    console.error('Failed to publish message:', error);
  }
}

async function viewStreamStats(streamName: string): Promise<void> {
  viewingStreamStats.value = streamName;
  streamStatsData.value = null;

  if (!apiClient.value) return;

  try {
    const response = await apiClient.value.getStreamStats(streamName);
    if (response.success && response.data) {
      streamStatsData.value = response.data;
    }
  } catch (error) {
    console.error('Failed to load stream stats:', error);
  }
}

async function deleteStream(streamName: string): Promise<void> {
  if (!confirm(`Are you sure you want to delete stream "${streamName}"?`)) return;
  if (!apiClient.value) return;

  try {
    const response = await apiClient.value.streamDelete(streamName);
    if (response.success) {
      await refreshStreams();
    }
  } catch (error) {
    console.error('Failed to delete stream:', error);
  }
}

let refreshInterval: NodeJS.Timeout | null = null;

onMounted(() => {
  if (isConnected.value) {
    refreshStreams();
    // Auto-refresh a cada 5 segundos
    refreshInterval = setInterval(() => {
      if (isConnected.value) {
        refreshStreams();
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
    refreshStreams();
    if (!refreshInterval) {
      refreshInterval = setInterval(() => {
        if (isConnected.value) {
          refreshStreams();
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
