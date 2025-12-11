<template>
  <div class="p-6">
    <div class="mb-6">
      <h1 class="text-2xl font-bold text-text-primary">Data Structures</h1>
      <p class="text-text-secondary mt-1">Inspect Hash, List, Set, and Sorted Set structures</p>
    </div>

    <div v-if="!isConnected" class="bg-warning/10 border border-warning rounded-xl p-4 mb-6">
      <div class="flex items-center gap-2">
        <i class="fas fa-exclamation-triangle text-warning"></i>
        <p class="text-text-primary">
          No server connected. Please select or add a server to view data structures.
        </p>
      </div>
    </div>

    <div v-else class="space-y-6">
      <!-- Structure Type Tabs -->
      <div class="flex gap-2 border-b border-border pb-4">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          @click="activeTab = tab.id"
          :class="[
            'px-4 py-2 rounded-lg flex items-center gap-2 transition-colors',
            activeTab === tab.id
              ? 'bg-info text-white'
              : 'bg-bg-tertiary text-text-secondary hover:bg-bg-hover hover:text-text-primary'
          ]"
        >
          <i :class="tab.icon"></i>
          {{ tab.label }}
        </button>
      </div>

      <!-- Key Search -->
      <div class="flex gap-4">
        <div class="flex-1">
          <input
            v-model="searchKey"
            type="text"
            placeholder="Enter key name to inspect..."
            class="w-full px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary placeholder-text-muted focus:outline-none focus:border-border-focus"
            @keyup.enter="inspectKey"
          />
        </div>
        <button
          @click="inspectKey"
          :disabled="!searchKey"
          class="px-6 py-2 bg-info text-white rounded-lg hover:bg-info/80 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
        >
          <i class="fas fa-search"></i>
          Inspect
        </button>
      </div>

      <!-- Hash Inspector -->
      <div v-if="activeTab === 'hash'" class="bg-bg-card border border-border rounded-xl p-6">
        <h3 class="text-lg font-semibold text-text-primary mb-4 flex items-center gap-2">
          <i class="fas fa-hashtag text-info"></i>
          Hash Inspector
        </h3>

        <div v-if="!currentKey" class="text-center py-8 text-text-muted">
          Enter a key name above to inspect its hash fields
        </div>

        <div v-else-if="loading" class="text-center py-8">
          <i class="fas fa-spinner fa-spin text-2xl text-info"></i>
        </div>

        <div v-else-if="hashData">
          <div class="mb-4 flex items-center justify-between">
            <span class="text-text-secondary">
              <strong class="text-text-primary">{{ Object.keys(hashData).length }}</strong> fields
            </span>
            <button @click="addHashField" class="text-sm text-info hover:underline">
              <i class="fas fa-plus mr-1"></i> Add Field
            </button>
          </div>
          <div class="overflow-x-auto">
            <table class="w-full">
              <thead>
                <tr class="border-b border-border">
                  <th class="text-left py-2 px-3 text-text-secondary font-medium">Field</th>
                  <th class="text-left py-2 px-3 text-text-secondary font-medium">Value</th>
                  <th class="text-right py-2 px-3 text-text-secondary font-medium">Actions</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(value, field) in hashData" :key="field" class="border-b border-border hover:bg-bg-hover">
                  <td class="py-2 px-3 font-mono text-text-primary">{{ field }}</td>
                  <td class="py-2 px-3 font-mono text-text-secondary max-w-md truncate">{{ value }}</td>
                  <td class="py-2 px-3 text-right">
                    <button @click="deleteHashField(String(field))" class="text-error hover:text-error/80">
                      <i class="fas fa-trash"></i>
                    </button>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>

        <div v-else class="text-center py-8 text-text-muted">
          Key not found or is not a hash
        </div>
      </div>

      <!-- List Inspector -->
      <div v-if="activeTab === 'list'" class="bg-bg-card border border-border rounded-xl p-6">
        <h3 class="text-lg font-semibold text-text-primary mb-4 flex items-center gap-2">
          <i class="fas fa-list text-success"></i>
          List Inspector
        </h3>

        <div v-if="!currentKey" class="text-center py-8 text-text-muted">
          Enter a key name above to inspect its list elements
        </div>

        <div v-else-if="loading" class="text-center py-8">
          <i class="fas fa-spinner fa-spin text-2xl text-success"></i>
        </div>

        <div v-else-if="listData">
          <div class="mb-4 flex items-center justify-between">
            <span class="text-text-secondary">
              <strong class="text-text-primary">{{ listData.length }}</strong> elements
            </span>
            <div class="flex gap-2">
              <button @click="pushToList('left')" class="text-sm text-success hover:underline">
                <i class="fas fa-arrow-left mr-1"></i> LPUSH
              </button>
              <button @click="pushToList('right')" class="text-sm text-success hover:underline">
                RPUSH <i class="fas fa-arrow-right ml-1"></i>
              </button>
            </div>
          </div>
          <div class="space-y-2 max-h-96 overflow-y-auto">
            <div
              v-for="(item, index) in listData"
              :key="index"
              class="flex items-center justify-between p-3 bg-bg-tertiary rounded-lg"
            >
              <div class="flex items-center gap-3">
                <span class="text-text-muted text-sm w-8">{{ index }}</span>
                <span class="font-mono text-text-primary">{{ item }}</span>
              </div>
              <button @click="removeListItem(index)" class="text-error hover:text-error/80">
                <i class="fas fa-trash"></i>
              </button>
            </div>
          </div>
        </div>

        <div v-else class="text-center py-8 text-text-muted">
          Key not found or is not a list
        </div>
      </div>

      <!-- Set Inspector -->
      <div v-if="activeTab === 'set'" class="bg-bg-card border border-border rounded-xl p-6">
        <h3 class="text-lg font-semibold text-text-primary mb-4 flex items-center gap-2">
          <i class="fas fa-layer-group text-warning"></i>
          Set Inspector
        </h3>

        <div v-if="!currentKey" class="text-center py-8 text-text-muted">
          Enter a key name above to inspect its set members
        </div>

        <div v-else-if="loading" class="text-center py-8">
          <i class="fas fa-spinner fa-spin text-2xl text-warning"></i>
        </div>

        <div v-else-if="setData">
          <div class="mb-4 flex items-center justify-between">
            <span class="text-text-secondary">
              <strong class="text-text-primary">{{ setData.length }}</strong> members
            </span>
            <button @click="addSetMember" class="text-sm text-warning hover:underline">
              <i class="fas fa-plus mr-1"></i> Add Member
            </button>
          </div>
          <div class="flex flex-wrap gap-2 max-h-96 overflow-y-auto">
            <div
              v-for="member in setData"
              :key="member"
              class="flex items-center gap-2 px-3 py-1.5 bg-bg-tertiary rounded-lg"
            >
              <span class="font-mono text-text-primary">{{ member }}</span>
              <button @click="removeSetMember(member)" class="text-error hover:text-error/80">
                <i class="fas fa-times text-xs"></i>
              </button>
            </div>
          </div>
        </div>

        <div v-else class="text-center py-8 text-text-muted">
          Key not found or is not a set
        </div>
      </div>

      <!-- Sorted Set Inspector -->
      <div v-if="activeTab === 'zset'" class="bg-bg-card border border-border rounded-xl p-6">
        <h3 class="text-lg font-semibold text-text-primary mb-4 flex items-center gap-2">
          <i class="fas fa-sort-amount-down text-error"></i>
          Sorted Set Inspector
        </h3>

        <div v-if="!currentKey" class="text-center py-8 text-text-muted">
          Enter a key name above to inspect its sorted set members
        </div>

        <div v-else-if="loading" class="text-center py-8">
          <i class="fas fa-spinner fa-spin text-2xl text-error"></i>
        </div>

        <div v-else-if="zsetData">
          <div class="mb-4 flex items-center justify-between">
            <span class="text-text-secondary">
              <strong class="text-text-primary">{{ zsetData.length }}</strong> members
            </span>
            <button @click="addZSetMember" class="text-sm text-error hover:underline">
              <i class="fas fa-plus mr-1"></i> Add Member
            </button>
          </div>
          <div class="overflow-x-auto">
            <table class="w-full">
              <thead>
                <tr class="border-b border-border">
                  <th class="text-left py-2 px-3 text-text-secondary font-medium">Rank</th>
                  <th class="text-left py-2 px-3 text-text-secondary font-medium">Member</th>
                  <th class="text-left py-2 px-3 text-text-secondary font-medium">Score</th>
                  <th class="text-right py-2 px-3 text-text-secondary font-medium">Actions</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(item, index) in zsetData" :key="item.member" class="border-b border-border hover:bg-bg-hover">
                  <td class="py-2 px-3 text-text-muted">{{ index + 1 }}</td>
                  <td class="py-2 px-3 font-mono text-text-primary">{{ item.member }}</td>
                  <td class="py-2 px-3 font-mono text-text-secondary">{{ item.score }}</td>
                  <td class="py-2 px-3 text-right">
                    <button @click="removeZSetMember(item.member)" class="text-error hover:text-error/80">
                      <i class="fas fa-trash"></i>
                    </button>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>

        <div v-else class="text-center py-8 text-text-muted">
          Key not found or is not a sorted set
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { useApi } from '@/composables/useApi';

