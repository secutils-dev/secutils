import { defineConfig } from '@playwright/test';

/**
 * Config for standalone utility tests that do NOT require the full application
 * stack (no baseURL, no Docker). These tests validate tooling and transformers
 * against the currently installed Playwright version.
 *
 * Run:  cd e2e && npx playwright test --config=playwright.standalone.config.ts
 */
export default defineConfig({
  testDir: './standalone',
  fullyParallel: true,
  retries: 0,
  workers: 1,
  reporter: 'list',
  use: {
    trace: 'off',
    screenshot: 'off',
    video: 'off',
  },
});
