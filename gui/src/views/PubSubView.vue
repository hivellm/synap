<template>
  <div class="p-6">
    <div v-if="!isConnected" class="bg-warning/10 border border-warning rounded-lg p-4 mb-6">
      <div class="flex items-center gap-3">
        <i class="fas fa-exclamation-triangle text-warning"></i>
        <p class="text-text-primary text-sm">
          No server connected. Please select or add a server to view Pub/Sub.
        </p>
      </div>
    </div>

    <div v-else class="space-y-6">
      <!-- Header -->
      <div class="flex items-center justify-between">
        <div>
          <h1 class="text-2xl font-bold text-text-primary">Pub/Sub</h1>
          <p class="text-text-secondary mt-1">Real-time publish/subscribe messaging</p>
        </div>
        <div class="flex items-center gap-3">
          <button
            @click="refreshTopics"
            class="px-4 py-2 bg-bg-tertiary hover:bg-bg-hover border border-border rounded-lg text-text-primary text-sm transition-colors flex items-center gap-2"
            :disabled="loading"
          >
            <i class="fas fa-sync-alt" :class="{ 'fa-spin': loading }"></i>
            <span>Refresh</span>
          </button>
        </div>
      </div>

      <!-- Statistics -->
      <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Total Topics</span>
            <i class="fas fa-broadcast-tower text-info"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ stats?.total_topics || topics.length }}</div>
        </div>

        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Total Subscribers</span>
            <i class="fas fa-users text-success"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ stats?.total_subscribers || 0 }}</div>
        </div>

        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Messages Published</span>
            <i class="fas fa-paper-plane text-warning"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ stats?.total_messages_published || 0 }}</div>
        </div>
      </div>

      <!-- Topics List -->
      <div class="bg-bg-card border border-border rounded-lg overflow-hidden">
        <div class="px-4 py-3 bg-bg-secondary border-b border-border flex items-center justify-between">
          <h3 class="text-lg font-semibold text-text-primary">Topics</h3>
          <div class="text-text-secondary text-sm">
            {{ topics.length }} topics
          </div>
        </div>

        <div class="overflow-x-auto">
          <table class="w-full">
            <thead class="bg-bg-secondary border-b border-border">
              <tr>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">Topic</th>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">Subscribers</th>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">Messages Published</th>
                <th class="px-4 py-3 text-right text-sm font-semibold text-text-primary">Actions</th>
              </tr>
            </thead>
            <tbody>
              <tr v-if="loading" class="border-b border-border">
                <td colspan="4" class="px-4 py-8 text-center text-text-secondary">
                  <i class="fas fa-spinner fa-spin mr-2"></i>
                  Loading topics...
                </td>
              </tr>
              <tr v-else-if="topicsWithInfo.length === 0" class="border-b border-border">
                <td colspan="4" class="px-4 py-8 text-center text-text-secondary">
                  No active topics. Publish a message to create a topic.
                </td>
              </tr>
              <tr
                v-else
                v-for="topic in topicsWithInfo"
                :key="topic.name"
                class="border-b border-border hover:bg-bg-hover transition-colors"
              >
                <td class="px-4 py-3">
                  <div class="flex items-center gap-2">
                    <i class="fas fa-broadcast-tower text-text-muted"></i>
                    <span class="text-text-primary font-mono text-sm">{{ topic.name }}</span>
                  </div>
                </td>
                <td class="px-4 py-3">
                  <span class="px-2 py-1 bg-success/20 text-success text-xs rounded font-mono">
                    {{ topic.subscribers || 0 }}
                  </span>
                </td>
                <td class="px-4 py-3 text-text-primary text-sm font-mono">
                  {{ topic.messages_published || 0 }}
                </td>
                <td class="px-4 py-3">
                  <div class="flex items-center justify-end gap-2">
                    <button
                      @click="publishToTopic(topic.name)"
                      class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary hover:text-success"
                      title="Publish Message"
                    >
                      <i class="fas fa-paper-plane"></i>
                    </button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <!-- Quick Publish -->
      <div class="bg-bg-card border border-border rounded-lg p-6">
        <h3 class="text-lg font-semibold text-text-primary mb-4">Quick Publish</h3>
        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div>
            <label class="block text-text-secondary text-sm mb-2">Topic</label>
            <input
              v-model="quickPublishTopic"
              type="text"
              class="w-full px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary focus:outline-none focus:border-border-focus"
              placeholder="e.g., notifications"
            />
          </div>
          <div class="md:col-span-2">
            <label class="block text-text-secondary text-sm mb-2">Message</label>
            <div class="flex gap-2">
              <input
                v-model="quickPublishMessage"
                type="text"
                class="flex-1 px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary focus:outline-none focus:border-border-focus"
                placeholder="Enter message..."
                @keyup.enter="doQuickPublish"
              />
              <button
                @click="doQuickPublish"
                class="px-4 py-2 bg-success hover:bg-success/80 text-white rounded-lg transition-colors flex items-center gap-2"
                :disabled="!quickPublishTopic.trim() || !quickPublishMessage.trim()"
              >
                <i class="fas fa-paper-plane"></i>
                <span>Publish</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Publish Modal -->
    <div
      v-if="publishingToTopic"
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      @click.self="publishingToTopic = null"
    >
      <div class="bg-bg-card border border-border rounded-lg p-6 w-full max-w-lg">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-xl font-bold text-text-primary">Publish to {{ publishingToTopic }}</h2>
          <button
            @click="publishingToTopic = null"
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

          <div class="flex items-center justify-end gap-3 pt-4">
            <button
              @click="publishingToTopic = null"
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
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { useApi } from '@/composables/useApi';
import type { PubsubStats, TopicInfo } from '@/services/api';