const { apiClient, isConnected } = useApi();

interface ZSetMember {
  member: string;
  score: number;
}

const tabs = [
  { id: 'hash', label: 'Hash', icon: 'fas fa-hashtag' },
  { id: 'list', label: 'List', icon: 'fas fa-list' },
  { id: 'set', label: 'Set', icon: 'fas fa-layer-group' },
  { id: 'zset', label: 'Sorted Set', icon: 'fas fa-sort-amount-down' },
];

const activeTab = ref('hash');
const searchKey = ref('');
const currentKey = ref('');
const loading = ref(false);

// Data for each structure type
const hashData = ref<Record<string, string> | null>(null);
const listData = ref<string[] | null>(null);
const setData = ref<string[] | null>(null);
const zsetData = ref<ZSetMember[] | null>(null);

// Clear data when switching tabs
watch(activeTab, () => {
  currentKey.value = '';
  hashData.value = null;
  listData.value = null;
  setData.value = null;
  zsetData.value = null;
});

async function inspectKey() {
  if (!apiClient.value || !searchKey.value) return;

  currentKey.value = searchKey.value;
  loading.value = true;

  try {
    // First, get the key type
    const typeResponse = await apiClient.value.keyType(searchKey.value);
    if (!typeResponse.success) {
      clearAllData();
      return;
    }

    const keyType = typeResponse.data;

    // Fetch data based on type and active tab
    if (activeTab.value === 'hash' && keyType === 'hash') {
      await fetchHashData();
    } else if (activeTab.value === 'list' && keyType === 'list') {
      await fetchListData();
    } else if (activeTab.value === 'set' && keyType === 'set') {
      await fetchSetData();
    } else if (activeTab.value === 'zset' && keyType === 'zset') {
      await fetchZSetData();
    } else {
      clearAllData();
    }
  } catch (error) {
    console.error('Failed to inspect key:', error);
    clearAllData();
  } finally {
    loading.value = false;
  }
}

