import { readFileSync } from 'node:fs';
import type { StorybookConfig } from '@storybook/nextjs-vite';

const config: StorybookConfig = {
  stories: ['../src/stories/**/*.stories.tsx'],
  framework: { name: '@storybook/nextjs-vite', options: {} },
  core: { disableTelemetry: true },
  previewHead: (head) => {
    const icon = readFileSync(new URL('../src/app/icon.svg', import.meta.url), 'utf8');
    return `${head}<link rel="icon" href="data:image/svg+xml,${encodeURIComponent(icon)}">`;
  },
};

export default config;
