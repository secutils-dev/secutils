import { existsSync, readFileSync, writeFileSync } from 'fs';
import { resolve } from 'path';

import type { APIRequestContext, Locator, Page } from '@playwright/test';
import { expect } from '@playwright/test';
import { PNG } from 'pngjs';

export const DOCS_IMG_DIR = resolve(__dirname, '../components/secutils-docs/static/img/docs/guides');

export const EMAIL = 'e2e@secutils.dev';
export const PASSWORD = 'e2e_secutils_pass';

// 10-year operator JWT for @secutils, generated with:
// cargo run -p secutils-jwt-tools -- generate --secret <JWT_SECRET> --sub @secutils --exp 10years
export const OPERATOR_TOKEN =
  'eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJleHAiOjIwODcxMDY2MDQsInN1YiI6IkBzZWN1dGlscyJ9.7UT-E9YkTqTiktTtZal6wbjsgB8PTjmdATxNaQPG9zs';

export interface UserCredentials {
  email: string;
  password: string;
}

const patchedScreenshotPages = new WeakSet<Page>();

function patchPageScreenshot(page: Page) {
  if (patchedScreenshotPages.has(page)) {
    return;
  }

  const originalScreenshot = page.screenshot.bind(page);
  const patchedPage = page as Page & { screenshot: Page['screenshot'] };
  patchedPage.screenshot = (async (...args: Parameters<Page['screenshot']>) => {
    await waitForStableUiBeforeScreenshot(page);

    const opts = args[0];
    const path = opts && 'path' in opts ? (opts.path as string | undefined) : undefined;
    let referenceBytes: Buffer | null = null;
    if (path && existsSync(path)) {
      referenceBytes = readFileSync(path);
    }

    const buffer = await originalScreenshot(...args);
    if (path && referenceBytes) {
      stabilizeScreenshot(path, referenceBytes);
    }
    return buffer;
  }) as Page['screenshot'];

  patchedScreenshotPages.add(page);
}

async function waitForStableUiBeforeScreenshot(page: Page) {
  await page.waitForLoadState('domcontentloaded').catch(() => {});
  await page.waitForLoadState('networkidle', { timeout: 5000 }).catch(() => {});

  await page
    .waitForFunction(() => !document.querySelector('.euiIcon[data-is-loading="true"]'), undefined, {
      timeout: 5000,
    })
    .catch(() => {});

  await page
    .waitForFunction(
      () => {
        if (!('fonts' in document)) return true;
        const fonts = document as Document & { fonts: FontFaceSet };
        return fonts.fonts.status === 'loaded';
      },
      undefined,
      { timeout: 5000 },
    )
    .catch(() => {});

  // Replace user-specific webhook UUIDs in visible DOM elements so
  // screenshots don't change when a new user is created per run.
  await page
    .evaluate(() => {
      const WEBHOOK_RE = /\/api\/webhooks\/u\/[^/]+\//g;
      const STABLE = '/api/webhooks/u/preview/';
      for (const a of Array.from(document.querySelectorAll<HTMLAnchorElement>('a[href*="/api/webhooks/u/"]'))) {
        a.href = a.href.replace(WEBHOOK_RE, STABLE);
        if (a.textContent?.includes('/api/webhooks/u/')) {
          a.textContent = a.href;
        }
      }
      for (const el of Array.from(
        document.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>('input[value*="/api/webhooks/u/"], textarea'),
      )) {
        if (el.value.includes('/api/webhooks/u/')) {
          el.value = el.value.replace(WEBHOOK_RE, STABLE);
        }
      }
      const containers = Array.from(
        document.querySelectorAll('.euiCodeBlock, [data-test-subj="euiDataGridExpansionPopover"], pre, code'),
      );
      for (const container of containers) {
        if (!container.textContent?.includes('/api/webhooks/u/')) continue;
        const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT);
        let node;
        while ((node = walker.nextNode())) {
          if (node.textContent?.includes('/api/webhooks/u/')) {
            node.textContent = node.textContent.replace(WEBHOOK_RE, STABLE);
          }
        }
      }
    })
    .catch(() => {});

  // Wait for three animation frames so layout/paint/composite fully settle.
  await page.evaluate(
    () =>
      new Promise<void>((resolve) => {
        requestAnimationFrame(() => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
      }),
  );
}

