import { defineConfig } from 'vitest/config';
import vue from '@vitejs/plugin-vue';
import { fileURLToPath } from 'node:url';

// Tests run against a real synap-server (see tests/support/server.ts), so
// they get generous timeouts. The Electron plugins of vite.config.ts are left
// out: the tests exercise the renderer code only.
export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  test: {
    include: ['tests/**/*.test.ts'],
    environment: 'node',
    testTimeout: 30_000,
    hookTimeout: 120_000,
  },
});
