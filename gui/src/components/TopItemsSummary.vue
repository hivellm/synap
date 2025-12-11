<template>
  <div class="bg-bg-card border border-border rounded-lg p-4">
    <div class="flex items-center gap-2 mb-3">
      <i v-if="icon" :class="[icon, 'text-text-muted text-xs']"></i>
      <h3 class="text-sm font-semibold text-text-primary">{{ title }}</h3>
    </div>
    
    <div v-if="items.length === 0" class="text-center py-4 text-text-muted text-xs">
      No data available
    </div>
    
    <div v-else class="space-y-1">
      <div
        v-for="(item, index) in items"
        :key="index"
        class="flex items-center justify-between py-2 px-2 rounded hover:bg-bg-hover transition-colors"
      >
        <div class="flex items-center gap-2 flex-1 min-w-0">
          <span class="text-xs text-text-muted w-4">{{ index + 1 }}</span>
          <span class="text-xs text-text-primary truncate font-mono">{{ item.name }}</span>
        </div>
        <span class="text-xs text-text-secondary">
          {{ formatValue(item.value) }}
        </span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
interface Item {
  name: string;
  value: number | string;
}

interface Props {
  title: string;
  icon?: string;
  items: Item[];
  formatValue?: (value: number | string) => string;
}

const props = withDefaults(defineProps<Props>(), {
  formatValue: (value: number | string) => String(value),
});
</script>
