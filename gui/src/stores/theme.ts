import { defineStore } from 'pinia';
import { ref, watch } from 'vue';

export type Theme = 'dark' | 'light';

export const useThemeStore = defineStore('theme', () => {
  const theme = ref<Theme>('dark');

  // Load theme from localStorage
  function loadTheme(): void {
    const saved = localStorage.getItem('synap-theme') as Theme | null;
    if (saved && (saved === 'dark' || saved === 'light')) {
      theme.value = saved;
    } else {
      // Default to dark
      theme.value = 'dark';
    }
    applyTheme(theme.value);
  }

  // Apply theme to document
  function applyTheme(newTheme: Theme): void {
    if (newTheme === 'dark') {
      document.documentElement.classList.add('dark');
      document.documentElement.classList.remove('light');
    } else {
      document.documentElement.classList.add('light');
      document.documentElement.classList.remove('dark');
    }
  }

  // Set theme
  function setTheme(newTheme: Theme): void {
    theme.value = newTheme;
    localStorage.setItem('synap-theme', newTheme);
    applyTheme(newTheme);
  }

  // Toggle theme
  function toggleTheme(): void {
    const newTheme = theme.value === 'dark' ? 'light' : 'dark';
    setTheme(newTheme);
  }

  return {
    theme,
    setTheme,
    toggleTheme,
    loadTheme,
  };
});

