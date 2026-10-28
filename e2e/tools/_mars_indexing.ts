import type { BrowserContext, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

// Exercise the real index scheduler, storage, and UI without downloading a model.
// Only the semantic worker is replaced; document rendering keeps its normal workers.
type WorkerState = { embeds: string[][]; hold: boolean; release(): void };
type TestWindow = Window & { __marsWorker: WorkerState };

async function mockWorker(context: BrowserContext) {
  await context.addInitScript(() => {
    const waiting: (() => void)[] = [];
    const state: WorkerState = {
      embeds: [],
      hold: false,
      release: () => {
        state.hold = false;
        for (const reply of waiting.splice(0)) reply();
      },
    };
    (window as TestWindow).__marsWorker = state;
    window.Worker = new Proxy(window.Worker, {
      construct(Target, args) {
        if (args[1]?.name !== 'mars-smart-search') return Reflect.construct(Target, args);
        return {
          onmessage: null as ((e: MessageEvent) => void) | null,
          postMessage(msg: { id: number; type: string; texts?: string[] }) {
            if (msg.texts) state.embeds.push(msg.texts);
            const reply = () =>
              this.onmessage?.({
                data: {
                  id: msg.id,
                  ok: true,
                  device: 'wasm',
                  dims: 384,
                  data: new Float32Array((msg.texts?.length ?? 0) * 384).fill(1),
                },
              } as MessageEvent);
            if (state.hold) waiting.push(reply);
            else setTimeout(reply, 0);
          },
          terminate() {
            waiting.splice(0);
          },
        };
      },
    });
  });
}

async function importDoc(page: Page, name: string) {
  await page.locator('#importInput').setInputFiles({
    name: `${name}.md`,
    mimeType: 'text/markdown',
    buffer: Buffer.from(`# ${name}\n\nA document with enough words to index.\n`),
  });
  await expect(page.locator('#toast')).toContainText('Imported 1 document');
  await expect(page.locator('#tree .tree-label').getByText(name, { exact: true })).toBeVisible();
}

async function turnOn(page: Page) {
  await page.getByRole('button', { name: 'Smart search', exact: true }).click();
  await page.getByRole('radio', { name: /Best speed/ }).check();
  await page.locator('#marsModalFoot').getByRole('button', { name: 'Turn on', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Smart search', exact: true })).toHaveAttribute('title', /CPU\)/);
  await expect(page.locator('#sbSmartStatus')).toBeHidden();
}

async function toggle(page: Page, action: 'Pause' | 'Resume') {
  await page.getByRole('button', { name: 'Smart search', exact: true }).click();
  await page.getByRole('menuitem', { name: new RegExp(`^${action} indexing`) }).click();
}

const calls = (page: Page) => page.evaluate(() => (window as TestWindow).__marsWorker.embeds.length);
const hold = (page: Page) =>
  page.evaluate(() => {
    (window as TestWindow).__marsWorker.hold = true;
  });
const release = (page: Page) => page.evaluate(() => (window as TestWindow).__marsWorker.release());

async function indexedNames(page: Page) {
  return page.evaluate(async () => {
    const db = await new Promise<IDBDatabase>((resolve, reject) => {
      const req = indexedDB.open('mars');
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
    try {
      const tx = db.transaction(['nodes', 'docs'], 'readonly');
      const nodes = tx.objectStore('nodes').getAll();
      const keys = tx.objectStore('docs').getAllKeys();
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      return nodes.result
        .filter((n) => keys.result.includes(`vec:${n.id}`))
        .map((n) => n.name)
        .sort();
    } finally {
      db.close();
    }
  });
}

export function indexingTests(path: string) {
  test.describe('indexing pause', () => {
    test.beforeEach(async ({ context }) => mockWorker(context));

    test('survives reload and keeps search available until resumed', async ({ page }) => {
      await page.goto(path);
      await importDoc(page, 'Original');
      await turnOn(page);
      await toggle(page, 'Pause');
      await page.reload();
      await expect(page.locator('#sbSmartStatus')).toHaveText('Paused');
      await importDoc(page, 'Later');
      await page.waitForTimeout(3500); // Past both the startup and edit indexing delays.
      expect(await calls(page)).toBe(0);
      expect(await indexedNames(page)).toEqual(['Original']);

      await page.getByRole('searchbox', { name: 'Search documents' }).fill('zebra');
      await expect(page.locator('#tree .tree-icon.smart')).toHaveCount(1);
      await expect(page.locator('#sbSmartStatus')).toHaveText('Paused');
      expect(await calls(page)).toBe(1); // The query can still load and use the model.
      await toggle(page, 'Resume');
      await expect(page.locator('#sbSmartStatus')).toBeHidden();
      await expect.poll(() => indexedNames(page)).toEqual(['Later', 'Original']);
      await expect(page.locator('#tree .tree-icon.smart')).toHaveCount(2);
      await page.reload();
      await expect(page.locator('#sbSmartStatus')).toBeHidden();
      expect(await page.evaluate(() => localStorage.getItem('mars.indexingPaused'))).toBeNull();
    });

    test('pause and resume synchronize existing and newly opened tabs', async ({ page, context }) => {
      await page.goto(path);
      await importDoc(page, 'Original');
      await turnOn(page);
      const other = await context.newPage();
      await other.goto(path);
      await expect(other.locator('#sbSmartBtn')).toHaveAttribute('aria-pressed', 'true');
      await toggle(page, 'Pause');
      await expect(other.locator('#sbSmartStatus')).toHaveText('Paused');
      const newest = await context.newPage();
      await newest.goto(path);
      await expect(newest.locator('#sbSmartStatus')).toHaveText('Paused');
      // The saved pause renders before app boot finishes. Wait for an interactive tab
      // before importing, so its remote-document listener is registered too.
      await newest.locator('#sbSmartStatus').click();
      await expect(newest.getByRole('menuitem', { name: /^Resume indexing/ })).toBeVisible();
      await newest.keyboard.press('Escape');
      await importDoc(page, 'Later');
      for (const tab of [other, newest])
        await expect(tab.locator('#tree .tree-label').getByText('Later', { exact: true })).toBeVisible();
      await page.waitForTimeout(3500);
      expect(await indexedNames(page)).toEqual(['Original']);
      for (const tab of [other, newest]) expect(await calls(tab)).toBe(0);

      // Hold replies so each tab must visibly start its own catch-up pass on remote resume.
      for (const tab of [page, other, newest]) await hold(tab);
      await toggle(other, 'Resume');
      for (const tab of [page, other, newest])
        await expect(tab.locator('#sbSmartStatus')).toHaveText(/Preparing|Indexing/);
      await Promise.all([page, other, newest].map(release));
      for (const tab of [page, other, newest]) {
        await expect.poll(() => calls(tab)).toBeGreaterThan(0);
        await expect(tab.locator('#sbSmartStatus')).toBeHidden();
      }
      await expect.poll(() => indexedNames(page)).toEqual(['Later', 'Original']);
    });

    test('a remote pause stops an active pass after its current document', async ({ page, context }) => {
      await page.goto(path);
      await importDoc(page, 'Original');
      await turnOn(page);
      const other = await context.newPage();
      await other.goto(path);
      await expect(other.locator('#sbSmartBtn')).toHaveAttribute('aria-pressed', 'true');
      await hold(page);
      await importDoc(page, 'First');
      await importDoc(page, 'Second');
      await expect.poll(() => calls(page)).toBe(2); // Original, then the held First document.
      await toggle(other, 'Pause');
      await expect(page.locator('#sbSmartStatus')).toHaveText('Paused');
      await release(page);
      await page.waitForTimeout(2500);
      expect(await calls(page)).toBe(2);
      await toggle(other, 'Resume');
      await expect.poll(() => calls(page)).toBe(3);
    });

    test('rebuilding respects pause and turning smart search off clears it', async ({ page }) => {
      await page.goto(path);
      await importDoc(page, 'Original');
      await turnOn(page);
      await toggle(page, 'Pause');
      await page.getByRole('button', { name: 'Smart search', exact: true }).click();
      await page.getByRole('menuitem', { name: 'Rebuild the index', exact: true }).click();
      await expect(page.locator('#sbSmartStatus')).toHaveText('Paused');
      expect(await indexedNames(page)).toEqual([]);
      expect(await calls(page)).toBe(1);
      await toggle(page, 'Resume');
      await expect.poll(() => indexedNames(page)).toEqual(['Original']);
      await toggle(page, 'Pause');
      await page.getByRole('button', { name: 'Smart search', exact: true }).click();
      await page.getByRole('menuitem', { name: /^Turn off smart search/ }).click();
      await expect(page.locator('#sbSmartStatus')).toBeHidden();
      expect(await page.evaluate(() => localStorage.getItem('mars.indexingPaused'))).toBeNull();
    });
  });
}
