import { randomUUID } from 'node:crypto';

import { defineConfig, devices } from '@playwright/test';

process.env.MARS_TEST_RUN_ID ??= randomUUID();

// Playwright config for the `e2e/tools/` suite. Two responsibilities:
//
//  1. `og.spec.ts`        - local-only, captures 1200x630 PNGs from
//                           `dev/tools/og-template.html` (no app stack required).
//  2. `<slug>.spec.ts`,
//     `registry.spec.ts`  - hit the deployed `tools.secutils.dev` (env-overridable
//                           via `BASE_URL`) for SEO + functional + skill assertions.
//
// Tools tests are read-only against an internet host so we keep workers low and
// retries on (the public host can hiccup briefly between calls).
export default defineConfig({
  testDir: './tools',
  fullyParallel: false,
  retries: process.env.CI ? 2 : 0,
  workers: 1,
  reporter: 'list',
  use: {
    baseURL: process.env.BASE_URL ?? 'https://tools.secutils.dev',
    screenshot: 'only-on-failure',
    video: 'off',
    trace: 'on-first-retry',
    contextOptions: {
      reducedMotion: 'reduce',
    },
  },
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        viewport: { width: 1280, height: 800 },
      },
    },
    {
      name: 'firefox',
      testMatch:
        '**/{mars,md,mars-export,mars-import,mars-models,mars-tables,mars-temporary,mars-sync,mars-sync-ux,mars-sync-followup,mars-sharing,mars-review-markers,mars-cleanup,pdf}.spec.ts',
      grep: /source selection|selection toolbar|admonition selection|app identity|Mermaid navigation|preview links|block controls|list dragging|reading width|native document links|export workspace|source import|model parity|MARS handoff|table editing|temporary sessions|sync protocol|sync user experience|review markers/,
      use: {
        ...devices['Desktop Firefox'],
        viewport: { width: 1280, height: 800 },
      },
    },
    {
      name: 'webkit',
      testMatch:
        '**/{mars,mars-export,mars-import,mars-models,mars-tables,mars-temporary,mars-sync,mars-sync-ux,mars-sync-followup,mars-sharing,mars-review-markers,mars-cleanup,pdf}.spec.ts',
      grep: /selection toolbar|native document links|export workspace|source import|model parity|MARS handoff|table editing|temporary sessions|sync protocol|sync user experience|review markers/,
      use: {
        ...devices['Desktop Safari'],
        viewport: { width: 1280, height: 800 },
      },
    },
  ],
});
