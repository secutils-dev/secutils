import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, test } from '@playwright/test';

const origin = 'https://tools.secutils.dev';
const root = join(__dirname, '../..');
const token = Buffer.alloc(32, 7).toString('base64url');
const now = Math.floor(Date.now() / 1000);
const columns = [
  'at',
  'entries',
  'totalBytes',
  'maxEntries',
  'maxTotalBytes',
  'oldestCreatedAt',
  'nextExpiresAt',
  'settingsSize',
  'requestCount',
  'lastRequestedAt',
  'updatedAt',
];
const storage = {
  entries: 194,
  totalBytes: 123129,
  maxEntries: 100000,
  maxTotalBytes: 1073741824,
  oldestCreatedAt: now - 5 * 86400,
  nextExpiresAt: now + 2 * 86400,
};
const current = {
  at: now,
  storage,
  responder: {
    enabled: true,
    settingsSize: 268352,
    requestCount: 30,
    requestsToTrack: 30,
    lastRequestedAt: now - 3600,
    updatedAt: now - 86400,
  },
  intervalSec: 1800,
  retentionSec: 2592000,
};
const points = Array.from({ length: 24 }, (_, i) => [
  now - 43200 + i * 1800,
  100 + i * 4,
  60000 + i * 2500,
  100000,
  1073741824,
  storage.oldestCreatedAt,
  storage.nextExpiresAt,
  268352,
  30,
  now - 3600,
  now - 86400,
]);
let html: string;

test.beforeAll(() => {
  html = execFileSync(process.execPath, ['--input-type=module'], {
    cwd: root,
    encoding: 'utf8',
    input: `
      import { readFileSync } from 'node:fs';
      import { minify } from 'html-minifier-terser';
      import { MINIFY_OPTIONS } from './dev/tools/deploy.ts';
      import { inlineSharedAssets } from './dev/tools/shared-assets.ts';
      const source = readFileSync('dev/tools/mars-stats.html', 'utf8')
        .replaceAll('{{TOOLS_HOST}}', 'tools.secutils.dev').replaceAll('{{MARS_HOST}}', 'mars.secutils.dev');
      process.stdout.write(await minify(inlineSharedAssets(source), MINIFY_OPTIONS));
    `,
  });
});

test.beforeEach(async ({ page }) => {
  await page.route('**/*', async (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== origin || url.pathname !== '/mars-stats') return route.abort();
    const action = url.searchParams.get('action');
    if (!action) return route.fulfill({ contentType: 'text/html', body: html });
    if (route.request().headers().authorization !== `Bearer ${token}`)
      return route.fulfill({ status: 401, json: { error: 'Open your private dashboard link to view statistics.' } });
    return route.fulfill({
      json: action === 'stats' ? current : { columns, points, cursor: null, intervalSec: 1800, retentionSec: 2592000 },
    });
  });
});

