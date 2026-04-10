import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { ipcBridge } from '@/services/ipc';

export type NotificationType = 'info' | 'success' | 'warning' | 'error';

export interface Notification {
  id: string;
  type: NotificationType;
  title: string;
  message: string;
  timestamp: number;
  duration?: number; // Auto-dismiss after milliseconds (0 = no auto-dismiss)
  actions?: Array<{
    label: string;
    action: () => void;
  }>;
}

export const useNotificationsStore = defineStore('notifications', () => {
  const notifications = ref<Notification[]>([]);
  const maxNotifications = ref<number>(50);

  const unreadCount = computed(() => {
    return notifications.value.length;
  });

  const hasUnread = computed(() => {
    return unreadCount.value > 0;
  });

  function addNotification(notification: Omit<Notification, 'id' | 'timestamp'>): string {
    const id = `notif-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
    const newNotification: Notification = {
      ...notification,
      id,
      timestamp: Date.now(),
      duration: notification.duration ?? 5000, // Default 5 seconds
    };

    notifications.value.unshift(newNotification);

    // Trim if exceeding max
    if (notifications.value.length > maxNotifications.value) {
      notifications.value = notifications.value.slice(0, maxNotifications.value);
    }

    // Show system notification if available
    if (window.electron) {
      try {
        ipcBridge.showNotification(newNotification.title, newNotification.message);
      } catch (error) {
        console.warn('Failed to show system notification:', error);
      }
    }

    // Auto-dismiss if duration is set
    if (newNotification.duration && newNotification.duration > 0) {
      setTimeout(() => {
        removeNotification(id);
      }, newNotification.duration);
    }

    return id;
  }

  function removeNotification(id: string): void {
    const index = notifications.value.findIndex((n) => n.id === id);
    if (index !== -1) {
      notifications.value.splice(index, 1);
    }
  }

  function clearNotifications(): void {
    notifications.value = [];
  }

  function clearNotificationsByType(type: NotificationType): void {
    notifications.value = notifications.value.filter((n) => n.type !== type);
  }

  // Convenience methods
  function info(title: string, message: string, options?: Partial<Notification>): string {
    return addNotification({ type: 'info', title, message, ...options });
  }

  function success(title: string, message: string, options?: Partial<Notification>): string {
    return addNotification({ type: 'success', title, message, ...options });
  }

  function warning(title: string, message: string, options?: Partial<Notification>): string {
    return addNotification({ type: 'warning', title, message, ...options });
  }

  function error(title: string, message: string, options?: Partial<Notification>): string {
    return addNotification({ type: 'error', title, message, ...options });
  }

  return {
    notifications,
    unreadCount,
    hasUnread,
    addNotification,
    removeNotification,
    clearNotifications,
    clearNotificationsByType,
    info,
    success,
    warning,
    error,
  };
});