function clearAllData() {
  hashData.value = null;
  listData.value = null;
  setData.value = null;
  zsetData.value = null;
}

async function fetchHashData() {
  if (!apiClient.value) return;
  try {
    const response = await apiClient.value.request({
      method: 'GET',
      url: `/hash/${encodeURIComponent(currentKey.value)}/getall`,
    });
    if (response.success) {
      hashData.value = response.data;
    }
  } catch (error) {
    console.error('Failed to fetch hash data:', error);
  }
}

async function fetchListData() {
  if (!apiClient.value) return;
  try {
    const response = await apiClient.value.request({
      method: 'POST',
      url: `/list/${encodeURIComponent(currentKey.value)}/range`,
      data: { start: 0, stop: -1 },
    });
    if (response.success) {
      listData.value = response.data;
    }
  } catch (error) {
    console.error('Failed to fetch list data:', error);
  }
}

async function fetchSetData() {
  if (!apiClient.value) return;
  try {
    const response = await apiClient.value.request({
      method: 'GET',
      url: `/set/${encodeURIComponent(currentKey.value)}/members`,
    });
    if (response.success) {
      setData.value = response.data;
    }
  } catch (error) {
    console.error('Failed to fetch set data:', error);
  }
}

async function fetchZSetData() {
  if (!apiClient.value) return;
  try {
    const response = await apiClient.value.request({
      method: 'POST',
      url: `/sortedset/${encodeURIComponent(currentKey.value)}/range`,
      data: { start: 0, stop: -1, with_scores: true },
    });
    if (response.success) {
      zsetData.value = response.data;
    }
  } catch (error) {
    console.error('Failed to fetch sorted set data:', error);
  }
}

