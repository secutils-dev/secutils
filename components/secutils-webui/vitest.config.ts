import { defineConfig } from 'vitest/config';

export default defineConfig({
  oxc: { jsx: { runtime: 'automatic', importSource: '@emotion/react' } },
  test: {
    include: ['src/**/*.test.ts'],
  },
});