const MAX_CHANNEL_DIFF = 1;

/**
 * Compare the freshly captured screenshot at `filePath` against
 * `referenceBytes` (the previous file on disk).  If every RGBA channel
 * differs by at most {@link MAX_CHANNEL_DIFF} the image hasn't
 * meaningfully changed - restore the reference file so there is zero diff.
 */
function stabilizeScreenshot(filePath: string, referenceBytes: Buffer): void {
  try {
    const refPng = PNG.sync.read(referenceBytes);
    const newPng = PNG.sync.read(readFileSync(filePath));
    if (refPng.width !== newPng.width || refPng.height !== newPng.height) return;

    const ref = refPng.data;
    const cur = newPng.data;
    for (let i = 0; i < ref.length; i++) {
      if (Math.abs(ref[i] - cur[i]) > MAX_CHANNEL_DIFF) return;
    }

    writeFileSync(filePath, referenceBytes);
  } catch {
    // If either PNG can't be decoded, leave the new file as-is.
  }
}

function generateRandomEmail(): string {
  const id = Math.random().toString(36).slice(2, 10);
  return `e2e-${id}@secutils.dev`;
}

export async function ensureUserAndLogin(
  request: APIRequestContext,
  page: Page,
  credentials?: UserCredentials,
): Promise<UserCredentials> {
  const email = credentials?.email ?? generateRandomEmail();
  const password = credentials?.password ?? PASSWORD;

  await page.context().clearCookies();
  await request.post('/api/users/remove', {
    headers: { Authorization: `Bearer ${OPERATOR_TOKEN}` },
    data: { email },
  });

  await goto(page, '/');

  const createAccountButton = page.getByRole('button', { name: 'Create account' });
  await expect(createAccountButton).toBeVisible({ timeout: 15000 });
  await createAccountButton.click();
  await expect(page).toHaveURL(/signup/);

  const emailInput = page.getByPlaceholder('Email');
  const continueButton = page.getByRole('button', { name: 'Continue with password' });
  await expect(emailInput).toBeVisible({ timeout: 15000 });
  // Use pressSequentially so React's synthetic event system tracks each keystroke and
  // properly updates form state. fill() sets the DOM value directly but can leave React's
  // internal state stale. The poll checks the button-enabled state (not just the DOM value)
  // so it retries the whole fill cycle if React resets the field before settling.
  await expect
    .poll(
      async () => {
        await emailInput.fill('');
        await emailInput.pressSequentially(email);
        return continueButton.isEnabled();
      },
      { timeout: 15000 },
    )
    .toBeTruthy();

  await continueButton.click();

  const passwordInput = page.getByPlaceholder('Password', { exact: true });
  const repeatPasswordInput = page.getByPlaceholder('Repeat password');
  await expect(passwordInput).toBeVisible({ timeout: 15000 });
  await passwordInput.fill(password);
  await repeatPasswordInput.fill(password);
  await page.getByRole('button', { name: 'Sign up', exact: true }).click();

  await expect(page).toHaveURL(/\/ws/, { timeout: 30000 });
  await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });

  return { email, password };
}

const STABILITY_CSS = [
  '*, *::before, *::after {',
  '  animation-duration: 0s !important; animation-delay: 0s !important;',
  '  transition-duration: 0s !important; transition-delay: 0s !important;',
  '}',
  'body { -webkit-font-smoothing: antialiased; text-rendering: geometricPrecision; }',
  '.euiButtonIcon, .euiSwitch__body { will-change: transform; }',
  '.monaco-editor .decorationsOverviewRuler { display: none !important; }',
  '.monaco-editor .cursors-layer { display: none !important; }',
  '.monaco-editor .minimap { display: none !important; }',
  '.monaco-editor .scroll-decoration { display: none !important; }',
  '* { caret-color: transparent !important; }',
  '::-webkit-scrollbar { width: 0 !important; height: 0 !important; }',
].join('\n');

export async function goto(page: Page, url: string) {
  patchPageScreenshot(page);

  await page.goto(url);
  await page.addStyleTag({ content: STABILITY_CSS });
}

export async function highlightOn(locator: Locator) {
  await locator.evaluate((el) => {
    el.style.outline = '3px dashed red';
    el.style.outlineOffset = '3px';
    el.style.borderRadius = '5px';
  });
}