// Hash operations
function addHashField() {
  const field = prompt('Enter field name:');
  const value = prompt('Enter value:');
  if (field && value && apiClient.value) {
    apiClient.value.request({
      method: 'POST',
      url: `/hash/${encodeURIComponent(currentKey.value)}/set`,
      data: { field, value },
    }).then(() => fetchHashData());
  }
}

function deleteHashField(field: string) {
  if (confirm(`Delete field "${field}"?`) && apiClient.value) {
    // OpenAPI: POST /hash/{key}/del with { fields: [field] }
    apiClient.value.request({
      method: 'POST',
      url: `/hash/${encodeURIComponent(currentKey.value)}/del`,
      data: { fields: [field] },
    }).then(() => fetchHashData());
  }
}

// List operations
function pushToList(side: 'left' | 'right') {
  const value = prompt('Enter value to push:');
  if (value && apiClient.value) {
    const endpoint = side === 'left' ? 'lpush' : 'rpush';
    // OpenAPI: POST /list/{key}/lpush or /list/{key}/rpush with { values: [value] }
    apiClient.value.request({
      method: 'POST',
      url: `/list/${encodeURIComponent(currentKey.value)}/${endpoint}`,
      data: { values: [value] },
    }).then(() => fetchListData());
  }
}

function removeListItem(index: number) {
  if (confirm(`Remove item at index ${index}?`) && apiClient.value && listData.value) {
    const value = listData.value[index];
    // OpenAPI: POST /list/{key}/lrem with { count: 1, value }
    apiClient.value.request({
      method: 'POST',
      url: `/list/${encodeURIComponent(currentKey.value)}/lrem`,
      data: { count: 1, value },
    }).then(() => fetchListData());
  }
}

// Set operations
function addSetMember() {
  const member = prompt('Enter member to add:');
  if (member && apiClient.value) {
    // OpenAPI: POST /set/{key}/sadd with { members: [member] }
    apiClient.value.request({
      method: 'POST',
      url: `/set/${encodeURIComponent(currentKey.value)}/sadd`,
      data: { members: [member] },
    }).then(() => fetchSetData());
  }
}

function removeSetMember(member: string) {
  if (confirm(`Remove member "${member}"?`) && apiClient.value) {
    // OpenAPI: POST /set/{key}/srem with { members: [member] }
    apiClient.value.request({
      method: 'POST',
      url: `/set/${encodeURIComponent(currentKey.value)}/srem`,
      data: { members: [member] },
    }).then(() => fetchSetData());
  }
}

// Sorted Set operations
function addZSetMember() {
  const member = prompt('Enter member:');
  const scoreStr = prompt('Enter score:');
  if (member && scoreStr && apiClient.value) {
    const score = parseFloat(scoreStr);
    // OpenAPI: POST /sortedset/{key}/zadd with { members: [{ member, score }] }
    apiClient.value.request({
      method: 'POST',
      url: `/sortedset/${encodeURIComponent(currentKey.value)}/zadd`,
      data: { members: [{ member, score }] },
    }).then(() => fetchZSetData());
  }
}

function removeZSetMember(member: string) {
  if (confirm(`Remove member "${member}"?`) && apiClient.value) {
    // OpenAPI: POST /sortedset/{key}/zrem with { members: [member] }
    apiClient.value.request({
      method: 'POST',
      url: `/sortedset/${encodeURIComponent(currentKey.value)}/zrem`,
      data: { members: [member] },
    }).then(() => fetchZSetData());
  }
}
</script>

