import { defineConfig, globalIgnores } from 'eslint/config';
import nextVitals from 'eslint-config-next/core-web-vitals';
import nextTs from 'eslint-config-next/typescript';

export default defineConfig([
  ...nextVitals,
  ...nextTs,
  {
    rules: {
      // A module is used only through its barrel (frontend.md §1).
      'no-restricted-imports': [
        'error',
        {
          patterns: [
            {
              group: ['@/modules/*/*'],
              message: 'Import a module through its index.ts (e.g. @/modules/identity).',
            },
          ],
        },
      ],
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/no-non-null-assertion': 'error',
    },
  },
  {
    // Inside a module, its own files import each other directly.
    files: ['src/modules/**'],
    rules: { 'no-restricted-imports': 'off' },
  },
  {
    // shadcn components, taken from the registry as published (frontend.md §5): the React
    // Compiler rules flag patterns upstream chose on purpose (a random skeleton width, the
    // mobile breakpoint read in an effect). Our own code keeps every rule.
    files: ['src/components/ui/**', 'src/components/shared/hooks/use-mobile.ts'],
    rules: { 'react-hooks/purity': 'off', 'react-hooks/set-state-in-effect': 'off' },
  },
  globalIgnores(['.next/**', 'out/**', 'next-env.d.ts', 'src/components/shared/api/schema.d.ts']),
]);
