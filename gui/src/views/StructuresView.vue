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

      <div v-if="currentKey && !loading && (keyInfo || inspectError)" class="text-sm">
        <p v-if="keyInfo" class="text-text-secondary">
          <span class="font-mono text-text-primary">{{ currentKey }}</span>
          &middot; type <strong class="text-text-primary">{{ keyInfo.type }}</strong>
          &middot; memory <strong class="text-text-primary">{{ keyInfo.memory }}</strong>
        </p>
        <p v-if="inspectError" class="text-error">
          <i class="fas fa-exclamation-circle mr-1"></i>{{ inspectError }}
        </p>
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
                  <td class="py-2 px-3 font-mono text-text-secondary max-w-md truncate">{{ formatValue(value) }}</td>
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
          No data to show for this key
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
                <span class="font-mono text-text-primary">{{ formatValue(item) }}</span>
              </div>
              <button @click="removeListItem(index)" class="text-error hover:text-error/80">
                <i class="fas fa-trash"></i>
              </button>
            </div>
          </div>
        </div>

        <div v-else class="text-center py-8 text-text-muted">
          No data to show for this key
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
              :key="formatValue(member)"
              class="flex items-center gap-2 px-3 py-1.5 bg-bg-tertiary rounded-lg"
            >
              <span class="font-mono text-text-primary">{{ formatValue(member) }}</span>
              <button @click="removeSetMember(member)" class="text-error hover:text-error/80">
                <i class="fas fa-times text-xs"></i>
              </button>
            </div>
          </div>
        </div>

        <div v-else class="text-center py-8 text-text-muted">
          No data to show for this key
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
          No data to show for this key
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { useApi } from '@/composables/useApi';
import type { ApiResponse, KeyTypeName, ScoredMember } from '@/services/api';

const { apiClient, isConnected } = useApi();

type TabId = 'hash' | 'list' | 'set' | 'zset';

const tabs: { id: TabId; label: string; icon: string }[] = [
  { id: 'hash', label: 'Hash', icon: 'fas fa-hashtag' },
  { id: 'list', label: 'List', icon: 'fas fa-list' },
  { id: 'set', label: 'Set', icon: 'fas fa-layer-group' },
  { id: 'zset', label: 'Sorted Set', icon: 'fas fa-sort-amount-down' },
];

/** What `key.type` calls each tab's structure. */
const TAB_KEY_TYPES: Record<TabId, KeyTypeName> = {
  hash: 'hash',
  list: 'list',
  set: 'set',
  zset: 'zset',
};

const activeTab = ref<TabId>('hash');
const searchKey = ref('');
const currentKey = ref('');
const loading = ref(false);
const inspectError = ref('');
const keyInfo = ref<{ type: KeyTypeName; memory: string } | null>(null);

// Data for each structure type
const hashData = ref<Record<string, unknown> | null>(null);
const listData = ref<unknown[] | null>(null);
const setData = ref<unknown[] | null>(null);
const zsetData = ref<ScoredMember[] | null>(null);

// Clear data when switching tabs
watch(activeTab, () => {
  currentKey.value = '';
  keyInfo.value = null;
  inspectError.value = '';
  clearAllData();
});

/**
 * Inspect `searchKey`: its type through `key.type`, its memory through
 * `memory.usage`, then the contents of the structure the active tab shows.
 */
async function inspectKey(): Promise<void> {
  const client = apiClient.value;
  if (!client || !searchKey.value) return;

  const key = searchKey.value;
  currentKey.value = key;
  loading.value = true;
  inspectError.value = '';
  keyInfo.value = null;
  clearAllData();

  try {
    const typeResponse = await client.keyType(key);
    if (!typeResponse.success || !typeResponse.data) {
      inspectError.value = typeResponse.error || `Could not read the type of "${key}"`;
      return;
    }
    const keyType = typeResponse.data;
    if (keyType === 'none') {
      inspectError.value = `Key "${key}" does not exist`;
      return;
    }

    const memory = await client.memoryUsage(key);
    keyInfo.value = {
      type: keyType,
      memory: memory.success && memory.data ? memory.data.human : 'unavailable',
    };
    if (!memory.success) {
      inspectError.value = memory.error || `Could not read the memory usage of "${key}"`;
    }

    const wanted = TAB_KEY_TYPES[activeTab.value];
    if (keyType !== wanted) {
      inspectError.value = `Key "${key}" is a ${keyType}, not a ${wanted}`;
      return;
    }
    await fetchActiveData();
  } catch (error) {
    inspectError.value = error instanceof Error ? error.message : String(error);
  } finally {
    loading.value = false;
  }
}

