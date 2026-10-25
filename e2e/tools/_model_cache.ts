// A persistent browser profile for smart-search tests: the browser keeps downloaded models in
// its own Cache Storage (and Transformers.js and ONNX Runtime in its HTTP cache), so each
// model downloads once per origin instead of once per test. Delete `e2e/.cache/profile/` to
// refresh it. (Intercepting the downloads with `route.fulfill` instead fails: bodies of
// 100+ MB crash the browser on the way through the DevTools protocol.)
import { resolve } from 'node:path';

import { type BrowserContext, chromium, type Page } from '@playwright/test';

const PROFILE = resolve(__dirname, '../.cache/profile');

export function launchWithModelCache(args: string[]): Promise<BrowserContext> {
  return chromium.launchPersistentContext(PROFILE, { headless: true, args, viewport: { width: 1280, height: 800 } });
}

// Opens MARS with no documents or settings, keeping the cached models.
export async function openFresh(page: Page, baseURL: string): Promise<void> {
  await page.goto(`${baseURL}/llms.txt`);
  await page.evaluate(async () => {
    localStorage.clear();
    await new Promise((done) => {
      const req = indexedDB.deleteDatabase('mars');
      req.onsuccess = req.onerror = req.onblocked = done;
    });
  });
  await page.goto(`${baseURL}/`);
}
