import { readFileSync } from 'node:fs';

import type { BrowserContext, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

import { getTool } from './_helpers';
import { launchWithModelCache, openFresh } from './_model_cache';

// Smart search with real models, in a persistent browser profile (e2e/.cache/profile/), so
// only the first run downloads them. Local-only: run against `npm run serve` in
// dev/tools/js/mars, or the live host:
//   MARS_BASE_URL=http://localhost:7373 make e2e-tools-test ARGS=mars-smart.spec.ts
// MARS_SMART_LIBRARY=<export.zip> times a real library instead of the fixture, and logs the
// matches for MARS_SMART_QUERIES (separated by `|`) without asserting them.
// MARS_SMART_DEVICE=cpu hides the GPU, to compare devices. Search quality is measured by
// dev/tools/js/mars/eval/, not here.
const tool = getTool('mars');
const MARS_BASE_URL = process.env.MARS_BASE_URL ?? `https://${tool.host}`;
const LIBRARY = process.env.MARS_SMART_LIBRARY;
const LIBRARY_QUERIES = (process.env.MARS_SMART_QUERIES ?? 'dog|single sign-on|how are releases shipped').split('|');
const CPU_ONLY = process.env.MARS_SMART_DEVICE === 'cpu';
// Metal gives headless Chromium a real GPU on macOS. Leave out `--enable-unsafe-webgpu`: it
// exposes experimental features that ONNX Runtime then picks and fails on.
const ARGS = CPU_ONLY ? [] : ['--use-angle=metal'];

const FIXTURE = [
  [
    'caching.md',
    '# Caching\n\nThe API cache keeps responses for fifteen minutes; then the TTL expires and the next request refetches.\n',
  ],
  ['login.md', '# Login\n\nUsers sign in with SAML through the identity provider, and sessions last eight hours.\n'],
  [
    'releases.md',
    `# Releases\n\nWe ship the Docker image every Tuesday after the end-to-end tests pass.\n\n<!-- su-comments\n${JSON.stringify(
      [
        {
          id: 't1',
          quote: 'every Tuesday',
          comments: [
            {
              id: 'c1',
              author: 'Oleg',
              at: '2026-10-21T09:00:00.000Z',
              body: 'Rotate the API tokens before each one.',
            },
          ],
        },
      ],
    )}\n-->\n`,
  ],
  // A one-word note: it gets no vector, so it can't match unrelated short queries.
  ['pets.md', 'Dog\n'],
];

async function importLibrary(page: Page) {
  if (LIBRARY) {
    await page
      .locator('#importInput')
      .setInputFiles({ name: 'library.zip', mimeType: 'application/zip', buffer: readFileSync(LIBRARY) });
  } else {
    await page
      .locator('#importInput')
      .setInputFiles(FIXTURE.map(([name, text]) => ({ name, mimeType: 'text/markdown', buffer: Buffer.from(text) })));
  }
  await expect(page.locator('#toast')).toContainText('Imported');
}

// Times turning smart search on: loading (download or cache, then compile) and indexing.
async function turnOn(page: Page, model: string) {
  await page.getByRole('button', { name: 'Smart search' }).click();
  await page.getByRole('radio', { name: new RegExp(model) }).check();
  const t0 = Date.now();
  await page.locator('#marsModalFoot').getByRole('button', { name: 'Turn on' }).click();
  const status = page.locator('#sbSmartStatus');
  const button = page.getByRole('button', { name: 'Smart search' });
  // Loaded once the button names the device; done once the status badge hides after that.
  // A small library can index between two polls, so "Indexing" may never be seen.
  let loaded = 0;
  for (;;) {
    const text = (await status.isHidden()) ? '' : ((await status.textContent()) ?? '');
    expect(text).not.toMatch(/unavailable/i);
    if (!loaded && /(GPU|CPU)\)/.test((await button.getAttribute('title')) ?? '')) loaded = Date.now();
    if (!text && loaded) break;
    await page.waitForTimeout(100);
  }
  const done = Date.now();
  return { load: (loaded - t0) / 1000, index: (done - loaded) / 1000 };
}

async function meaningHits(page: Page, query: string) {
  await page.getByRole('searchbox', { name: 'Search documents' }).fill(query);
  await expect(page.locator('#tree .tree-note')).toHaveCount(0, { timeout: 30_000 });
  await page.waitForTimeout(500);
  return page
    .locator('#tree .tree-row')
    .evaluateAll((rows) =>
      rows.filter((r) => r.querySelector('.tree-icon.smart')).map((r) => r.querySelector('.tree-label')!.textContent!),
    );
}

