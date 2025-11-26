<template>
  <div class="p-8">
    <div v-if="!isConnected" class="bg-warning/10 border border-warning rounded-xl p-4 mb-6">
      <div class="flex items-center gap-2">
        <i class="fas fa-exclamation-triangle text-warning"></i>
        <p class="text-text-primary">
          No server connected. Please select or add a server to view KV Store.
        </p>
      </div>
    </div>

    <div v-else class="space-y-6">
      <!-- Header -->
      <div class="flex items-center justify-between">
        <div>
          <h1 class="text-2xl font-bold text-text-primary">KV Store Inspector</h1>
          <p class="text-text-secondary mt-1">Browse and manage key-value pairs</p>
        </div>
        <div class="flex items-center gap-3">
          <button
            @click="refreshKeys"
            class="px-4 py-2 bg-bg-tertiary hover:bg-bg-hover border border-border rounded-lg text-text-primary text-sm transition-colors flex items-center gap-2"
            :disabled="loading"
          >
            <i class="fas fa-sync-alt" :class="{ 'fa-spin': loading }"></i>
            <span>Refresh</span>
          </button>
          <button
            @click="showAddKeyModal = true"
            class="px-4 py-2 bg-info hover:bg-info/80 text-white rounded-lg transition-colors flex items-center gap-2"
          >
            <i class="fas fa-plus"></i>
            <span>Add Key</span>
          </button>
        </div>
      </div>

      <!-- Statistics Cards -->
      <div class="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Total Keys</span>
            <i class="fas fa-key text-info"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ statistics.totalKeys }}</div>
        </div>

        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Total Size</span>
            <i class="fas fa-database text-success"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ formatBytes(statistics.totalSize) }}</div>
        </div>

        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Keys with TTL</span>
            <i class="fas fa-clock text-warning"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ statistics.keysWithTTL }}</div>
        </div>

        <div class="bg-bg-card border border-border rounded-lg p-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-text-secondary text-sm">Memory Usage</span>
            <i class="fas fa-memory text-error"></i>
          </div>
          <div class="text-2xl font-bold text-text-primary">{{ formatBytes(statistics.memoryUsage) }}</div>
        </div>
      </div>

      <!-- Search and Filter -->
      <div class="bg-bg-card border border-border rounded-lg p-4">
        <div class="flex items-center gap-4">
          <div class="flex-1 relative">
            <i class="fas fa-search absolute left-3 top-1/2 transform -translate-y-1/2 text-text-muted"></i>
            <input
              v-model="searchQuery"
              type="text"
              placeholder="Search keys..."
              class="w-full pl-10 pr-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary placeholder-text-muted focus:outline-none focus:border-border-focus"
              @input="filterKeys"
            />
          </div>
          <select
            v-model="filterType"
            @change="filterKeys"
            class="px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary text-sm focus:outline-none focus:border-border-focus"
          >
            <option value="all">All Keys</option>
            <option value="with-ttl">With TTL</option>
            <option value="no-ttl">No TTL</option>
          </select>
          <select
            v-model="sortBy"
            @change="sortKeys"
            class="px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary text-sm focus:outline-none focus:border-border-focus"
          >
            <option value="name">Sort by Name</option>
            <option value="size">Sort by Size</option>
            <option value="ttl">Sort by TTL</option>
          </select>
        </div>
      </div>

      <!-- Keys List -->
      <div class="bg-bg-card border border-border rounded-lg overflow-hidden">
        <div class="overflow-x-auto">
          <table class="w-full">
            <thead class="bg-bg-secondary border-b border-border">
              <tr>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">Key</th>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">Type</th>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">Size</th>
                <th class="px-4 py-3 text-left text-sm font-semibold text-text-primary">TTL</th>
                <th class="px-4 py-3 text-right text-sm font-semibold text-text-primary">Actions</th>
              </tr>
            </thead>
            <tbody>
              <tr v-if="loading" class="border-b border-border">
                <td colspan="5" class="px-4 py-8 text-center text-text-secondary">
                  <i class="fas fa-spinner fa-spin mr-2"></i>
                  Loading keys...
                </td>
              </tr>
              <tr v-else-if="filteredKeys.length === 0" class="border-b border-border">
                <td colspan="5" class="px-4 py-8 text-center text-text-secondary">
                  No keys found
                </td>
              </tr>
              <tr
                v-else
                v-for="key in paginatedKeys"
                :key="key.name"
                class="border-b border-border hover:bg-bg-hover transition-colors"
              >
                <td class="px-4 py-3">
                  <div class="flex items-center gap-2">
                    <i :class="getKeyIcon(key.type)" class="text-text-muted"></i>
                    <span class="text-text-primary font-mono text-sm">{{ key.name }}</span>
                  </div>
                </td>
                <td class="px-4 py-3">
                  <span class="px-2 py-1 bg-bg-tertiary text-text-secondary text-xs rounded">
                    {{ key.type || 'string' }}
                  </span>
                </td>
                <td class="px-4 py-3 text-text-secondary text-sm">
                  {{ formatBytes(key.size || 0) }}
                </td>
                <td class="px-4 py-3">
                  <span v-if="key.ttl" class="text-text-secondary text-sm">
                    {{ formatTTL(key.ttl) }}
                  </span>
                  <span v-else class="text-text-muted text-sm">No TTL</span>
                </td>
                <td class="px-4 py-3">
                  <div class="flex items-center justify-end gap-2">
                    <button
                      @click="viewKey(key.name)"
                      class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary hover:text-text-primary"
                      title="View"
                    >
                      <i class="fas fa-eye"></i>
                    </button>
                    <button
                      @click="editKey(key.name)"
                      class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary hover:text-info"
                      title="Edit"
                    >
                      <i class="fas fa-edit"></i>
                    </button>
                    <button
                      @click="deleteKey(key.name)"
                      class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary hover:text-error"
                      title="Delete"
                    >
                      <i class="fas fa-trash"></i>
                    </button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <!-- Pagination -->
        <div v-if="filteredKeys.length > pageSize" class="px-4 py-3 bg-bg-secondary border-t border-border flex items-center justify-between">
          <div class="text-text-secondary text-sm">
            Showing {{ (currentPage - 1) * pageSize + 1 }} to {{ Math.min(currentPage * pageSize, filteredKeys.length) }} of {{ filteredKeys.length }} keys
          </div>
          <div class="flex items-center gap-2">
            <button
              @click="currentPage--"
              :disabled="currentPage === 1"
              class="px-3 py-1 bg-bg-tertiary hover:bg-bg-hover border border-border rounded text-text-primary text-sm disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              <i class="fas fa-chevron-left"></i>
            </button>
            <span class="text-text-secondary text-sm">
              Page {{ currentPage }} of {{ totalPages }}
            </span>
            <button
              @click="currentPage++"
              :disabled="currentPage === totalPages"
              class="px-3 py-1 bg-bg-tertiary hover:bg-bg-hover border border-border rounded text-text-primary text-sm disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              <i class="fas fa-chevron-right"></i>
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Add/Edit Key Modal -->
    <div
      v-if="showAddKeyModal || editingKey"
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      @click.self="closeModal"
    >
      <div class="bg-bg-card border border-border rounded-lg p-6 w-full max-w-2xl max-h-[90vh] overflow-y-auto">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-xl font-bold text-text-primary">
            {{ editingKey ? 'Edit Key' : 'Add New Key' }}
          </h2>
          <button
            @click="closeModal"
            class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary"
          >
            <i class="fas fa-times"></i>
          </button>
        </div>

        <div class="space-y-4">
          <div>
            <label class="block text-text-secondary text-sm mb-2">Key Name</label>
            <input
              v-model="keyForm.name"
              type="text"
              :disabled="!!editingKey"
              class="w-full px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary focus:outline-none focus:border-border-focus disabled:opacity-50"
              placeholder="e.g., user:123"
            />
          </div>

          <div>
            <label class="block text-text-secondary text-sm mb-2">Value</label>
            <textarea
              v-model="keyForm.value"
              rows="6"
              class="w-full px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary focus:outline-none focus:border-border-focus font-mono text-sm"
              placeholder="Enter value..."
            ></textarea>
          </div>

          <div>
            <label class="block text-text-secondary text-sm mb-2">TTL (seconds, optional)</label>
            <input
              v-model.number="keyForm.ttl"
              type="number"
              min="0"
              class="w-full px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-text-primary focus:outline-none focus:border-border-focus"
              placeholder="Leave empty for no expiration"
            />
          </div>

          <div class="flex items-center justify-end gap-3 pt-4">
            <button
              @click="closeModal"
              class="px-4 py-2 bg-bg-tertiary hover:bg-bg-hover border border-border rounded-lg text-text-primary transition-colors"
            >
              Cancel
            </button>
            <button
              @click="saveKey"
              class="px-4 py-2 bg-info hover:bg-info/80 text-white rounded-lg transition-colors"
            >
              {{ editingKey ? 'Update' : 'Create' }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- View Key Modal -->
    <div
      v-if="viewingKey"
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      @click.self="viewingKey = null"
    >
      <div class="bg-bg-card border border-border rounded-lg p-6 w-full max-w-3xl max-h-[90vh] overflow-y-auto">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-xl font-bold text-text-primary">Key: {{ viewingKey }}</h2>
          <button
            @click="viewingKey = null"
            class="p-2 hover:bg-bg-hover rounded transition-colors text-text-secondary"
          >
            <i class="fas fa-times"></i>
          </button>
        </div>

        <div v-if="keyDetails" class="space-y-4">
          <div>
            <label class="block text-text-secondary text-sm mb-2">Value</label>
            <pre class="p-4 bg-bg-tertiary border border-border rounded-lg text-text-primary font-mono text-sm overflow-x-auto">{{ keyDetails.value }}</pre>
          </div>

          <div class="grid grid-cols-2 gap-4">
            <div>
              <label class="block text-text-secondary text-sm mb-2">Type</label>
              <div class="p-2 bg-bg-tertiary border border-border rounded-lg text-text-primary">
                {{ keyDetails.type || 'string' }}
              </div>
            </div>
            <div>
              <label class="block text-text-secondary text-sm mb-2">Size</label>
              <div class="p-2 bg-bg-tertiary border border-border rounded-lg text-text-primary">
                {{ formatBytes(keyDetails.size || 0) }}
              </div>
            </div>
            <div>
              <label class="block text-text-secondary text-sm mb-2">TTL</label>
              <div class="p-2 bg-bg-tertiary border border-border rounded-lg text-text-primary">
                {{ keyDetails.ttl ? formatTTL(keyDetails.ttl) : 'No TTL' }}
              </div>
            </div>
            <div>
              <label class="block text-text-secondary text-sm mb-2">Memory Usage</label>
              <div class="p-2 bg-bg-tertiary border border-border rounded-lg text-text-primary">
                {{ formatBytes(keyDetails.memoryUsage || 0) }}
              </div>
            </div>
          </div>
        </div>

        <div v-else class="text-center py-8 text-text-secondary">
          <i class="fas fa-spinner fa-spin mr-2"></i>
          Loading key details...
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { useServersStore } from '@/stores/servers';
import { useApi } from '@/composables/useApi';

interface KeyInfo {
  name: string;
  type?: string;
  size?: number;
  ttl?: number;
  memoryUsage?: number;
}

const serversStore = useServersStore();
const { apiClient, isConnected } = useApi();

const loading = ref(false);
const keys = ref<KeyInfo[]>([]);
const searchQuery = ref('');
const filterType = ref('all');
const sortBy = ref('name');
const currentPage = ref(1);
const pageSize = ref(50);
const showAddKeyModal = ref(false);
const editingKey = ref<string | null>(null);
const viewingKey = ref<string | null>(null);
const keyDetails = ref<KeyInfo | null>(null);

const keyForm = ref({
  name: '',
  value: '',
  ttl: undefined as number | undefined,
});

const statistics = computed(() => {
  const totalKeys = keys.value.length;
  const totalSize = keys.value.reduce((sum, key) => sum + (key.size || 0), 0);
  const keysWithTTL = keys.value.filter(key => key.ttl && key.ttl > 0).length;
  const memoryUsage = keys.value.reduce((sum, key) => sum + (key.memoryUsage || key.size || 0), 0);

  return {
    totalKeys,
    totalSize,
    keysWithTTL,
    memoryUsage,
  };
});

const filteredKeys = computed(() => {
  let result = [...keys.value];

  // Filter by search query
  if (searchQuery.value) {
    const query = searchQuery.value.toLowerCase();
    result = result.filter(key => key.name.toLowerCase().includes(query));
  }

  // Filter by type
  if (filterType.value === 'with-ttl') {
    result = result.filter(key => key.ttl && key.ttl > 0);
  } else if (filterType.value === 'no-ttl') {
    result = result.filter(key => !key.ttl || key.ttl <= 0);
  }

  // Sort
  result.sort((a, b) => {
    if (sortBy.value === 'name') {
      return a.name.localeCompare(b.name);
    } else if (sortBy.value === 'size') {
      return (b.size || 0) - (a.size || 0);
    } else if (sortBy.value === 'ttl') {
      return (b.ttl || 0) - (a.ttl || 0);
    }
    return 0;
  });

  return result;
});

const totalPages = computed(() => Math.ceil(filteredKeys.value.length / pageSize.value));

const paginatedKeys = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value;
  const end = start + pageSize.value;
  return filteredKeys.value.slice(start, end);
});

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}