test('private fragment unlocks, is scrubbed, and survives reload without leaking into requests', async ({ page }) => {
  const errors: string[] = [];
  const requests: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('request', (r) => requests.push(r.url()));
  await page.goto(`${origin}/mars-stats#key=${token}`);
  await expect(page.getByRole('heading', { name: 'Room to grow' })).toBeVisible();
  await expect(page.locator('#entryCount')).toHaveText('194');
  await expect(page.locator('#storageChart .series')).toHaveCount(1);
  expect(new URL(page.url()).hash).toBe('');
  expect(requests.every((url) => !url.includes(token))).toBe(true);
  expect(requests.some((url) => /\/(shared|js\/mars)\//.test(url))).toBe(false);
  expect(html).not.toContain(token);
  expect(html).not.toContain('@su:responder-script');
  await page.reload();
  await expect(page.locator('#entryCount')).toHaveText('194');
  await page.getByLabel('Storage chart metric').selectOption('averageBytes');
  await expect(page.locator('#storageChartTitle')).toHaveText('Average entry size over time');
  await page.getByRole('button', { name: '24h', exact: true }).click();
  await expect(page.getByRole('button', { name: '24h', exact: true })).toHaveAttribute('aria-pressed', 'true');
  expect(errors).toEqual([]);
});

test('no token stays locked; an incorrect token produces a visible error', async ({ page }) => {
  await page.goto(`${origin}/mars-stats`);
  await expect(page.getByRole('heading', { name: 'Your private observatory' })).toBeVisible();
  await expect(page.locator('#dashboard')).toBeHidden();
  await page.getByLabel('Dashboard access token').fill(Buffer.alloc(32, 8).toString('base64url'));
  await page.getByRole('button', { name: 'Open observatory' }).click();
  await expect(page.getByRole('alert')).toContainText('Open your private dashboard link');
  await expect(page.locator('#dashboard')).toBeHidden();
});

test('owner-only chrome keeps navigation but omits the selected sidebar content and promotion', async ({ page }) => {
  await page.goto(`${origin}/mars-stats#key=${token}`);
  await expect(page.locator('#entryCount')).toHaveText('194');
  await expect(page.getByRole('navigation', { name: 'Statistics sections' })).toBeVisible();
  await expect(page.locator('.side-info, .side-footer, .su-more-tools')).toHaveCount(0);
  await expect(page.getByText('Separate by design', { exact: true })).toHaveCount(0);
  await expect(page.getByText('Small, steady snapshots', { exact: true })).toHaveCount(0);
  await expect(page.getByRole('link', { name: 'Open MARS', exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Privacy & data', exact: true })).toHaveCount(1);
  await expect(page.getByText(/Samples: 30 minutes/)).toBeVisible();
  await expect(page.locator('#coverage')).toContainText('0 collection gaps');
});

for (const theme of ['dark', 'light']) {
  for (const width of [1280, 390, 320]) {
    test(`MARS styling, chart data, and dialog work in ${theme} at ${width}px`, async ({ page }) => {
      const errors: string[] = [];
      page.on('pageerror', (e) => errors.push(e.message));
      await page.setViewportSize({ width, height: 900 });
      await page.addInitScript((value) => localStorage.setItem('su-tool-theme', value), theme);
      await page.goto(`${origin}/mars-stats#key=${token}`);
      await expect(page.locator('#entryCount')).toHaveText('194');
      await expect(page.locator('#coverage')).toContainText('24 samples');
      expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)).toBe(false);
      const brand = await page.evaluate(() =>
        getComputedStyle(document.documentElement).getPropertyValue('--brand').trim(),
      );
      expect(brand).toBe(theme === 'dark' ? '#ff6a4d' : '#c93b32');
      await page.getByText('View sample data', { exact: true }).click();
      await expect(page.locator('#sampleRows tr')).toHaveCount(24);
      const privacyButton = page.getByRole('button', { name: 'Privacy & data', exact: true }).last();
      await privacyButton.focus();
      await page.keyboard.press('Enter');
      await expect(page.getByRole('dialog', { name: 'Privacy & data' })).toBeVisible();
      await page.keyboard.press('Escape');
      await expect(page.getByRole('dialog')).toBeHidden();
      await expect(page.getByRole('button', { name: 'Privacy & data', exact: true }).last()).toBeFocused();
      expect(errors).toEqual([]);
    });
  }
}

test('CSV and JSON exports contain all loaded points and no token', async ({ page }) => {
  await page.goto(`${origin}/mars-stats#key=${token}`);
  await expect(page.getByRole('button', { name: 'Export CSV' })).toBeEnabled();
  for (const format of ['CSV', 'JSON']) {
    const downloaded = page.waitForEvent('download');
    await page.getByRole('button', { name: `Export ${format}` }).click();
    const file = await (await downloaded).path();
    const text = readFileSync(file!, 'utf8');
    expect(text).not.toContain(token);
    if (format === 'JSON') {
      expect(JSON.parse(text).points).toHaveLength(24);
      expect(JSON.parse(text).intervalSec).toBe(1800);
    } else expect(text.trim().split('\n')).toHaveLength(25);
  }
});

test('missing periods have dashed bridges and no fabricated observations', async ({ page }) => {
  await page.route('**/mars-stats?action=history*', (route) =>
    route.fulfill({ json: { columns, points: [points[0], points[1], points[20], points[21]], cursor: null } }),
  );
  await page.goto(`${origin}/mars-stats#key=${token}`);
  await expect(page.locator('#storageChart .series')).toHaveCount(2);
  await expect(page.locator('#storageChart .gap-bridge')).toHaveCount(1);
  await expect(page.locator('#entriesChart .gap-bridge')).toHaveCount(1);
  await expect(page.locator('#storageChart .gap-band')).toHaveCount(1);
  await expect(page.locator('#storageChart .gap-bridge')).toHaveCSS('stroke-dasharray', '5px, 5px');
  await expect(page.locator('#storageChart .gap-bridge title')).toContainText('No measurements');
  await expect(page.locator('#coverage')).toContainText('1 collection gaps');
  await page.locator('#storageChart .gap-band').hover();
  await expect(page.locator('#readout')).toContainText('No measurements between');
  await page.getByText('View sample data', { exact: true }).click();
  await expect(page.locator('#sampleRows tr')).toHaveCount(4);
  const exported = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Export JSON' }).click();
  const path = await (await exported).path();
  expect(JSON.parse(readFileSync(path!, 'utf8')).points).toHaveLength(4);
});

test('stale snapshots are visibly labeled but real history remains available', async ({ page }) => {
  await page.route('**/mars-stats?action=stats', (route) =>
    route.fulfill({
      json: {
        ...current,
        at: now - 3600,
        stale: true,
        failure: { code: 'upstream-timeout', stage: 'storage-api', retryable: true },
      },
    }),
  );
  await page.goto(`${origin}/mars-stats#key=${token}`);
  await expect(page.locator('#health')).toHaveText('Stale data');
  await expect(page.getByRole('alert')).toContainText('last successful snapshot');
  await expect(page.getByRole('alert')).toContainText('storage-api: upstream-timeout');
  await expect(page.locator('#entryCount')).toHaveText('194');
  await expect(page.getByRole('button', { name: 'Export JSON' })).toBeEnabled();
  await expect(page.locator('#coverage')).toContainText('24 samples');
});

test('an initial live-statistics failure does not hide available history', async ({ page }) => {
  await page.route('**/mars-stats?action=stats', (route) =>
    route.fulfill({ status: 502, json: { error: 'Temporary upstream failure.' } }),
  );
  await page.goto(`${origin}/mars-stats#key=${token}`);
  await expect(page.locator('#dashboard')).toBeVisible();
  await expect(page.locator('#locked')).toBeHidden();
  await expect(page.locator('#coverage')).toContainText('24 samples');
  await expect(page.getByRole('alert')).toContainText('Showing available measured history');
  await expect(page.locator('#health')).toHaveText('Update failed');
  await expect(page.getByRole('button', { name: 'Export JSON' })).toBeEnabled();
});

test('hidden tabs resume; live-off preserves manual refresh', async ({ page }) => {
  await page.goto(`${origin}/mars-stats#key=${token}`);
  await expect(page.getByRole('button', { name: 'Refresh', exact: true })).toBeEnabled();
  await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', { configurable: true, value: true });
    document.dispatchEvent(new Event('visibilitychange'));
  });
  await expect(page.locator('#health')).toHaveText('Updates paused');
  let statRequests = 0;
  await page.route('**/mars-stats?action=stats', (route) => {
    statRequests++;
    return route.fulfill({ json: current });
  });
  await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', { configurable: true, value: false });
    document.dispatchEvent(new Event('visibilitychange'));
  });
  await expect.poll(() => statRequests).toBe(1);
  await expect(page.getByRole('button', { name: 'Refresh', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: 'Live updates' }).click();
  await expect(page.getByRole('button', { name: 'Live updates' })).toHaveAttribute('aria-pressed', 'false');
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect.poll(() => statRequests).toBe(2);
});

