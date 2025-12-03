<template>
  <div id="app" class="flex flex-col h-screen bg-bg-primary text-text-primary">
    <!-- Custom Titlebar -->
    <div class="h-8 bg-bg-secondary border-b border-border flex items-center justify-between px-4 drag-region flex-shrink-0">
      <div class="flex items-center gap-2 text-xs text-text-secondary">
        <i class="fas fa-cube"></i>
        <span>Synap Desktop</span>
      </div>
      <div class="flex items-center gap-1 no-drag">
        <button @click="minimizeWindow" class="p-1 w-8 h-8 hover:bg-bg-hover transition-colors rounded">
          <i class="fas fa-window-minimize text-xs"></i>
        </button>
        <button @click="maximizeWindow" class="p-1 w-8 h-8 hover:bg-bg-hover transition-colors rounded">
          <i class="fas fa-window-maximize text-xs"></i>
        </button>
        <button @click="closeWindow" class="p-1 w-8 h-8 hover:bg-red-600 hover:text-white transition-colors rounded">
          <i class="fas fa-times text-xs"></i>
        </button>
      </div>
    </div>

    <!-- Main Content -->
    <div class="flex flex-1 min-h-0">
      <!-- Sidebar -->
      <aside class="w-64 bg-bg-secondary border-r border-border flex flex-col">
        <!-- Server Selector -->
        <div class="h-14 flex items-center px-4 border-b border-border flex-shrink-0">
          <div class="connection-dropdown relative w-full" :class="{ 'z-dropdown': isDropdownOpen }">
            <button 
              @click="serverList.length === 0 ? openServerModal() : toggleDropdown()" 
              class="w-full flex items-center justify-between gap-3 p-3 rounded-lg bg-bg-tertiary hover:bg-bg-hover transition-colors cursor-pointer"
            >
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2 whitespace-nowrap overflow-hidden text-ellipsis">
                  <span class="text-sm font-medium text-text-primary">{{ activeServer?.name || 'Select Server' }}</span>
                  <span class="text-text-muted">•</span>
                  <span :class="['w-2 h-2 rounded-full', activeServer?.status === 'online' ? 'bg-success' : 'bg-text-muted']"></span>
                  <span class="text-xs text-text-secondary">{{ activeServer?.status || 'No servers' }}</span>
                </div>
              </div>
              <i class="fas fa-chevron-down text-xs text-text-muted transition-transform" :class="{ 'rotate-180': isDropdownOpen }"></i>
            </button>
            
            <div v-if="isDropdownOpen" class="absolute top-full left-0 right-0 mt-1 bg-bg-elevated border border-border rounded-lg shadow-lg z-dropdown">
              <div v-if="serverList.length === 0" class="p-4 text-center text-text-secondary">
                <i class="fas fa-exclamation-circle mb-2 block"></i>
                <span class="text-sm">No servers available</span>
              </div>
              <div v-else>
                <div 
                  v-for="server in serverList" 
                  :key="server.id"
                  :class="['flex items-center justify-between p-3 hover:bg-bg-hover cursor-pointer transition-colors', { 'bg-bg-hover': activeServerId === server.id }]"
                  @click="selectServer(server.id)"
                >
                  <div class="flex-1 min-w-0">
                    <div class="text-sm font-medium text-text-primary truncate">{{ server.name }}</div>
                    <div class="flex items-center gap-2 text-xs text-text-secondary">
                      <span :class="['w-1.5 h-1.5 rounded-full', server.status === 'online' ? 'bg-success' : 'bg-text-muted']"></span>
                      {{ server.url }}{{ server.port ? `:${server.port}` : '' }}
                    </div>
                  </div>
                  <div v-if="activeServerId === server.id" class="text-success">
                    <i class="fas fa-check text-sm"></i>
                  </div>
                </div>
              </div>
              
              <div class="border-t border-border p-3">
                <button @click="openServerModal" class="w-full flex items-center gap-2 px-3 py-2 text-sm text-text-secondary hover:text-text-primary hover:bg-bg-hover rounded transition-colors">
                  <i class="fas fa-cog"></i>
                  Manage Servers
                </button>
              </div>
            </div>
          </div>
        </div>

        <!-- Navigation -->
        <nav class="flex-1 flex flex-col min-h-0 p-4">
          <router-link 
            v-for="item in menuItems" 
            :key="item.path"
            :to="item.path" 
            class="flex items-center gap-3 py-2 text-text-secondary hover:text-text-primary transition-colors cursor-pointer text-sm rounded px-2"
            :class="{ 'bg-bg-hover text-text-primary': isActive(item.path) }"
          >
            <i :class="[item.icon, 'w-4 text-center']"></i>
            <span>{{ item.label }}</span>
          </router-link>
        </nav>
      </aside>

      <!-- Main Content -->
      <main class="flex-1 flex flex-col">
        <header class="h-14 border-b border-border flex items-center justify-between px-6 bg-bg-secondary">
          <div class="flex items-center gap-3">
            <i :class="[pageIcon, 'text-text-secondary']"></i>
            <span class="text-lg font-semibold text-text-primary">{{ pageTitle }}</span>
          </div>
          <div class="flex items-center gap-2">
            <!-- Global connection status -->
            <div class="flex items-center gap-2 ml-4 pl-4 border-l border-border">
              <span :class="['w-2 h-2 rounded-full', isConnected ? 'bg-success' : 'bg-text-muted']"></span>
              <span class="text-xs text-text-secondary">{{ isConnected ? 'Connected' : 'Disconnected' }}</span>
            </div>
          </div>
        </header>

        <div class="flex-1 overflow-y-auto">
          <router-view />
        </div>
      </main>
    </div>

    <!-- Server Connection Modal -->
    <ServerConnectionModal
      :is-open="showServerModal"
      :server-id="editingServerId"
      @close="showServerModal = false"
      @saved="handleServerSaved"
    />

    <!-- Notification Center -->
    <NotificationCenter />

    <!-- Theme Toggle Button -->
    <button
      @click="toggleTheme"
      class="fixed bottom-4 right-4 w-12 h-12 rounded-full bg-bg-elevated border border-border hover:bg-bg-hover transition-colors flex items-center justify-center shadow-lg z-40"
      title="Toggle theme (Ctrl+T)"
    >
      <i :class="theme === 'dark' ? 'fas fa-sun text-warning' : 'fas fa-moon text-info'"></i>
    </button>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useRoute } from 'vue-router';