function formatTTL(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`;
  return `${Math.floor(seconds / 86400)}d`;
}

function getKeyIcon(type?: string): string {
  const icons: Record<string, string> = {
    string: 'fas fa-font',
    hash: 'fas fa-layer-group',
    list: 'fas fa-list',
    set: 'fas fa-braille',
    zset: 'fas fa-sort-numeric-down',
  };
  return icons[type || 'string'] || 'fas fa-key';
}

async function refreshKeys(): Promise<void> {
  if (!apiClient.value) {
    console.warn('No API client available');
    return;
  }

  loading.value = true;
  try {
    // Get all keys from the server
    const response = await apiClient.value.getKvKeys();
    console.log('refreshKeys response:', response);
    
    if (response.success && response.data) {
      const keysList = response.data.keys || [];
      console.log(`Found ${keysList.length} keys`);
      
      if (keysList.length === 0) {
        keys.value = [];
        return;
      }
      
      // For each key, get additional info (type, TTL) - limit to 100 for performance
      const keyInfoPromises = keysList.slice(0, 100).map(async (keyName: string) => {
        const keyInfo: KeyInfo = { name: keyName };
        
        try {
          // Get key type
          const typeRes = await apiClient.value!.keyType(keyName);
          if (typeRes.success) {
            keyInfo.type = typeRes.data;
          }
          
          // Get TTL
          const ttlRes = await apiClient.value!.getKeyTtl(keyName);
          if (ttlRes.success && ttlRes.data !== undefined && ttlRes.data > 0) {
            keyInfo.ttl = ttlRes.data;
          }
          
          // Get value to calculate size
          const getRes = await apiClient.value!.get(keyName);
          console.log(`Key ${keyName} get result:`, getRes);
          
          if (getRes.success && getRes.data !== undefined && getRes.data !== null) {
            // Calculate size from value
            const value = getRes.data;
            let size = 0;
            
            try {
              if (typeof value === 'string') {
                size = new TextEncoder().encode(value).length;
              } else if (typeof value === 'object') {
                size = new TextEncoder().encode(JSON.stringify(value)).length;
              } else if (typeof value === 'number' || typeof value === 'boolean') {
                size = String(value).length;
              } else {
                size = String(value).length;
              }
              
              keyInfo.size = size;
              keyInfo.memoryUsage = size;
              console.log(`Key ${keyName} size calculated: ${size} bytes`);
            } catch (e) {
              console.warn(`Failed to calculate size for ${keyName}:`, e);
            }
          } else {
            console.warn(`Key ${keyName} has no value or get failed:`, getRes.error);
            // Fallback: try memoryUsage API
            const memRes = await apiClient.value!.memoryUsage(keyName);
            if (memRes.success && memRes.data) {
              keyInfo.size = memRes.data.bytes || 0;
              keyInfo.memoryUsage = memRes.data.bytes || 0;
            } else {
              // If both fail, set size to 0
              keyInfo.size = 0;
              keyInfo.memoryUsage = 0;
            }
          }
        } catch (e) {
          console.warn(`Failed to get info for key ${keyName}:`, e);
          // Continue with just the name
        }
        
        return keyInfo;
      });
      
      keys.value = await Promise.all(keyInfoPromises);
      console.log(`Loaded ${keys.value.length} keys with details`);
    } else {
      console.error('Failed to get keys:', response.error);
      keys.value = [];
    }
  } catch (error) {
    console.error('Failed to load keys:', error);
    keys.value = [];
  } finally {
    loading.value = false;
  }
}

function filterKeys(): void {
  currentPage.value = 1;
}

function sortKeys(): void {
  currentPage.value = 1;
}

async function viewKey(keyName: string): Promise<void> {
  viewingKey.value = keyName;
  keyDetails.value = null;

  if (!apiClient.value) return;

  try {
    const response = await apiClient.value.get(keyName);
    if (response.success && response.data) {
      const key = keys.value.find(k => k.name === keyName);
      keyDetails.value = {
        name: keyName,
        value: response.data,
        type: key?.type,
        size: key?.size,
        ttl: key?.ttl,
        memoryUsage: key?.memoryUsage,
      };
    }
  } catch (error) {
    console.error('Failed to load key details:', error);
  }
}

function editKey(keyName: string): void {
  const key = keys.value.find(k => k.name === keyName);
  editingKey.value = keyName;
  keyForm.value.name = keyName;
  keyForm.value.value = '';
  keyForm.value.ttl = key?.ttl;

  // Load current value
  if (apiClient.value) {
    apiClient.value.get(keyName).then(response => {
      if (response.success && response.data) {
        keyForm.value.value = response.data;
      }
    });
  }
}

async function deleteKey(keyName: string): Promise<void> {
  if (!confirm(`Are you sure you want to delete key "${keyName}"?`)) return;

  if (!apiClient.value) return;

  try {
    const response = await apiClient.value.delete(keyName);
    if (response.success) {
      keys.value = keys.value.filter(k => k.name !== keyName);
    }
  } catch (error) {
    console.error('Failed to delete key:', error);
  }
}

async function saveKey(): Promise<void> {
  if (!apiClient.value || !keyForm.value.name || !keyForm.value.value) return;

  try {
    const response = await apiClient.value.set(
      keyForm.value.name,
      keyForm.value.value,
      keyForm.value.ttl
    );

    if (response.success) {
      closeModal();
      await refreshKeys();
    }
  } catch (error) {
    console.error('Failed to save key:', error);
  }
}

function closeModal(): void {
  showAddKeyModal.value = false;
  editingKey.value = null;
  keyForm.value = {
    name: '',
    value: '',
    ttl: undefined,
  };
}

onMounted(() => {
  if (isConnected.value) {
    refreshKeys();
  }
});

// Watch for connection changes
watch(isConnected, (connected) => {
  if (connected) {
    refreshKeys();
  } else {
    keys.value = [];
  }
});
</script>
