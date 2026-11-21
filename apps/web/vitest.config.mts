import react from '@vitejs/plugin-react';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: [
      { find: '@', replacement: fileURLToPath(new URL('./src', import.meta.url)) },
      // Monaco publishes its browser entry in `module`, with no Node main entry.
      {
        find: /^monaco-editor$/,
        replacement: fileURLToPath(
          new URL('./node_modules/monaco-editor/esm/vs/editor/editor.api.js', import.meta.url),
        ),
      },
    ],
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./vitest.setup.ts', './src/test/navigation-mock.tsx'],
    include: ['src/**/*.test.{ts,tsx}'],
    coverage: {
      provider: 'v8',
      // Domain services, contracts and transformations; UI rendering has component/browser checks.
      include: ['src/modules/**/*.ts', 'src/components/shared/api/**/*.ts'],
      exclude: ['**/*.test.ts', '**/index.ts', '**/schema.d.ts'],
      reporter: ['text', 'json-summary', 'lcov'],
      thresholds: { lines: 70 },
    },
  },
});
