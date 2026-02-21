import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './docs',
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: 'list',
  use: {
    baseURL: process.env.BASE_URL ?? 'http://localhost:7171',
    screenshot: 'only-on-failure',
    video: 'off',
    trace: 'off',
    viewport: { width: 1600, height: 900 },
    colorScheme: 'light',
    contextOptions: {
      reducedMotion: 'reduce',
    },
  },
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        viewport: { width: 1600, height: 900 },
      },
    },
  ],
});
