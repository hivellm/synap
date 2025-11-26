<template>
  <div class="fixed top-12 right-4 z-50 flex flex-col gap-2 max-w-md w-full pointer-events-none">
    <TransitionGroup name="notification" tag="div">
      <div
        v-for="notification in notifications"
        :key="notification.id"
        class="bg-bg-elevated border border-border rounded-lg p-4 shadow-lg pointer-events-auto flex items-start gap-3"
        :class="notificationTypeClass(notification.type)"
      >
        <div class="flex-shrink-0 mt-0.5">
          <i :class="notificationIcon(notification.type)" class="text-lg"></i>
        </div>
        <div class="flex-1 min-w-0">
          <h4 class="text-sm font-semibold text-text-primary mb-1">{{ notification.title }}</h4>
          <p class="text-xs text-text-secondary">{{ notification.message }}</p>
          <div v-if="notification.actions && notification.actions.length > 0" class="mt-2 flex gap-2">
            <button
              v-for="(action, idx) in notification.actions"
              :key="idx"
              @click="action.action"
              class="text-xs px-2 py-1 rounded bg-bg-tertiary hover:bg-bg-hover text-text-primary transition-colors"
            >
              {{ action.label }}
            </button>
          </div>
        </div>
        <button
          @click="removeNotification(notification.id)"
          class="flex-shrink-0 text-text-muted hover:text-text-primary transition-colors"
        >
          <i class="fas fa-times text-xs"></i>
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useNotificationsStore, type NotificationType } from '@/stores/notifications';

const notificationsStore = useNotificationsStore();

const notifications = computed(() => notificationsStore.notifications.slice(0, 5)); // Show max 5

function notificationTypeClass(type: NotificationType): string {
  const classes: Record<NotificationType, string> = {
    info: 'border-info/30 bg-info/10',
    success: 'border-success/30 bg-success/10',
    warning: 'border-warning/30 bg-warning/10',
    error: 'border-error/30 bg-error/10',
  };
  return classes[type] || '';
}

function notificationIcon(type: NotificationType): string {
  const icons: Record<NotificationType, string> = {
    info: 'fas fa-info-circle text-info',
    success: 'fas fa-check-circle text-success',
    warning: 'fas fa-exclamation-triangle text-warning',
    error: 'fas fa-times-circle text-error',
  };
  return icons[type] || 'fas fa-info-circle';
}

function removeNotification(id: string): void {
  notificationsStore.removeNotification(id);
}
</script>

<style scoped>
.notification-enter-active,
.notification-leave-active {
  transition: all 0.3s ease;
}

.notification-enter-from {
  opacity: 0;
  transform: translateX(100%);
}

.notification-leave-to {
  opacity: 0;
  transform: translateX(100%);
}

.notification-move {
  transition: transform 0.3s ease;
}
</style>