const { apiClient, isConnected } = useApi();

const loading = ref(false);
const topics = ref<string[]>([]);
const stats = ref<PubsubStats | null>(null);
const publishingToTopic = ref<string | null>(null);
const publishMessage = ref('');
const quickPublishTopic = ref('');
const quickPublishMessage = ref('');

// Combine topics list with stats info
const topicsWithInfo = computed(() => {
  const topicInfoMap = new Map<string, TopicInfo>();
  
  // Add topics from stats if available
  if (stats.value?.topics) {
    stats.value.topics.forEach(t => {
      topicInfoMap.set(t.name, t);
    });
  }
  
  // Add any topics from the topics list that aren't in stats
  topics.value.forEach(name => {
    if (!topicInfoMap.has(name)) {
      topicInfoMap.set(name, { name, subscribers: 0, messages_published: 0 });
    }
  });
  
  return Array.from(topicInfoMap.values()).sort((a, b) => a.name.localeCompare(b.name));
});

async function refreshTopics(): Promise<void> {
  if (!apiClient.value) return;

  loading.value = true;
  try {
    const [topicsRes, statsRes] = await Promise.all([
      apiClient.value.getPubsubTopics(),
      apiClient.value.getPubsubStats(),
    ]);

    if (topicsRes.success && topicsRes.data) {
      topics.value = topicsRes.data;
    } else {
      topics.value = [];
    }

    if (statsRes.success && statsRes.data) {
      stats.value = statsRes.data;
    }
  } catch (error) {
    console.error('Failed to load pubsub data:', error);
    topics.value = [];
    stats.value = null;
  } finally {
    loading.value = false;
  }
}

function publishToTopic(topic: string): void {
  publishingToTopic.value = topic;
  publishMessage.value = '';
}

async function doPublish(): Promise<void> {
  if (!apiClient.value || !publishingToTopic.value || !publishMessage.value.trim()) return;

  try {
    const response = await apiClient.value.pubsubPublish(
      publishingToTopic.value,
      publishMessage.value
    );
    if (response.success) {
      publishingToTopic.value = null;
      await refreshTopics();
    }
  } catch (error) {
    console.error('Failed to publish message:', error);
  }
}

async function doQuickPublish(): Promise<void> {
  if (!apiClient.value || !quickPublishTopic.value.trim() || !quickPublishMessage.value.trim()) return;

  try {
    const response = await apiClient.value.pubsubPublish(
      quickPublishTopic.value,
      quickPublishMessage.value
    );
    if (response.success) {
      quickPublishMessage.value = '';
      await refreshTopics();
    }
  } catch (error) {
    console.error('Failed to publish message:', error);
  }
}

onMounted(() => {
  if (isConnected.value) {
    refreshTopics();
  }
});

watch(isConnected, (connected) => {
  if (connected) {
    refreshTopics();
  }
});
</script>
