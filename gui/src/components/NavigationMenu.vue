<template>
  <nav class="bg-white dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700 w-64 min-h-screen">
    <div class="p-4">
      <h1 class="text-xl font-bold text-gray-900 dark:text-white mb-6">Synap Desktop</h1>
      
      <ul class="space-y-1">
        <li v-for="item in menuItems" :key="item.id">
          <router-link
            :to="item.path"
            class="flex items-center space-x-3 px-4 py-2 rounded-md text-sm font-medium transition-colors"
            :class="isActive(item.path) 
              ? 'bg-primary-100 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300' 
              : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700'"
          >
            <span class="text-xl">{{ getIcon(item.id) }}</span>
            <span>{{ item.label }}</span>
          </router-link>
        </li>
      </ul>
    </div>

    <div class="absolute bottom-0 left-0 right-0 p-4 border-t border-gray-200 dark:border-gray-700">
      <div class="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
        <span>v{{ version }}</span>
        <button
          @click="toggleTheme"
          class="p-2 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-md"
          title="Toggle theme"
        >
          <svg v-if="isDark" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
          </svg>
          <svg v-else class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
          </svg>
        </button>
      </div>
    </div>
  </nav>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useRoute } from 'vue-router';

interface MenuItem {
  id: string;
  label: string;
  path: string;
}

const route = useRoute();
const version = ref('0.1.0');
const isDark = ref(false);

const menuItems: MenuItem[] = [
  {
    id: 'dashboard',
    label: 'Dashboard',
    path: '/',
    icon: 'DashboardIcon',
  },
  {
    id: 'metrics',
    label: 'Metrics',
    path: '/metrics',
    icon: 'ChartIcon',
  },
  {
    id: 'kv-store',
    label: 'KV Store',
    path: '/kv-store',
    icon: 'DatabaseIcon',
  },
  {
    id: 'queues',
    label: 'Queues',
    path: '/queues',
    icon: 'QueueIcon',
  },
  {
    id: 'streams',
    label: 'Streams',
    path: '/streams',
    icon: 'StreamIcon',
  },
  {
    id: 'pubsub',
    label: 'Pub/Sub',
    path: '/pubsub',
    icon: 'PubSubIcon',
  },
  {
    id: 'logs',
    label: 'Logs',
    path: '/logs',
    icon: 'LogIcon',
  },
  {
    id: 'config',
    label: 'Configuration',
    path: '/config',
    icon: 'ConfigIcon',
  },
];

function isActive(path: string): boolean {
  return route.path === path;
}

function getIcon(id: string): string {
  const icons: Record<string, string> = {
    dashboard: '📊',
    metrics: '📈',
    'kv-store': '🗄️',
    queues: '📬',
    streams: '🌊',
    pubsub: '📡',
    logs: '📝',
    config: '⚙️',
  };
  return icons[id] || '•';
}

function toggleTheme() {
  isDark.value = !isDark.value;
  if (isDark.value) {
    document.documentElement.classList.add('dark');
  } else {
    document.documentElement.classList.remove('dark');
  }
}

onMounted(async () => {
  if (window.electron) {
    version.value = await window.electron.getVersion();
  }
  
  // Check initial theme
  isDark.value = document.documentElement.classList.contains('dark');
});
</script>