function clearAllData(): void {
  hashData.value = null;
  listData.value = null;
  setData.value = null;
  zsetData.value = null;
}

/** Store `response.data` through `assign`, or surface the server's error. */
function apply<T>(response: ApiResponse<T>, assign: (data: T) => void): void {
  if (response.success && response.data !== undefined) {
    assign(response.data);
  } else {
    inspectError.value = response.error || 'The server returned no data';
  }
}

async function fetchActiveData(): Promise<void> {
  switch (activeTab.value) {
    case 'hash':
      return fetchHashData();
    case 'list':
      return fetchListData();
    case 'set':
      return fetchSetData();
    case 'zset':
      return fetchZSetData();
  }
}

async function fetchHashData(): Promise<void> {
  if (!apiClient.value) return;
  apply(await apiClient.value.hashGetAll(currentKey.value), (data) => (hashData.value = data));
}

async function fetchListData(): Promise<void> {
  if (!apiClient.value) return;
  apply(await apiClient.value.listRange(currentKey.value), (data) => (listData.value = data));
}

async function fetchSetData(): Promise<void> {
  if (!apiClient.value) return;
  apply(await apiClient.value.setMembers(currentKey.value), (data) => (setData.value = data));
}

async function fetchZSetData(): Promise<void> {
  if (!apiClient.value) return;
  apply(await apiClient.value.zsetRange(currentKey.value), (data) => (zsetData.value = data));
}

/** Run a write, then reload the structure; a refused write is shown, not swallowed. */
async function mutate(write: Promise<ApiResponse<boolean>>, reload: () => Promise<void>): Promise<void> {
  const response = await write;
  if (!response.success) {
    inspectError.value = response.error || 'The server refused the change';
    return;
  }
  inspectError.value = '';
  await reload();
}

function formatValue(value: unknown): string {
  return typeof value === 'string' ? value : JSON.stringify(value);
}

// Hash operations
function addHashField(): void {
  const field = prompt('Enter field name:');
  const value = prompt('Enter value:');
  if (field && value && apiClient.value) {
    mutate(apiClient.value.hashSet(currentKey.value, field, value), fetchHashData);
  }
}

function deleteHashField(field: string): void {
  if (confirm(`Delete field "${field}"?`) && apiClient.value) {
    mutate(apiClient.value.hashDelete(currentKey.value, field), fetchHashData);
  }
}

// List operations
function pushToList(side: 'left' | 'right'): void {
  const value = prompt('Enter value to push:');
  if (value && apiClient.value) {
    mutate(apiClient.value.listPush(currentKey.value, side, value), fetchListData);
  }
}

function removeListItem(index: number): void {
  if (confirm(`Remove item at index ${index}?`) && apiClient.value && listData.value) {
    mutate(apiClient.value.listRemove(currentKey.value, listData.value[index]), fetchListData);
  }
}

// Set operations
function addSetMember(): void {
  const member = prompt('Enter member to add:');
  if (member && apiClient.value) {
    mutate(apiClient.value.setAdd(currentKey.value, member), fetchSetData);
  }
}

function removeSetMember(member: unknown): void {
  if (confirm(`Remove member "${formatValue(member)}"?`) && apiClient.value) {
    mutate(apiClient.value.setRemove(currentKey.value, member), fetchSetData);
  }
}

// Sorted Set operations
function addZSetMember(): void {
  const member = prompt('Enter member:');
  const scoreStr = prompt('Enter score:');
  if (member && scoreStr && apiClient.value) {
    const score = parseFloat(scoreStr);
    if (Number.isNaN(score)) {
      inspectError.value = `"${scoreStr}" is not a number`;
      return;
    }
    mutate(apiClient.value.zsetAdd(currentKey.value, member, score), fetchZSetData);
  }
}

function removeZSetMember(member: string): void {
  if (confirm(`Remove member "${member}"?`) && apiClient.value) {
    mutate(apiClient.value.zsetRemove(currentKey.value, member), fetchZSetData);
  }
}
</script>