export async function dismissAllToasts(page: Page) {
  const toasts = page.getByRole('button', { name: 'Dismiss toast' });
  for (const toast of await toasts.all()) {
    await toast.click();
  }
}

/** Fixed timestamp (Feb 19 2025) used to pin entity timestamps in screenshots.
 *  It is >3 days old so `TimestampTableCell` renders the absolute date "February 19, 2025"
 *  instead of an unstable relative string like "a few seconds ago".
 */
export const FIXED_ENTITY_TIMESTAMP = 1740000000;

/**
 * Replace `createdAt` / `updatedAt` with {@link FIXED_ENTITY_TIMESTAMP} in a JSON
 * value (object or array of objects).  Mutates in place.
 */
export function pinEntityTimestamps(json: unknown): void {
  const items = Array.isArray(json) ? json : [json];
  for (const item of items) {
    if (item && typeof item === 'object') {
      if ('createdAt' in item) item.createdAt = FIXED_ENTITY_TIMESTAMP;
      if ('updatedAt' in item) item.updatedAt = FIXED_ENTITY_TIMESTAMP;

      // For scheduled trackers, inject fixed schedule timestamps so the
      // "Next run" and "Last ran" columns render stable absolute dates.
      if ('retrack' in item) {
        const retrack = (item as Record<string, unknown>).retrack;
        if (retrack && typeof retrack === 'object') {
          const rt = retrack as Record<string, unknown>;
          const config = rt.config as Record<string, unknown> | undefined;
          if (config?.job) {
            rt.scheduledAt = FIXED_ENTITY_TIMESTAMP;
            rt.lastRanAt = FIXED_ENTITY_TIMESTAMP;
          }
        }
      }
    }
  }
}

/**
 * Set up a route handler that pins `createdAt`/`updatedAt` in GET JSON responses
 * matching `urlPattern`.  Non-GET requests pass through unchanged.
 */
export async function fixEntityTimestamps(page: Page, urlPattern: string) {
  await page.route(urlPattern, async (route) => {
    if (route.request().method() !== 'GET') {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    if (!response.ok()) {
      await route.fulfill({ response });
      return;
    }
    const json = await response.json();
    pinEntityTimestamps(json);
    await route.fulfill({ response, json });
  });
}

/**
 * Intercept responder history API responses and replace dynamic `createdAt` and
 * `clientAddress` fields with fixed values so screenshots are stable.
 */
export async function fixResponderRequestFields(page: Page) {
  await page.route('**/api/utils/webhooks/responders/*/history', async (route) => {
    const response = await route.fetch();
    if (!response.ok()) {
      await route.fulfill({ response });
      return;
    }
    const json = await response.json();
    for (const req of json) {
      req.createdAt = FIXED_ENTITY_TIMESTAMP;
      req.clientAddress = '172.18.0.1:12345';
    }
    await route.fulfill({ response, json });
  });
}

/**
 * Intercept certificate template API responses and pin the `notValidBefore` /
 * `notValidAfter` timestamps to fixed dates while preserving their distance.
 */
export async function fixCertificateTemplateValidityDates(page: Page) {
  const FIXED_NOT_VALID_BEFORE = 1735689600; // Jan 1, 2025 00:00:00 UTC
  await page.route('**/api/utils/certificates/templates**', async (route) => {
    const response = await route.fetch();
    if (!response.ok()) {
      await route.fulfill({ response });
      return;
    }
    const json = await response.json();
    const isArray = Array.isArray(json);
    const templates = isArray ? json : [json];
    for (const tpl of templates) {
      if (tpl.attributes?.notValidBefore != null && tpl.attributes?.notValidAfter != null) {
        const diff = tpl.attributes.notValidAfter - tpl.attributes.notValidBefore;
        tpl.attributes.notValidBefore = FIXED_NOT_VALID_BEFORE;
        tpl.attributes.notValidAfter = FIXED_NOT_VALID_BEFORE + diff;
      }
    }
    pinEntityTimestamps(isArray ? templates : templates[0]);
    await route.fulfill({ response, json: isArray ? templates : templates[0] });
  });
}

/**
 * Intercept page tracker revision history responses and stabilize dynamic parts
 * (URLs, sizes, timestamps) so screenshots remain consistent across runs.
 * If the server fails to execute the tracker (e.g. no browser available in CI),
 * the optional `fallback` array is returned instead.
 */
export async function fixTrackerResourceRevisions(page: Page, fallback?: object[]) {
  await page.route('**/api/utils/web_scraping/page/*/history', async (route) => {
    const response = await route.fetch();
    if (!response.ok()) {
      if (fallback) {
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(fallback) });
      } else {
        await route.fulfill({ response });
      }
      return;
    }
    const json = await response.json();
    if (!Array.isArray(json)) {
      if (fallback) {
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(fallback) });
      } else {
        await route.fulfill({ response, json });
      }
      return;
    }
    for (const rev of json) {
      rev.createdAt = FIXED_ENTITY_TIMESTAMP;
      const original = rev.data?.original;
      if (!original?.rows) {
        continue;
      }

      for (const row of original.rows) {
        row.source = stabilizeResourceUrl(row.source);
        row.size = String(stableResourceSize(row.source));
      }

      for (const type of ['scripts', 'styles']) {
        for (const resource of original.source?.[type] ?? []) {
          resource.url = stabilizeResourceUrl(resource.url);
          if (resource.content) {
            resource.content.size = stableResourceSize(resource.url);
          }
        }
      }
    }
    await route.fulfill({ response, json });
  });
}