import { useServersStore } from '@/stores/servers';
import { useThemeStore } from '@/stores/theme';
import ServerConnectionModal from '@/components/ServerConnectionModal.vue';
import NotificationCenter from '@/components/NotificationCenter.vue';
import { ipcBridge } from '@/services/ipc';
import { useKeyboardShortcuts } from '@/composables/useKeyboardShortcuts';

const route = useRoute();
const serversStore = useServersStore();
const themeStore = useThemeStore();

const serverList = computed(() => serversStore.serverList);
const activeServerId = computed(() => serversStore.activeServerId);
const activeServer = computed(() => serversStore.activeServer);
const isConnected = computed(() => activeServer.value?.connected || false);
const theme = computed(() => themeStore.theme);

const isDropdownOpen = ref(false);
const showServerModal = ref(false);
const editingServerId = ref<string | undefined>(undefined);

const menuItems = [
  { path: '/', label: 'Dashboard', icon: 'fas fa-tachometer-alt' },
  { path: '/metrics', label: 'Metrics', icon: 'fas fa-chart-line' },
  { path: '/kv-store', label: 'KV Store', icon: 'fas fa-database' },
  { path: '/structures', label: 'Structures', icon: 'fas fa-layer-group' },
  { path: '/queues', label: 'Queues', icon: 'fas fa-inbox' },
  { path: '/streams', label: 'Streams', icon: 'fas fa-stream' },
  { path: '/pubsub', label: 'Pub/Sub', icon: 'fas fa-broadcast-tower' },
  { path: '/replication', label: 'Replication', icon: 'fas fa-network-wired' },
  { path: '/logs', label: 'Logs', icon: 'fas fa-file-alt' },
  { path: '/config', label: 'Configuration', icon: 'fas fa-cog' },
];

const pageTitle = computed(() => {
  const titles: Record<string, string> = {
    '/': 'Dashboard',
    '/metrics': 'Metrics',
    '/kv-store': 'KV Store',
    '/structures': 'Data Structures',
    '/queues': 'Queues',
    '/streams': 'Streams',
    '/pubsub': 'Pub/Sub',
    '/replication': 'Replication',
    '/logs': 'Logs',
    '/config': 'Configuration',
  };
  return titles[route.path] || 'Synap Desktop';
});

const pageIcon = computed(() => {
  const icons: Record<string, string> = {
    '/': 'fas fa-tachometer-alt',
    '/metrics': 'fas fa-chart-line',
    '/kv-store': 'fas fa-database',
    '/structures': 'fas fa-layer-group',
    '/queues': 'fas fa-inbox',
    '/streams': 'fas fa-stream',
    '/pubsub': 'fas fa-broadcast-tower',
    '/replication': 'fas fa-network-wired',
    '/logs': 'fas fa-file-alt',
    '/config': 'fas fa-cog',
  };
  return icons[route.path] || 'fas fa-cube';
});

function isActive(path: string): boolean {
  return route.path === path;
}

function toggleDropdown(): void {
  isDropdownOpen.value = !isDropdownOpen.value;
}

async function selectServer(id: string): Promise<void> {
  serversStore.setActiveServer(id);
  await serversStore.connectServer(id);
  isDropdownOpen.value = false;
}

function openServerModal(): void {
  editingServerId.value = undefined;
  showServerModal.value = true;
  isDropdownOpen.value = false;
}

function handleServerSaved(): void {
  showServerModal.value = false;
  editingServerId.value = undefined;
}

function minimizeWindow(): void {
  ipcBridge.minimizeWindow();
}

function maximizeWindow(): void {
  ipcBridge.toggleMaximize();
}

function closeWindow(): void {
  ipcBridge.closeWindow();
}

function toggleTheme(): void {
  themeStore.toggleTheme();
}

function handleClickOutside(event: MouseEvent): void {
  const target = event.target as HTMLElement;
  if (!target.closest('.connection-dropdown')) {
    isDropdownOpen.value = false;
  }
}

// Keyboard shortcuts
useKeyboardShortcuts({
  'ctrl+t': () => themeStore.toggleTheme(),
  'ctrl+k': () => openServerModal(),
  'ctrl+,': () => route.push('/config'),
});

onMounted(() => {
  document.addEventListener('click', handleClickOutside);
  
  // Initialize theme
  themeStore.loadTheme();
  
  // Auto-connect to active server if exists (after a short delay to ensure store is initialized)
  setTimeout(() => {
    if (activeServerId.value) {
      selectServer(activeServerId.value);
    }
  }, 100);
});

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside);
});
</script>

<style scoped>
.drag-region {
  -webkit-app-region: drag;
}
.no-drag {
  -webkit-app-region: no-drag;
}
.z-dropdown {
  z-index: 50;
}
</style>
