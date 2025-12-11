import { computed } from 'vue';
import { useServersStore } from '@/stores/servers';
import { SynapApiClient } from '@/services/api';

export function useApi() {
  const serversStore = useServersStore();

  const apiClient = computed(() => serversStore.activeClient as SynapApiClient | null);
  const isConnected = computed(() => serversStore.activeServer?.connected || false);

  async function execute<T = any>(
    operation: (client: SynapApiClient) => Promise<T>
  ): Promise<T> {
    const client = apiClient.value;
    if (!client) {
      throw new Error('No active server connection');
    }

    return operation(client);
  }

  return {
    apiClient,
    isConnected,
    execute,
  };
}