test.describe(`${tool.name} smart search`, () => {
  let context: BrowserContext;
  let page: Page;
  test.beforeEach(async () => {
    context = await launchWithModelCache(ARGS);
    page = context.pages()[0] ?? (await context.newPage());
    await openFresh(page, MARS_BASE_URL);
  });
  test.afterEach(async () => context?.close());

  for (const model of ['Best speed', 'Best quality']) {
    test(`${model} model finds passages by meaning`, async () => {
      test.setTimeout(15 * 60_000);
      const gpu = await page.evaluate(async () => {
        const a = await (
          navigator as unknown as {
            gpu?: { requestAdapter(): Promise<{ info?: { isFallbackAdapter?: boolean } } | null> };
          }
        ).gpu?.requestAdapter();
        return !!a && !a.info?.isFallbackAdapter;
      });
      test.skip(model === 'Best quality' && !gpu, 'needs a hardware GPU (WebGPU)');
      await importLibrary(page);
      const time = await turnOn(page, model);
      const device = (await page.getByRole('button', { name: 'Smart search' }).getAttribute('title'))?.match(
        /(GPU|CPU)\)/,
      )?.[1];
      const isolated = await page.evaluate(() => crossOriginIsolated);
      const summary = `${model}: load ${time.load.toFixed(1)}s, index ${time.index.toFixed(1)}s, ${device}, crossOriginIsolated=${isolated}`;
      console.log(summary);
      test.info().annotations.push({ type: 'timing', description: summary });

      if (LIBRARY) {
        for (const q of LIBRARY_QUERIES)
          console.log(`  ${JSON.stringify(q)} -> ${(await meaningHits(page, q)).join(', ') || '(none)'}`);
        return;
      }
      expect(await meaningHits(page, 'single sign-on')).toEqual(['login']);
      // Only the review comment is about credentials.
      expect(await meaningHits(page, 'rotate passwords')).toEqual(['releases']);
      expect(await meaningHits(page, 'document')).toEqual([]);

      // The picker marks the model as downloaded.
      await page.getByRole('button', { name: 'Smart search' }).click();
      await page.getByRole('menuitem', { name: /Change model/ }).click();
      await expect(page.locator('.model-option', { hasText: model }).locator('.model-badge')).toHaveText('Downloaded');
      await page.locator('#marsModalFoot').getByRole('button', { name: 'Cancel' }).click();

      // After a reload, the first query loads the model, and the status badge hides once it is ready.
      // Waits out the startup index pass, which would hide the badge on its own.
      await page.reload();
      await page.waitForTimeout(3500);
      expect(await meaningHits(page, 'single sign-on')).toEqual(['login']);
      await expect(page.locator('#sbSmartStatus')).toBeHidden();
    });
  }

  test('paused indexing leaves new documents out until resumed', async () => {
    test.skip(!!LIBRARY, 'uses the fixture');
    test.setTimeout(15 * 60_000);
    await importLibrary(page);
    await turnOn(page, 'Best speed');
    const button = page.getByRole('button', { name: 'Smart search' });
    const status = page.locator('#sbSmartStatus');
    const treeTop = async () => (await page.locator('#tree').boundingBox())!.y;
    const top = await treeTop();
    await button.click();
    await page.getByRole('menuitem', { name: /Pause indexing/ }).click();
    await expect(status).toHaveText('Paused');
    await expect(status).toHaveAttribute('data-state', 'paused');
    await expect(page.locator('#sbSmartLive')).toHaveText('Smart-search indexing is paused');
    // The badge sits beside the Documents title, so the tree below doesn't move.
    expect(await treeTop()).toBe(top);

    await page.locator('#importInput').setInputFiles({
      name: 'billing.md',
      mimeType: 'text/markdown',
      buffer: Buffer.from('# Billing\n\nInvoices go out on the first of each month, with thirty days to pay.\n'),
    });
    await expect(page.locator('#toast')).toContainText('Imported 1 document');
    // Past the 2-second indexing delay.
    await page.waitForTimeout(3000);
    await expect(status).toHaveText('Paused');
    await page.reload();
    await expect(status).toHaveText('Paused');
    expect(await meaningHits(page, 'when invoices are due')).not.toContain('billing');
    await expect(status).toHaveText('Paused');

    // The badge opens the smart-search menu.
    await status.click();
    await page.getByRole('menuitem', { name: /Resume indexing/ }).click();
    await expect(status).toBeHidden({ timeout: 60_000 });
    await expect(page.locator('#sbSmartLive')).toHaveText('Smart-search index is up to date');
    // The same query again: matches made before the document was indexed must not be reused.
    await expect.poll(() => meaningHits(page, 'when invoices are due')).toContain('billing');
  });
});
