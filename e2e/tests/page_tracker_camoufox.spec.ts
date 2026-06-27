import type { APIRequestContext, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

// These tests exercise the *real* Camoufox extraction path end-to-end: a page
// tracker is created with `target.engine = { type: 'camoufox' }`, then "Update"
// is clicked in the UI which makes Retrack connect to the Camoufox (Firefox)
// browser over the WebSocket endpoint and run the Playwright extractor script.
// Nothing is mocked - this is exactly the path a `playwright-core` bump can break.
//
// To tell Camoufox (Gecko) apart from a silent Chromium fallback we probe a
// rendering-engine-specific CSS feature: `-moz-appearance` is only recognized by
// Gecko, so `CSS.supports('-moz-appearance', 'none')` is `true` on Camoufox and
// `false` on Chromium. Using a `data:` URL keeps the tests free of any external
// network dependency.

const GECKO_PROBE_EXTRACTOR = `export async function execute(page) {
  await page.goto('data:text/html,<title>Camoufox%20E2E</title><body>ok</body>');
  const gecko = await page.evaluate(() => CSS.supports('-moz-appearance', 'none'));
  return 'CAMOUFOX_PROBE gecko=' + gecko;
}`;

const TITLE_EXTRACTOR = `export async function execute(page) {
  await page.goto('data:text/html,<title>CamoufoxTitleMarker42</title><body>ok</body>');
  return '## ' + (await page.title());
}`;

async function createPageTracker(
  request: APIRequestContext,
  name: string,
  extractor: string,
  engine?: 'camoufox' | 'chromium',
) {
  const res = await request.post('/api/web_scraping/page_trackers', {
    data: {
      name,
      config: { revisions: 3 },
      target: { extractor, ...(engine ? { engine: { type: engine } } : {}) },
    },
  });
  expect(res.ok()).toBeTruthy();
}

async function fetchLatestRevision(page: Page, trackerName: string) {
  await page.goto('/ws/web_scraping__page');
  const trackerRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: trackerName }) });
  await expect(trackerRow).toBeVisible({ timeout: 15000 });

  await trackerRow.getByRole('button', { name: 'Show history' }).click();

  const updateButton = page.getByRole('button', { name: 'Update', exact: true });
  await expect(updateButton).toBeVisible({ timeout: 15000 });
  await updateButton.click();
}

test.describe.serial('Page Tracker - Camoufox engine', () => {
  test.beforeEach(async ({ request, page }) => {
    // Real browser extraction (especially first Camoufox launch) is slow.
    test.setTimeout(120_000);
    await ensureUserAndLogin(request, page);
  });

  test('runs the extractor on the Camoufox (Gecko) engine', async ({ page }) => {
    await createPageTracker(page.request, 'Camoufox Gecko Probe', GECKO_PROBE_EXTRACTOR, 'camoufox');
    await fetchLatestRevision(page, 'Camoufox Gecko Probe');

    // Camoufox is Firefox/Gecko based, so the -moz- feature query is supported.
    await expect(page.getByText('gecko=true')).toBeVisible({ timeout: 90000 });
  });

  test('extracts the page title via the Camoufox engine', async ({ page }) => {
    await createPageTracker(page.request, 'Camoufox Title Probe', TITLE_EXTRACTOR, 'camoufox');
    await fetchLatestRevision(page, 'Camoufox Title Probe');

    await expect(page.getByText('CamoufoxTitleMarker42', { exact: false })).toBeVisible({ timeout: 90000 });
  });

  test('Chromium engine does not report the Gecko-only feature (probe is meaningful)', async ({ page }) => {
    // Sanity check that the Gecko discriminator above is not a false positive:
    // the same probe on the default Chromium engine must report gecko=false.
    await createPageTracker(page.request, 'Chromium Gecko Probe', GECKO_PROBE_EXTRACTOR);
    await fetchLatestRevision(page, 'Chromium Gecko Probe');

    await expect(page.getByText('gecko=false')).toBeVisible({ timeout: 90000 });
  });
});
