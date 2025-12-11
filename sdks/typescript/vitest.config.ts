import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    // Separate test types
    include: [
      '**/*.{test,spec}.ts',  // Unit tests (mock)
      process.env.RUN_S2S ? '**/*.s2s.test.ts' : undefined, // Server-to-server tests (optional)
    ].filter(Boolean) as string[],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      exclude: [
        'node_modules/',
        'dist/',
        '**/*.test.ts',
        '**/*.s2s.test.ts',
        '**/__tests__/**',
        '**/__mocks__/**',
      ],
    },
  },
});

