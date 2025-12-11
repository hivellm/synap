<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-modal"
    @click.self="close"
  >
    <div class="bg-bg-secondary border border-border rounded-xl w-full max-w-md shadow-xl">
      <div class="flex justify-between items-center p-6 border-b border-border">
        <h2 class="text-xl font-semibold text-text-primary">
          {{ isEditing ? 'Edit Server' : 'Add Server' }}
        </h2>
        <button
          @click="close"
          class="p-2 text-text-secondary hover:text-text-primary hover:bg-bg-hover rounded transition-colors"
        >
          <i class="fas fa-times"></i>
        </button>
      </div>

      <form @submit.prevent="handleSubmit" class="p-6 space-y-4">
        <div>
          <label class="block text-sm font-medium text-text-secondary mb-1">
            Server Name
          </label>
          <input
            v-model="form.name"
            type="text"
            required
            class="w-full px-3 py-2 border border-border rounded-lg bg-bg-tertiary text-text-primary focus:ring-2 focus:ring-border-focus focus:border-transparent"
            placeholder="My Synap Server"
          />
        </div>

        <div>
          <label class="block text-sm font-medium text-text-secondary mb-1">
            URL
          </label>
          <input
            v-model="form.url"
            type="text"
            required
            class="w-full px-3 py-2 border border-border rounded-lg bg-bg-tertiary text-text-primary focus:ring-2 focus:ring-border-focus focus:border-transparent"
            placeholder="http://localhost"
          />
        </div>

        <div>
          <label class="block text-sm font-medium text-text-secondary mb-1">
            Port
          </label>
          <input
            v-model.number="form.port"
            type="number"
            class="w-full px-3 py-2 border border-border rounded-lg bg-bg-tertiary text-text-primary focus:ring-2 focus:ring-border-focus focus:border-transparent"
            placeholder="15500"
          />
        </div>

        <div>
          <label class="block text-sm font-medium text-text-secondary mb-1">
            API Key (Optional)
          </label>
          <input
            v-model="form.apiKey"
            type="password"
            class="w-full px-3 py-2 border border-border rounded-lg bg-bg-tertiary text-text-primary focus:ring-2 focus:ring-border-focus focus:border-transparent"
            placeholder="Enter API key"
          />
        </div>

        <div v-if="testResult" class="p-3 rounded-lg" :class="testResult.success ? 'bg-success/10 text-success' : 'bg-error/10 text-error'">
          {{ testResult.message }}
        </div>

        <div class="flex justify-end gap-3 pt-4 border-t border-border">
          <button
            type="button"
            @click="close"
            class="px-4 py-2 text-sm font-medium bg-transparent text-text-secondary border border-border rounded-lg hover:bg-bg-hover hover:text-text-primary transition-colors"
          >
            Cancel
          </button>
          <button
            type="button"
            @click="testConnection"
            :disabled="testing"
            class="px-4 py-2 text-sm font-medium bg-bg-tertiary text-text-primary border border-border rounded-lg hover:bg-bg-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <i :class="['fas', testing ? 'fa-spinner fa-spin' : 'fa-plug']"></i>
            {{ testing ? 'Testing...' : 'Test' }}
          </button>
          <button
            type="submit"
            :disabled="saving"
            class="px-4 py-2 text-sm font-medium bg-bg-tertiary text-text-primary border border-border rounded-lg hover:bg-bg-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <i :class="['fas', saving ? 'fa-spinner fa-spin' : 'fa-save']"></i>
            {{ saving ? 'Saving...' : isEditing ? 'Update' : 'Add' }}
          </button>
        </div>
      </form>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, watch, computed } from 'vue';
import { useServersStore } from '@/stores/servers';
import type { ServerConfig } from '@/services/api';

interface Props {
  isOpen: boolean;
  serverId?: string;
}

interface Emits {
  (e: 'close'): void;
  (e: 'saved'): void;
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

const serversStore = useServersStore();
const saving = ref(false);
const testing = ref(false);
const testResult = ref<{ success: boolean; message: string } | null>(null);

const isEditing = computed(() => !!props.serverId);

const form = reactive<ServerConfig>({
  name: '',
  url: 'http://localhost',
  port: 15500,
  apiKey: '',
  timeout: 30000,
});

watch(() => props.isOpen, (open) => {
  if (open && props.serverId) {
    const server = serversStore.getServer(props.serverId);
    if (server) {
      form.name = server.name;
      form.url = server.url;
      form.port = server.port;
      form.apiKey = server.apiKey || '';
      form.timeout = server.timeout;
    }
  } else if (open) {
    // Reset form for new server
    form.name = '';
    form.url = 'http://localhost';
    form.port = 15500;
    form.apiKey = '';
    form.timeout = 30000;
  }
  testResult.value = null;
});

async function testConnection() {
  testing.value = true;
  testResult.value = null;

  try {
    const success = await serversStore.testConnection(form);
    testResult.value = {
      success,
      message: success ? 'Connection successful!' : 'Connection failed. Please check your settings.',
    };
  } catch (error: any) {
    testResult.value = {
      success: false,
      message: error.message || 'Connection failed',
    };
  } finally {
    testing.value = false;
  }
}

async function handleSubmit() {
  saving.value = true;

  try {
    if (isEditing.value && props.serverId) {
      serversStore.updateServer(props.serverId, form);
    } else {
      const id = serversStore.addServer(form);
      serversStore.setActiveServer(id);
    }
    emit('saved');
    close();
  } catch (error) {
    console.error('Failed to save server:', error);
  } finally {
    saving.value = false;
  }
}

function close() {
  emit('close');
}
</script>