function stabilizeResourceUrl(url: string): string {
  if (!url) return url;
  return url.replace(/\?[^?]*$/, '').replace(/https:\/\/[^.]+\.webhooks\./, 'https://preview.webhooks.');
}

function stableResourceSize(url: string): number {
  let hash = 0;
  for (let i = 0; i < url.length; i++) {
    hash = ((hash << 5) - hash + url.charCodeAt(i)) | 0;
  }
  return (Math.abs(hash) % 9000) + 1000;
}

/**
 * Intercept tracker execution log responses and pin timestamps/durations to fixed values
 * so screenshots remain consistent across runs.
 */
export async function fixTrackerExecutionLogs(page: Page) {
  const FIXED_STARTED_AT = FIXED_ENTITY_TIMESTAMP;
  const FIXED_FINISHED_AT = FIXED_ENTITY_TIMESTAMP + 3;
  const FIXED_PHASE_DURATION = 500;

  await page.route('**/api/utils/web_scraping/*/*/logs', async (route) => {
    if (route.request().method() !== 'GET') {
      await route.fallback();
      return;
    }
    const response = await route.fetch();
    if (!response.ok()) {
      await route.fulfill({ response });
      return;
    }
    const json = await response.json();
    if (!Array.isArray(json)) {
      await route.fulfill({ response, json });
      return;
    }
    for (const log of json) {
      log.startedAt = FIXED_STARTED_AT;
      log.finishedAt = FIXED_FINISHED_AT;
      if (Array.isArray(log.phases)) {
        for (const phase of log.phases) {
          phase.durationMs = FIXED_PHASE_DURATION;
        }
      }
    }
    await route.fulfill({ response, json });
  });
}

/**
 * Intercept tracker health summary (logs_summary) responses and pin timestamps/durations
 * to fixed values so health dot screenshots remain consistent across runs.
 */
export async function fixTrackerHealthDots(page: Page) {
  const FIXED_STARTED_AT = FIXED_ENTITY_TIMESTAMP;
  const FIXED_FINISHED_AT = FIXED_ENTITY_TIMESTAMP + 2;

  await page.route('**/api/utils/web_scraping/*/logs_summary', async (route) => {
    const response = await route.fetch();
    if (!response.ok()) {
      await route.fulfill({ response });
      return;
    }
    const json = await response.json();
    if (typeof json !== 'object' || json === null) {
      await route.fulfill({ response, json });
      return;
    }
    for (const trackerId of Object.keys(json)) {
      if (!Array.isArray(json[trackerId])) continue;
      for (const log of json[trackerId]) {
        log.startedAt = FIXED_STARTED_AT;
        log.finishedAt = FIXED_FINISHED_AT;
      }
    }
    await route.fulfill({ response, json });
  });
}

export async function highlightOff(locator: Locator) {
  await locator.evaluate((el) => {
    el.style.outline = 'unset';
    el.style.outlineOffset = 'unset';
    el.style.borderRadius = 'unset';
  });
}
