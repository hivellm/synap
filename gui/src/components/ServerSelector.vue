<template>
  <div class="relative w-full">
    <button
      @click="showDropdown = !showDropdown"
      class="w-full flex items-center justify-between gap-3 p-3 rounded-lg bg-bg-tertiary hover:bg-bg-hover transition-colors cursor-pointer"
    >
      <div class="flex-1 min-w-0">
        <div class="flex items-center gap-2 whitespace-nowrap overflow-hidden text-ellipsis">
          <span class="text-sm font-medium text-text-primary">{{ activeServer?.name || 'No server selected' }}</span>
          <span class="text-text-muted">•</span>
          <span
            v-if="activeServer"
            class="w-2 h-2 rounded-full"
            :class="activeServer.status === 'online' ? 'bg-success' : activeServer.status === 'connecting' ? 'bg-warning' : 'bg-error'"
          ></span>
          <span class="text-xs text-text-secondary">{{ activeServer?.status || 'No servers' }}</span>
        </div>
      </div>
      <i class="fas fa-chevron-down text-xs text-text-muted transition-transform" :class="{ 'rotate-180': showDropdown }"></i>
    </button>

    <div
      v-if="showDropdown"
      class="absolute top-full left-0 right-0 mt-1 bg-bg-elevated border border-border rounded-lg shadow-lg z-dropdown"
      @click.stop
    >
      <div v-if="serverList.length === 0" class="p-4 text-center text-text-secondary">
        <i class="fas fa-exclamation-circle mb-2 block"></i>
        <span class="text-sm">No servers available</span>
      </div>
      <div v-else>
        <div
          v-for="server in serverList"
          :key="server.id"
          @click="selectServer(server.id)"
          :class="['flex items-center justify-between p-3 hover:bg-bg-hover cursor-pointer transition-colors', { 'bg-bg-hover': activeServerId === server.id }]"
        >
          <div class="flex-1 min-w-0">
            <div class="text-sm font-medium text-text-primary truncate">{{ server.name }}</div>
            <div class="flex items-center gap-2 text-xs text-text-secondary">
              <span
                class="w-1.5 h-1.5 rounded-full"
                :class="server.status === 'online' ? 'bg-success' : server.status === 'connecting' ? 'bg-warning' : 'bg-error'"
              ></span>
              {{ server.url }}{{ server.port ? `:${server.port}` : '' }}
            </div>
          </div>
          <div v-if="activeServerId === server.id" class="text-success">
            <i class="fas fa-check text-sm"></i>
          </div>
        </div>
      </div>
      
      <div class="border-t border-border p-3">
        <button
          @click="addServer"
          class="w-full flex items-center gap-2 px-3 py-2 text-sm text-text-secondary hover:text-text-primary hover:bg-bg-hover rounded transition-colors"
        >
          <i class="fas fa-cog"></i>
          Manage Servers
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useServersStore } from '@/stores/servers';

interface Emits {
  (e: 'add-server'): void;
  (e: 'edit-server', id: string): void;
}

const emit = defineEmits<Emits>();

const serversStore = useServersStore();
const showDropdown = ref(false);

const serverList = computed(() => serversStore.serverList);
const activeServerId = computed(() => serversStore.activeServerId);
const activeServer = computed(() => serversStore.activeServer);

async function selectServer(id: string) {
  serversStore.setActiveServer(id);
  await serversStore.connectServer(id);
  showDropdown.value = false;
}

function addServer() {
  emit('add-server');
  showDropdown.value = false;
}

function editServer(id: string) {
  emit('edit-server', id);
  showDropdown.value = false;
}

async function removeServer(id: string) {
  if (confirm('Are you sure you want to remove this server?')) {
    await serversStore.disconnectServer(id);
    serversStore.removeServer(id);
  }
}

function handleClickOutside(event: MouseEvent) {
  const target = event.target as HTMLElement;
  if (!target.closest('.relative')) {
    showDropdown.value = false;
  }
}

onMounted(() => {
  document.addEventListener('click', handleClickOutside);
});

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside);
});
</script>