test('hiding a tab aborts an in-flight statistics request', async ({ page }) => {
  await page.goto(`${origin}/mars-stats#key=${token}`);
  await expect(page.getByRole('button', { name: 'Refresh', exact: true })).toBeEnabled();
  await page.evaluate(() => {
    const nativeFetch = window.fetch;
    window.fetch = async (input, options) => {
      if (String(input).includes('action=stats')) {
        options!.signal!.addEventListener('abort', () => {
          document.documentElement.dataset.requestAborted = 'true';
        });
      }
      return nativeFetch(input, options);
    };
  });
  let release: () => void = () => {};
  let pending = false;
  await page.route('**/mars-stats?action=stats', async (route) => {
    pending = true;
    await new Promise<void>((resolve) => {
      release = resolve;
    });
    await route.fulfill({ json: current }).catch(() => {});
  });
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect.poll(() => pending).toBe(true);
  await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', { configurable: true, value: true });
    document.dispatchEvent(new Event('visibilitychange'));
  });
  await expect(page.locator('html')).toHaveAttribute('data-request-aborted', 'true');
  await expect(page.locator('#dashboard')).toHaveAttribute('aria-busy', 'false');
  release();
});

test('empty storage shows zeros without a false utilization arc', async ({ page }) => {
  await page.route('**/mars-stats?action=stats', (route) =>
    route.fulfill({
      json: {
        ...current,
        storage: { ...storage, entries: 0, totalBytes: 0, oldestCreatedAt: null, nextExpiresAt: null },
      },
    }),
  );
  await page.route('**/mars-stats?action=history*', (route) =>
    route.fulfill({ json: { columns, points: [], cursor: null } }),
  );
  await page.goto(`${origin}/mars-stats#key=${token}`);
  await expect(page.locator('#entryCount')).toHaveText('0');
  await expect(page.locator('#usedPercent')).toHaveText('0%');
  await expect(page.locator('#capacityArc')).toHaveCSS('visibility', 'hidden');
  await expect(page.getByRole('button', { name: 'Export CSV' })).toBeDisabled();
});
