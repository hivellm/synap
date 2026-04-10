<template>
  <div class="border border-border rounded-lg overflow-hidden">
    <button
      @click="isExpanded = !isExpanded"
      class="w-full flex items-center justify-between p-3 bg-bg-tertiary hover:bg-bg-hover transition-colors"
    >
      <div class="flex items-center gap-2">
        <i :class="['fas', isExpanded ? 'fa-chevron-down' : 'fa-chevron-right', 'text-xs text-text-muted']"></i>
        <span class="font-medium text-text-primary capitalize">{{ title }}</span>
      </div>
      <span class="text-xs text-text-muted">
        {{ typeof data === 'object' ? Object.keys(data || {}).length + ' items' : String(data) }}
      </span>
    </button>
    
    <div v-if="isExpanded" class="p-3 bg-bg-primary border-t border-border">
      <div v-if="typeof data === 'object' && data !== null" class="space-y-2">
        <div
          v-for="(value, key) in data"
          :key="key"
          class="flex items-start justify-between py-1"
        >
          <span class="text-sm text-text-secondary">{{ key }}:</span>
          <span class="text-sm text-text-primary font-mono ml-4 text-right">
            {{ formatValue(value) }}
          </span>
        </div>
      </div>
      <div v-else class="text-sm text-text-primary font-mono">
        {{ data }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';

interface Props {
  title: string;
  data: unknown;
}

defineProps<Props>();

const isExpanded = ref(true);

function formatValue(value: unknown): string {
  if (typeof value === 'boolean') {
    return value ? 'true' : 'false';
  }
  if (typeof value === 'object' && value !== null) {
    return JSON.stringify(value);
  }
  return String(value);
}
</script>

