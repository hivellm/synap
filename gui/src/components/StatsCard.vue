<template>
  <div class="bg-bg-card border border-border rounded-lg p-4 transition-all hover:border-border-light">
    <div class="flex items-center justify-between mb-3">
      <span class="text-text-secondary text-sm">{{ title }}</span>
      <div :class="[
        'w-8 h-8 rounded-lg flex items-center justify-center',
        iconBgClass
      ]">
        <i :class="[icon, iconTextClass]"></i>
      </div>
    </div>
    <div class="text-2xl font-bold text-text-primary">{{ value }}</div>
    <div v-if="subtitle" class="text-xs text-text-muted mt-1">{{ subtitle }}</div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';

interface Props {
  title: string;
  value: string | number;
  icon: string;
  subtitle?: string;
  variant?: 'primary' | 'secondary' | 'success' | 'warning' | 'danger' | 'info';
}

const props = withDefaults(defineProps<Props>(), {
  variant: 'info'
});

const iconBgClass = computed(() => {
  const classes: Record<string, string> = {
    primary: 'bg-info/20',
    info: 'bg-info/20',
    secondary: 'bg-bg-tertiary',
    success: 'bg-success/20',
    warning: 'bg-warning/20',
    danger: 'bg-error/20',
  };
  return classes[props.variant] || 'bg-info/20';
});

const iconTextClass = computed(() => {
  const classes: Record<string, string> = {
    primary: 'text-info',
    info: 'text-info',
    secondary: 'text-text-secondary',
    success: 'text-success',
    warning: 'text-warning',
    danger: 'text-error',
  };
  return classes[props.variant] || 'text-info';
});
</script>
