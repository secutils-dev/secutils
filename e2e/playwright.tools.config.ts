import { defineConfig, devices } from '@playwright/test';

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
  ],
});
