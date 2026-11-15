import { createRequire } from 'node:module';
import { resolve } from 'node:path';

import { expect, type Page } from '@playwright/test';

import { mockWorker } from './_mars_indexing';
import { test } from './_mars_test';

const MARS_BASE_URL = process.env.MARS_BASE_URL ?? 'https://mars.secutils.dev';
test.use({ baseURL: MARS_BASE_URL });
const require = createRequire(resolve(process.cwd(), '../dev/tools/mars/package.json'));
const { zipSync } = require('fflate') as { zipSync(files: Record<string, Uint8Array>): Uint8Array };

async function boot(page: Page) {
  await page.goto('/');
  await expect(page.locator('html')).not.toHaveClass(/mars-boot/);
}
async function root(page: Page) {
  await boot(page);
  await page.locator('#importInput').setInputFiles({
    name: 'seed.zip',
    mimeType: 'application/zip',
    buffer: Buffer.from(
      zipSync({
        'Root/main.md': Buffer.from('# Main\n\nThe root seed stays intact.'),
        'Root/Nested/seed.md': Buffer.from('# Seed\n\nThe nested seed stays intact.'),
      }),
    ),
  });
  await page.getByRole('button', { name: 'Actions for Root', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
  await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible({ timeout: 30000 });
}
async function pair(owner: Page, peer: Page) {
  const url = await owner.getByRole('textbox', { name: 'Sync link', exact: true }).inputValue();
  await owner.getByRole('button', { name: 'Copy link', exact: true }).click();
  await peer.goto(url);
  await peer.getByRole('button', { name: 'Connect', exact: true }).click();
  await owner.getByRole('button', { name: 'Codes match', exact: true }).click({ timeout: 30000 });
  await owner.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(peer.getByRole('button', { name: 'Actions for Nested', exact: true })).toBeVisible({ timeout: 30000 });
  await peer
    .getByRole('button', { name: 'Actions for Nested', exact: true })
    .locator('..')
    .locator('.tree-label')
    .click();
  await expect(peer.getByRole('link', { name: 'seed', exact: true })).toBeVisible({ timeout: 30000 });
  await peer.getByRole('link', { name: 'seed', exact: true }).click();
}
async function importFiles(page: Page, names: string[]) {
  await page.getByRole('button', { name: 'Actions for Nested', exact: true }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('menuitem', { name: /^Import files/ }).click();
  await (
    await chooser
  ).setFiles(
    names.map((name) => ({
      name: `${name}.md`,
      mimeType: 'text/markdown',
      buffer: Buffer.from(
        `# ${name}\n\nExact imported content for ${name}.\n\n<!-- su-comments\n[{"id":"thread","quote":"Exact imported content","comments":[{"id":"comment","author":"Reviewer","body":"Keep the review comment intact."}]}]\n-->`,
      ),
    })),
  );
}
async function rename(page: Page, from: string, to: string) {
  await page.getByRole('button', { name: `Actions for ${from}`, exact: true }).click();
  await page.getByRole('menuitem', { name: 'Rename', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Name', exact: true });
  await input.fill(to);
  await input.press('Enter');
}
async function inventory(page: Page) {
  return page.evaluate(async () => {
    const db = await new Promise<IDBDatabase>((resolve, reject) => {
      const r = indexedDB.open('mars');
      r.onsuccess = () => resolve(r.result);
      r.onerror = () => reject(r.error);
    });
    const tx = db.transaction(['nodes', 'docs', 'meta'], 'readonly');
    const nodes = tx.objectStore('nodes').getAll(),
      keys = tx.objectStore('docs').getAllKeys(),
      meta = tx.objectStore('meta').getAllKeys();
    await new Promise<void>((resolve, reject) => {
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
    db.close();
    return {
      nodes: nodes.result as {
        id: string;
        name: string;
        parentId: string | null;
        connection?: { collection: string; document: string };
        syncPending?: unknown;
      }[],
      jobs: keys.result.filter((key) => String(key).startsWith('sync-create:')).length,
      archives: meta.result.filter((key) => String(key).startsWith('sync:unfinished-setup:')).length,
    };
  });
}
test.beforeEach(() =>
  test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Local-only regression coverage'),
);

test('twelve tabs retain nested imports, stable registrations, renames, and offline enrollment through reload', async ({
  context,
  browser,
  page,
}) => {
  test.setTimeout(180000);
  const peerContext = await browser.newContext({ baseURL: MARS_BASE_URL });
  const peer = await peerContext.newPage();
  try {
    await root(page);
    await pair(page, peer);
    const tabs = [page];
    for (let i = 1; i < 12; i++) {
      const tab = await context.newPage();
      await boot(tab);
      tabs.push(tab);
    }
    const failed = new Set<string>(),
      requests = new Set<string>();
    let failing = true;
    await context.route('**/sync/document', async (route) => {
      const id = route.request().postDataJSON().document as string;
      if (!requests.has(id)) {
        requests.add(id);
        if ([1, 4, 7].includes(requests.size)) failed.add(id);
      }
      if (failing && failed.has(id))
        await route.fulfill({ status: 503, contentType: 'application/json', body: '{"error":"service-unavailable"}' });
      else await route.continue();
    });
    const names = Array.from({ length: 12 }, (_, i) => `Item-${String(i + 1).padStart(2, '0')}`);
    await context.setOffline(true);
    await importFiles(page, names);
    await expect
      .poll(async () => (await inventory(page)).nodes.filter((n) => n.name.startsWith('Item-')).length)
      .toBe(12);
    await rename(tabs[3], 'Item-08', 'Item-08-renamed');
    expect((await inventory(page)).jobs).toBe(12);
    expect(requests.size).toBe(0);
    await context.setOffline(false);
    await expect.poll(async () => (await inventory(page)).jobs, { timeout: 60000 }).toBe(3);
    for (const tab of tabs) await tab.reload();
    await expect.poll(async () => (await inventory(page)).jobs).toBe(3);
    await page.getByRole('button', { name: 'Actions for Root', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    failing = false;
    await page.getByRole('button', { name: 'Try again', exact: true }).click();
    await expect.poll(async () => (await inventory(page)).jobs, { timeout: 60000 }).toBe(0);
    await expect.poll(async () => (await inventory(page)).nodes.filter((n) => n.syncPending).length).toBe(0);
    await page.getByRole('button', { name: 'Close', exact: true }).click();
    expect(requests.size).toBe(12);
    for (const tab of tabs)
      await expect(tab.locator('#tree .tree-connection')).toHaveCount(1, {
        timeout: 30000,
      });
    for (const name of names.map((name) => (name === 'Item-08' ? `${name}-renamed` : name))) {
      await expect(peer.getByRole('link', { name, exact: true })).toBeVisible({ timeout: 30000 });
      await peer.getByRole('link', { name, exact: true }).click();
      await expect(peer.locator('#previewArticle')).toContainText('Exact imported content');
      await expect(peer.locator('#commentsRail')).toContainText('Keep the review comment intact.');
    }
    expect((await inventory(peer)).nodes.filter((n) => n.name.startsWith('Item-'))).toHaveLength(12);
  } finally {
    await peerContext.close();
  }
});

test('automatic recovery retains stranded replica identity, local edits, review history, and an archive', async ({
  page,
  browser,
}) => {
  test.setTimeout(120000);
  const peerContext = await browser.newContext({ baseURL: MARS_BASE_URL });
  const peer = await peerContext.newPage();
  try {
    await root(page);
    await pair(page, peer);
    await importFiles(page, ['Recover-me']);
    await expect(peer.getByRole('link', { name: 'Recover-me', exact: true })).toBeVisible({ timeout: 30000 });
    await expect(page.locator('#saveState')).toContainText('Up to date', { timeout: 30000 });
    const original = (await inventory(page)).nodes.find((n) => n.name === 'Recover-me')!;
    let rejectUploads = true;
    await page.route('**/sync/append', (route) =>
      rejectUploads
        ? route.fulfill({ status: 503, contentType: 'application/json', body: '{"error":"service-unavailable"}' })
        : route.continue(),
    );
    await page.getByRole('link', { name: 'Recover-me', exact: true }).click();
    await page.getByRole('button', { name: 'Source', exact: true }).click();
    await page.getByRole('textbox', { name: 'Markdown source', exact: true }).waitFor();
    await page.evaluate(() => {
      const editor = (window as unknown as { __suEditorAPI: { getText(): string; setText(text: string): void } })
        .__suEditorAPI;
      editor.setText(`Queued unsent edit.\n\n${editor.getText()}`);
    });
    await page.getByRole('button', { name: 'Preview', exact: true }).click();
    const pending = () =>
      page.evaluate(async (id) => {
        const db = await new Promise<IDBDatabase>((resolve) => {
          const r = indexedDB.open('mars');
          r.onsuccess = () => resolve(r.result);
        });
        const record = await new Promise<{ outbox: { envelope?: unknown }[] }>((resolve) => {
          const r = db.transaction('docs').objectStore('docs').get(`sync:${id}`);
          r.onsuccess = () => resolve(r.result);
        });
        db.close();
        return record.outbox;
      }, original.id);
    await expect.poll(async () => (await pending()).some((packet) => !!packet.envelope)).toBe(true);
    const originalPackets = await pending();
    await page.evaluate(async (id) => {
      const db = await new Promise<IDBDatabase>((resolve) => {
        const r = indexedDB.open('mars');
        r.onsuccess = () => resolve(r.result);
      });
      const tx = db.transaction(['nodes', 'docs'], 'readwrite'),
        nodes = tx.objectStore('nodes'),
        r = nodes.get(id);
      r.onsuccess = () => {
        const node = r.result;
        delete node.connection;
        node.name = 'Locally renamed';
        node.updatedAt++;
        nodes.put(node);
        const text = tx.objectStore('docs').get(id);
        text.onsuccess = () =>
          tx.objectStore('docs').put(String(text.result).replace('Exact imported content', 'Unsent local content'), id);
      };
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, original.id);
    await page.reload();
    await expect
      .poll(async () => (await inventory(page)).nodes.find((n) => n.id === original.id)?.connection?.document, {
        timeout: 30000,
      })
      .toBe(original.connection!.document);
    await expect.poll(async () => (await inventory(page)).archives).toBe(1);
    const archivedPackets = await page.evaluate(async (id) => {
      const db = await new Promise<IDBDatabase>((resolve) => {
        const r = indexedDB.open('mars');
        r.onsuccess = () => resolve(r.result);
      });
      const archives = await new Promise<{ snapshot?: { records: Record<string, { outbox: unknown[] }> } }[]>(
        (resolve) => {
          const r = db.transaction('meta').objectStore('meta').getAll();
          r.onsuccess = () => resolve(r.result);
        },
      );
      db.close();
      return archives.find((value) => value?.snapshot?.records[id])!.snapshot!.records[id].outbox;
    }, original.id);
    expect(archivedPackets).toEqual(originalPackets);
    rejectUploads = false;
    await expect(peer.getByRole('link', { name: 'Locally renamed', exact: true })).toBeVisible({ timeout: 30000 });
    await peer.getByRole('link', { name: 'Locally renamed', exact: true }).click();
    await expect(peer.locator('#previewArticle')).toContainText('Unsent local content');
    await expect(peer.locator('#previewArticle')).toContainText('Queued unsent edit.');
    await expect(peer.locator('#commentsRail')).toContainText('Keep the review comment intact.');
    expect(
      (await inventory(page)).nodes.filter((n) => n.connection?.document === original.connection!.document),
    ).toHaveLength(1);
  } finally {
    await peerContext.close();
  }
});

test('automatic folder reconciliation enrolls missed imports but preserves legacy explicit local copies outside Sync', async ({
  page,
  browser,
  context,
}) => {
  test.setTimeout(120000);
  const peerContext = await browser.newContext({ baseURL: MARS_BASE_URL });
  const peer = await peerContext.newPage();
  try {
    await root(page);
    await pair(page, peer);
    await importFiles(page, ['Kept local']);
    await expect(page.locator('#saveState')).toContainText('Up to date', { timeout: 30000 });
    await page.evaluate(async () => {
      const db = await new Promise<IDBDatabase>((resolve) => {
        const r = indexedDB.open('mars');
        r.onsuccess = () => resolve(r.result);
      });
      const tx = db.transaction(['nodes', 'docs', 'meta'], 'readwrite'),
        nodes = tx.objectStore('nodes'),
        r = nodes.getAll();
      r.onsuccess = () => {
        const kept = r.result.find((node) => node.name === 'Kept local');
        const key = `sync:scope:${kept.connection.collection}:${kept.connection.member}`;
        const scope = tx.objectStore('meta').get(key);
        const remote = kept.connection.document;
        scope.onsuccess = () =>
          tx.objectStore('meta').put({ ...scope.result, exclusions: [...scope.result.exclusions, remote] }, key);
        delete kept.connection;
        kept.originId = crypto.randomUUID();
        nodes.put(kept);
        tx.objectStore('docs').delete(`sync:${kept.id}`);
        const id = crypto.randomUUID();
        nodes.put({
          id,
          originId: id,
          parentId: kept.parentId,
          kind: 'doc',
          name: 'Missed import',
          order: 99,
          createdAt: Date.now(),
          updatedAt: Date.now(),
        });
        tx.objectStore('docs').put('# Missed import\n\nPreserve and sync this formerly skipped text.', id);
      };
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    });
    let registrations = 0;
    await context.route('**/sync/document', async (route) => {
      registrations++;
      await route.continue();
    });
    await page.reload();
    await expect(peer.getByRole('link', { name: 'Missed import', exact: true })).toBeVisible({ timeout: 30000 });
    await expect
      .poll(async () => {
        const { nodes } = await inventory(page),
          kept = nodes.find((node) => node.name === 'Kept local');
        return nodes.find((node) => node.id === kept?.parentId)?.name;
      })
      .toBe('Local copies');
    expect((await inventory(page)).nodes.find((node) => node.name === 'Kept local')!.connection).toBeUndefined();
    expect(registrations).toBe(1);
  } finally {
    await peerContext.close();
  }
});

test('child Sync controls disconnect the whole folder, and both import menus omit their hint', async ({ page }) => {
  test.setTimeout(90000);
  await root(page);
  await page.getByRole('button', { name: 'Close', exact: true }).click();
  await page.getByRole('button', { name: 'Actions for Nested', exact: true }).click();
  let item = page.getByRole('menuitem', { name: 'Import files…', exact: true });
  await expect(item).toHaveText('Import files…');
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'New document, folder, or import', exact: true }).click();
  item = page.getByRole('menuitem', { name: 'Import files…', exact: true });
  await expect(item).toHaveText('Import files…');
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Actions for seed', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Sync', exact: true });
  await expect(dialog.locator('p').first()).toHaveText('seed');
  await expect(dialog).toContainText('Sync inherited from Root');
  await expect(dialog.getByRole('button', { name: 'Disconnect this device', exact: true })).toHaveCount(0);
  await dialog.getByRole('button', { name: 'Manage folder Sync', exact: true }).click();
  await expect(dialog.locator('p').first()).toHaveText('Root');
  await dialog.getByRole('button', { name: 'Back', exact: true }).click();
  await expect(dialog.locator('p').first()).toHaveText('seed');
  await dialog.getByRole('button', { name: 'Manage folder Sync', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Recover unsynced files', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Disconnect this device', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Disconnect this device?', exact: true })).toContainText(
    'This folder and its documents',
  );
  await page.getByRole('button', { name: 'Disconnect', exact: true }).click();
  await expect.poll(async () => (await inventory(page)).nodes.filter((node) => node.connection).length).toBe(0);
});

for (const width of [320, 1280]) {
  for (const theme of ['light', 'dark']) {
    test(`child Sync controls and composite glyphs fit ${width}px ${theme} at fractional zoom`, async ({
      page,
    }, testInfo) => {
      await root(page);
      await page.getByRole('button', { name: 'Close', exact: true }).click();
      await expect(page.locator('#tree .tree-connection')).toHaveCount(1);
      await page.setViewportSize({ width, height: 820 });
      await page.evaluate((theme) => (document.documentElement.dataset.theme = theme), theme);
      const row = (name: string) =>
        page.getByRole('button', { name: `Actions for ${name}`, exact: true }).locator('..');
      if (width === 320) {
        await expect(page.locator('body')).toHaveClass(/sb-collapsed/);
        await page.getByRole('button', { name: 'Toggle documents sidebar', exact: true }).click();
      }
      await expect(row('seed')).toHaveAttribute('aria-description', 'Sync inherited from Root');
      await expect(row('Nested').locator('.tree-connection')).toHaveCount(0);
      await expect(row('seed').locator('.tree-connection')).toHaveCount(0);
      for (const zoom of [1, 1.25, 1.5]) {
        await page.evaluate((zoom) => (document.documentElement.style.zoom = String(zoom)), zoom);
        for (const name of ['Root', 'Nested', 'seed']) {
          const glyph = row(name).locator('.tree-icon');
          await expect(glyph.locator('svg')).toHaveCount(1);
          await expect(glyph.locator('.tree-sync-mark')).toHaveCount(1);
          // SVG geometry can retain its pre-zoom cache until the next layout/paint.
          await expect
            .poll(() => glyph.locator('svg').evaluate((el) => el.getBoundingClientRect().width))
            .toBeCloseTo(16 * zoom, 0);
          const boxes = await glyph.evaluate((el) => {
            const svg = el.querySelector('svg')!,
              mark = el.querySelector('.tree-sync-mark')!;
            return {
              svg: svg.getBoundingClientRect().toJSON(),
              mark: mark.getBoundingClientRect().toJSON(),
              width: getComputedStyle(svg).width,
              zoom: document.documentElement.style.zoom,
              animations: svg.getAnimations({ subtree: true }).filter((animation) => animation instanceof CSSAnimation)
                .length,
            };
          });
          expect(boxes.svg.width, JSON.stringify({ name, zoom, boxes })).toBeCloseTo(16 * zoom, 0);
          expect(boxes.mark.x).toBeGreaterThan(boxes.svg.x);
          expect(boxes.mark.y).toBeGreaterThan(boxes.svg.y);
          expect(boxes.mark.x + boxes.mark.width).toBeLessThan(boxes.svg.x + boxes.svg.width);
          expect(boxes.mark.y + boxes.mark.height).toBeLessThan(boxes.svg.y + boxes.svg.height);
          expect(boxes.animations).toBe(0);
        }
        for (const name of ['seed', 'Nested']) {
          await page.getByRole('button', { name: `Actions for ${name}`, exact: true }).click();
          await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
          const dialog = page.getByRole('dialog', { name: 'Sync', exact: true });
          await expect(dialog.locator('p').first()).toHaveText(name);
          await expect(dialog.getByRole('button', { name: 'Devices', exact: true })).toHaveCount(0);
          await expect(dialog.getByRole('button', { name: 'Disconnect this device', exact: true })).toHaveCount(0);
          if (zoom === 1.25 && name === 'seed') {
            const path = testInfo.outputPath('item-status.png');
            await page.screenshot({ path });
            await testInfo.attach('item status', { path, contentType: 'image/png' });
          }
          await dialog.getByRole('button', { name: 'Manage folder Sync', exact: true }).click();
          await expect(dialog.locator('p').first()).toHaveText('Root');
          await expect(dialog).toContainText('Folder settings apply to this folder and all its contents.');
          const bounds = (await dialog.boundingBox())!;
          expect(bounds.x).toBeGreaterThanOrEqual(0);
          expect(bounds.x + bounds.width).toBeLessThanOrEqual(width + 1);
          for (const control of await dialog.locator('footer button').all()) {
            await control.scrollIntoViewIfNeeded();
            const box = (await control.boundingBox())!;
            expect(box.x).toBeGreaterThanOrEqual(bounds.x);
            expect(box.x + box.width).toBeLessThanOrEqual(bounds.x + bounds.width);
          }
          if (zoom === 1.25 && name === 'seed') {
            const path = testInfo.outputPath('folder-settings.png');
            await page.screenshot({ path });
            await testInfo.attach('folder settings', { path, contentType: 'image/png' });
          }
          await dialog.getByRole('button', { name: 'Back', exact: true }).click();
          await expect(dialog.locator('p').first()).toHaveText(name);
          await dialog.getByRole('button', { name: 'Close', exact: true }).click();
        }
      }
      await page.evaluate(() => (document.documentElement.style.zoom = '1.25'));
      await page.getByRole('button', { name: 'Actions for Root', exact: true }).click();
      await page.getByRole('menuitem', { name: 'Use as default', exact: true }).click();
      for (let i = 0; i < 2; i++) {
        await expect(row('Root').locator('.tree-icon svg')).toHaveCount(1);
        await expect(row('Root').locator('.tree-default-star')).toHaveCount(1);
        await expect(row('Root').locator('.tree-sync-mark')).toHaveCount(0);
        await row('Root').locator('.tree-label').click();
      }
      await expect(row('Root')).toHaveAttribute('aria-description', 'Default destination. Sync configured');
      const path = testInfo.outputPath('sidebar-glyphs.png');
      await page.screenshot({ path });
      await testInfo.attach('sidebar glyphs', { path, contentType: 'image/png' });
      if (testInfo.project.name === 'chromium') {
        await page.emulateMedia({ forcedColors: 'active' });
        const forced = await row('seed')
          .locator('.tree-sync-mark')
          .evaluate((el) => {
            const probe = document.createElement('span');
            probe.style.color = 'CanvasText';
            document.body.append(probe);
            const color = getComputedStyle(probe).color;
            probe.remove();
            return {
              fill: getComputedStyle(el).fill,
              stroke: getComputedStyle(el).stroke,
              color,
              active: matchMedia('(forced-colors: active)').matches,
            };
          });
        expect(forced.active).toBe(true);
        await expect(row('seed').locator('.tree-sync-mark')).toHaveCSS('fill', forced.color);
        await expect(row('seed').locator('.tree-sync-mark')).toHaveCSS('stroke', forced.color);
      }
    });
  }
}

test('child Sync controls explain pending enrollment while only roots and issues keep badges', async ({
  page,
  context,
}) => {
  await root(page);
  await page.getByRole('button', { name: 'Close', exact: true }).click();
  await context.route('**/sync/document', (route) =>
    route.fulfill({ status: 503, contentType: 'application/json', body: '{"error":"service-unavailable"}' }),
  );
  await importFiles(page, ['Pending']);
  const row = page.getByRole('button', { name: 'Actions for Pending', exact: true }).locator('..');
  await expect(row.locator('.tree-connection[data-phase="attention"]')).toHaveCount(1, { timeout: 15000 });
  await expect(page.locator('#tree .tree-connection')).toHaveCount(2);
  await expect(row.locator('.tree-sync-mark')).toHaveCount(0);
  await row.locator('.tree-connection').click();
  const dialog = page.getByRole('dialog', { name: 'Sync', exact: true });
  await expect(dialog).toContainText('Saved locally · Sync enrollment needs attention');
  await expect(dialog.getByRole('button', { name: 'Try again', exact: true })).toHaveCount(0);
  await dialog.getByRole('button', { name: 'Manage folder Sync', exact: true }).click();
  await expect(dialog.getByRole('button', { name: 'Try again', exact: true })).toBeVisible();
  await context.unroute('**/sync/document');
  await dialog.getByRole('button', { name: 'Try again', exact: true }).click();
  await expect.poll(async () => (await inventory(page)).jobs, { timeout: 30000 }).toBe(0);
  await dialog.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(row.locator('.tree-sync-mark')).toHaveCount(1);
  await expect(row.locator('.tree-connection')).toHaveCount(0);
});

test('twelve tabs share inference and GTE renames only update index metadata', async ({ page, context }) => {
  test.setTimeout(120000);
  await mockWorker(context);
  await boot(page);
  await page.locator('#importInput').setInputFiles({
    name: 'Indexed.md',
    mimeType: 'text/markdown',
    buffer: Buffer.from('# Indexed\n\nA document with enough words to index.'),
  });
  await page.getByRole('button', { name: 'Smart search', exact: true }).click();
  await page.getByRole('radio', { name: /Best speed/ }).check();
  await page.locator('#marsModalFoot').getByRole('button', { name: 'Turn on', exact: true }).click();
  await expect(page.locator('#sbSmartStatus')).toBeHidden();
  const tabs = [page];
  for (let i = 1; i < 12; i++) {
    const tab = await context.newPage();
    await boot(tab);
    tabs.push(tab);
  }
  const calls = async () =>
    (
      await Promise.all(
        tabs.map((tab) =>
          tab.evaluate(() => (window as unknown as { __marsWorker: { embeds: unknown[] } }).__marsWorker.embeds.length),
        ),
      )
    ).reduce((a, b) => a + b, 0);
  await rename(page, 'Indexed', 'Renamed');
  await page.waitForTimeout(4000);
  expect(await calls()).toBe(1);
  await page.getByRole('link', { name: 'Renamed', exact: true }).click();
  await page.getByRole('button', { name: 'Source', exact: true }).click();
  await page.getByRole('textbox', { name: 'Markdown source', exact: true }).waitFor();
  await page.evaluate(() =>
    (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(
      '# Indexed\n\nChanged body with sufficiently many distinct words.',
    ),
  );
  await page.getByRole('button', { name: 'Preview', exact: true }).click();
  await expect.poll(calls, { timeout: 15000 }).toBe(2);
  await page.waitForTimeout(4000);
  expect(await calls()).toBe(2);
});

test('EmbeddingGemma renames change the exact passage input and require new vectors', async ({ page, context }) => {
  await mockWorker(context);
  await context.addInitScript(() =>
    Object.defineProperty(navigator, 'gpu', {
      value: { requestAdapter: async () => ({ features: new Set(['shader-f16']), isFallbackAdapter: false }) },
    }),
  );
  await boot(page);
  await page.locator('#importInput').setInputFiles({
    name: 'Quality.md',
    mimeType: 'text/markdown',
    buffer: Buffer.from('# Body heading\n\nAn unchanged paragraph with sufficiently many words.'),
  });
  await page.getByRole('button', { name: 'Smart search', exact: true }).click();
  await page.getByRole('radio', { name: /Best quality/ }).check();
  await page.locator('#marsModalFoot').getByRole('button', { name: 'Turn on', exact: true }).click();
  const inputs = () =>
    page.evaluate(() => (window as unknown as { __marsWorker: { embeds: string[][] } }).__marsWorker.embeds);
  await expect.poll(async () => (await inputs()).length).toBe(1);
  expect((await inputs())[0][0]).toMatch(/^title: Quality \| text: /);
  await rename(page, 'Quality', 'Renamed quality');
  await expect.poll(async () => (await inputs()).length).toBe(2);
  expect((await inputs())[1][0]).toMatch(/^title: Renamed quality \| text: /);
});
