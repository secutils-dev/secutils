import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';

import { type Browser, expect, type Page } from '@playwright/test';

import { test } from './_mars_test';

const baseURL = process.env.MARS_BASE_URL ?? 'http://localhost:7373';
test.use({ baseURL });
const require = createRequire(resolve(process.cwd(), '../dev/tools/js/mars/package.json'));
const { unzipSync } = require('fflate') as { unzipSync(bytes: Uint8Array): Record<string, Uint8Array> };
const { zipSync } = require('fflate') as { zipSync(files: Record<string, Uint8Array>): Uint8Array };
async function clients(browser: Browser, count = 2) {
  const client = crypto.randomUUID(),
    origin = new URL(baseURL).origin;
  const contexts = await Promise.all(Array.from({ length: count }, () => browser.newContext()));
  for (const context of contexts)
    await context.route(`${origin}/**`, (route) =>
      route.continue({ headers: { ...route.request().headers(), 'x-mars-test-client': client } }),
    );
  const pages = await Promise.all(contexts.map((context) => context.newPage()));
  return {
    contexts,
    pages,
    close: async () => {
      await Promise.all(pages.map((page) => page.goto('about:blank').catch(() => {})));
      await Promise.all(contexts.map((context) => context.close().catch(() => {})));
    },
  };
}
async function openDoc(page: Page, name: string, text: string) {
  if (page.url() === 'about:blank') {
    await page.goto(baseURL);
    await expect(page.locator('#emptyState')).toBeVisible();
  }
  await page
    .locator('#fileInput')
    .setInputFiles({ name: `${name}.md`, mimeType: 'text/markdown', buffer: Buffer.from(text) });
  await expect(page.locator('#docTitle')).toHaveText(name);
}
async function live(page: Page, permission = 'view') {
  return (await import('./_mars_sharing')).copyLink(page, {
    mode: 'live',
    permission: permission as 'view' | 'comment' | 'edit',
  });
}
async function connections(page: Page, name: string) {
  await page.getByRole('button', { name: `Actions for ${name}`, exact: true }).click();
  await page.getByRole('menuitem', { name: 'Copy link', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'Copy link', exact: true })
    .getByRole('button', { name: 'Live links', exact: true })
    .click();
}

test.describe('sync protocol follow-up', () => {
  test('duplicate identity repair keeps distinct local reviews and pending work through encrypted backup restoration', async ({
    browser,
  }) => {
    test.setTimeout(90000);
    const setup = await clients(browser),
      [owner, peer] = setup.pages;
    const read = (page: Page) =>
      page.evaluate(async () => {
        const database = await new Promise<IDBDatabase>((resolve) => {
          const open = indexedDB.open('mars');
          open.onsuccess = () => resolve(open.result);
        });
        const transaction = database.transaction(['nodes', 'docs', 'meta']);
        const get = (store: string, method: 'getAll' | 'getAllKeys') =>
          new Promise<unknown[]>((resolve) => {
            const request = transaction.objectStore(store)[method]();
            request.onsuccess = () => resolve(request.result);
          });
        const [nodes, values, keys, metadata, metaKeys] = await Promise.all([
          get('nodes', 'getAll'),
          get('docs', 'getAll'),
          get('docs', 'getAllKeys'),
          get('meta', 'getAll'),
          get('meta', 'getAllKeys'),
        ]);
        database.close();
        return {
          nodes,
          docs: Object.fromEntries(keys.map((key, index) => [String(key), values[index]])),
          meta: Object.fromEntries(metaKeys.map((key, index) => [String(key), metadata[index]])),
        };
      });
    try {
      await owner.goto(baseURL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.locator('#importInput').setInputFiles({
        name: 'Root.zip',
        mimeType: 'application/zip',
        buffer: Buffer.from(zipSync({ 'Root/Review.md': Buffer.from('# Review\n\nOriginal text.') })),
      });
      await expect(owner.locator('#toast')).toContainText('Imported 1 document');
      await owner.getByRole('button', { name: 'Actions for Root', exact: true }).click();
      await owner.getByRole('menuitem', { name: 'Sync', exact: true }).click();
      await owner.getByRole('button', { name: 'Turn on sync', exact: true }).click();
      await expect(owner.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible();
      const injected = await owner.evaluate(async () => {
        const database = await new Promise<IDBDatabase>((resolve) => {
          const open = indexedDB.open('mars');
          open.onsuccess = () => resolve(open.result);
        });
        const tx = database.transaction(['nodes', 'docs'], 'readwrite'),
          store = tx.objectStore('nodes'),
          docs = tx.objectStore('docs');
        const request = <T>(value: IDBRequest<T>) =>
          new Promise<T>((resolve) => {
            value.onsuccess = () => resolve(value.result);
          });
        const nodes = await request(store.getAll()),
          original = nodes.find((node) => node.kind === 'doc');
        const record = await request(docs.get(`sync:${original.id}`)),
          text = await request(docs.get(original.id));
        const extra = { ...original, id: crypto.randomUUID(), createdAt: Date.now() + 1000 },
          identical = { ...extra, id: crypto.randomUUID() };
        const local = `${text}\n\n<!-- su-comments\n[{"id":"local-thread","quote":"Original","comments":[{"id":"local-comment","author":"Local","body":"UNIQUE LOCAL REVIEW"}]}]\n-->\n`;
        store.put({ ...original, createdAt: 0 });
        store.put(extra);
        store.put(identical);
        docs.put(local, extra.id);
        docs.put(
          {
            ...record,
            projection: '# UNSENT DISTINCT PROJECTION',
            outbox: [
              { operation: 'preserved-pending-operation', sequence: 77, bytes: 'exact pending bytes', envelope: null },
            ],
          },
          `sync:${extra.id}`,
        );
        docs.put(text, identical.id);
        docs.put(record, `sync:${identical.id}`);
        for (const kind of ['base', 'opened']) {
          const value = await request(docs.get(`${kind}:${original.id}`));
          if (value !== undefined) for (const node of [extra, identical]) docs.put(value, `${kind}:${node.id}`);
        }
        await new Promise<void>((resolve, reject) => {
          tx.oncomplete = () => resolve();
          tx.onabort = () => reject(tx.error);
        });
        database.close();
        return { original: original.id, extra: extra.id, identical: identical.id, local };
      });
      await owner.reload();
      await expect(owner.getByRole('button', { name: 'Actions for Recovered Sync copies', exact: true })).toBeVisible();
      const repaired = await read(owner);
      expect(repaired.docs[injected.extra]).toBe(injected.local);
      expect(repaired.docs[`sync:${injected.extra}`]).toBeUndefined();
      expect(repaired.docs[injected.identical]).toBeUndefined();
      expect(Object.values(repaired.docs)).toContain('# UNSENT DISTINCT PROJECTION');
      expect(Object.keys(repaired.docs).filter((key) => key.startsWith('sync:'))).toHaveLength(2);
      const archives = Object.entries(repaired.meta).filter(([key]) => key.startsWith('sync:unfinished-setup:'));
      expect(archives).toHaveLength(1);
      expect(JSON.stringify(archives)).toContain('exact pending bytes');
      await owner.locator('#sbExportAllBtn').click();
      await owner.getByRole('checkbox', { name: 'Include connections and pending changes', exact: true }).check();
      await owner.getByLabel('Password', { exact: true }).fill('preserved duplicate password');
      const downloaded = owner.waitForEvent('download');
      await owner.getByRole('button', { name: 'Download', exact: true }).click();
      const file = await downloaded,
        path = await file.path(),
        bytes = await readFile(path!);
      await owner.goto('about:blank');
      await peer.goto(baseURL);
      await expect(peer.locator('#emptyState')).toBeVisible();
      await peer.getByRole('button', { name: 'New document, folder, or import', exact: true }).click();
      const choosing = peer.waitForEvent('filechooser');
      await peer.getByRole('menuitem', { name: 'Restore backup…', exact: true }).click();
      await (await choosing).setFiles({ name: file.suggestedFilename(), mimeType: 'application/zip', buffer: bytes });
      await peer.getByLabel('Backup password', { exact: true }).fill('preserved duplicate password');
      await peer.getByRole('button', { name: 'Restore', exact: true }).click();
      await expect(peer.locator('#toast')).toContainText('Backup restored', { timeout: 30000 });
      const restored = await read(peer);
      expect(Object.values(restored.docs)).toContain(injected.local);
      expect(Object.values(restored.docs)).toContain('# UNSENT DISTINCT PROJECTION');
      expect(Object.keys(restored.docs).filter((key) => key.startsWith('sync:'))).toHaveLength(2);
      expect(JSON.stringify(restored.meta)).toContain('exact pending bytes');
      await peer.reload();
      expect(Object.keys((await read(peer)).docs).filter((key) => key.startsWith('sync:'))).toHaveLength(2);
    } finally {
      await setup.close();
    }
  });

  test('generic developer flags have one report location and profile-wide Off controls that survive reload', async ({
    page,
  }) => {
    await page.goto(`${baseURL}?flags=debug:sync,experimental:live_links`);
    await expect(page.locator('#emptyState')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Developer tools', exact: true })).toBeVisible();
    expect(new URL(page.url()).searchParams.has('flags')).toBe(false);
    const state = (page: Page) =>
      page.evaluate(() => (window as unknown as { marsFlags: { list(): string[] } }).marsFlags.list());
    expect(await state(page)).toEqual(['debug:sync', 'experimental:live_links']);
    const other = await page.context().newPage();
    await other.goto(baseURL);
    await expect(other.locator('#emptyState')).toBeVisible();
    expect(await state(other)).toEqual(await state(page));
    await page.getByRole('button', { name: 'Developer tools', exact: true }).click();
    const panel = page.getByRole('dialog', { name: 'Developer tools', exact: true });
    await expect(panel.getByRole('button', { name: 'Copy debug report', exact: true })).toBeVisible();
    await panel.getByRole('button', { name: 'Turn off debug', exact: true }).click();
    expect(await state(page)).toEqual(['experimental:live_links']);
    await expect.poll(() => state(other)).toEqual(['experimental:live_links']);
    await expect(panel.getByRole('button', { name: 'Copy debug report', exact: true })).toHaveCount(0);
    await panel.getByRole('button', { name: 'Close', exact: true }).click();
    await page.reload();
    await expect(page.getByRole('button', { name: 'Developer tools', exact: true })).toBeVisible();
    expect(await state(page)).toEqual(['experimental:live_links']);
    await page.getByRole('button', { name: 'Developer tools', exact: true }).click();
    await panel.getByRole('button', { name: 'Turn off experimental', exact: true }).click();
    expect(await state(page)).toEqual([]);
    await expect.poll(() => state(other)).toEqual([]);
    await panel.getByRole('button', { name: 'Close', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Developer tools', exact: true })).toBeHidden();
    await page.goto(`${baseURL}?debug=sync&debug=live&experimental=sync&experimental=live_links`);
    await expect(page.locator('#emptyState')).toBeVisible();
    expect(await state(page)).toEqual(['debug:live', 'debug:sync', 'experimental:live_links', 'experimental:sync']);
    await page.goto(`${baseURL}?flags=off`);
    await expect(page.locator('#emptyState')).toBeVisible();
    expect(await state(page)).toEqual([]);
    await expect.poll(() => state(other)).toEqual([]);
    await page.reload();
    await expect(page.locator('#emptyState')).toBeVisible();
    expect(await state(page)).toEqual([]);
    await other.close();
  });

  test('58-item folder with 25 unbound replicas restarts atomically and restores old pending work beside newer local text', async ({
    browser,
  }) => {
    test.setTimeout(180000);
    const c = await clients(browser, 1),
      [owner] = c.pages;
    const storage = (page: Page) =>
      page.evaluate(async () => {
        const database = await new Promise<IDBDatabase>((resolve) => {
          const open = indexedDB.open('mars');
          open.onsuccess = () => resolve(open.result);
        });
        const tx = database.transaction(['nodes', 'docs', 'meta']);
        const nodes = tx.objectStore('nodes').getAll(),
          keys = tx.objectStore('docs').getAllKeys(),
          values = tx.objectStore('docs').getAll(),
          metaKeys = tx.objectStore('meta').getAllKeys(),
          metaValues = tx.objectStore('meta').getAll();
        await new Promise<void>((resolve) => {
          tx.oncomplete = () => resolve();
        });
        database.close();
        const docs = Object.fromEntries(keys.result.map((key, index) => [String(key), values.result[index]]));
        return {
          nodes: nodes.result,
          docs,
          metadata: Object.fromEntries(metaKeys.result.map((key, index) => [String(key), metaValues.result[index]])),
        };
      });
    try {
      await owner.goto(baseURL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.evaluate(() => {
        localStorage.setItem('mars.smartNudge', '{"never":true}');
        (window as unknown as { marsFlags: { set(value: string): void } }).marsFlags.set('debug:sync');
      });
      await owner.locator('#importInput').setInputFiles({
        name: 'Interrupted.zip',
        mimeType: 'application/zip',
        buffer: Buffer.from(
          zipSync(
            Object.fromEntries(
              Array.from({ length: 50 }, (_, index) => [
                `Work/Folder ${index % 7}/Document ${index}.md`,
                Buffer.from(`# Document ${index}\n\nOriginal text ${index}.`),
              ]),
            ),
          ),
        ),
      });
      await expect(owner.locator('#toast')).toContainText('Imported 50 documents');
      await owner.getByRole('button', { name: 'Actions for Work', exact: true }).click();
      await owner.getByRole('menuitem', { name: 'Sync', exact: true }).click();
      await owner.getByRole('button', { name: 'Turn on sync', exact: true }).click();
      await expect(owner.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible({ timeout: 60000 });
      await owner.getByRole('button', { name: 'Close', exact: true }).click();
      await owner.route('**/sync/append', (route) =>
        route.fulfill({ status: 503, json: { error: 'service-unavailable' } }),
      );
      await owner.getByRole('link', { name: 'Document 0', exact: true }).click();
      await expect(owner.locator('#docTitle')).toHaveText('Document 0');
      const pendingId = new URL(owner.url()).hash.slice(5);
      await owner.getByRole('button', { name: 'Source', exact: true }).click();
      await owner.waitForFunction(() => !!(window as unknown as { __suEditorAPI?: unknown }).__suEditorAPI);
      await owner.evaluate(() =>
        (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(
          '# Document 0\n\nOlder unsent work.',
        ),
      );
      await expect
        .poll(async () =>
          (await storage(owner)).docs[`sync:${pendingId}`]?.outbox.some(
            (packet: { envelope: unknown }) => packet.envelope,
          ),
        )
        .toBe(true);
      await owner.goto('about:blank');
      await owner.goto(`${baseURL}?temporary=1`);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.evaluate(async (pendingId) => {
        const database = await new Promise<IDBDatabase>((resolve) => {
          const open = indexedDB.open('mars');
          open.onsuccess = () => resolve(open.result);
        });
        const tx = database.transaction(['nodes', 'docs'], 'readwrite'),
          nodes = tx.objectStore('nodes').getAll();
        nodes.onsuccess = () => {
          const root = nodes.result.find((node) => node.kind === 'folder' && !node.parentId);
          const keep = new Set([
            root.id,
            pendingId,
            ...nodes.result
              .filter((node) => node.kind === 'doc' && node.id !== pendingId)
              .slice(0, 23)
              .map((node) => node.id),
          ]);
          for (const node of nodes.result) {
            delete node.connection;
            if (node.id === pendingId) {
              node.updatedAt++;
              tx.objectStore('docs').put('# Document 0\n\nNewer local text.', node.id);
            }
            tx.objectStore('nodes').put(node);
            if (!keep.has(node.id)) tx.objectStore('docs').delete(`sync:${node.id}`);
            if (node.kind === 'doc')
              tx.objectStore('docs').put({ id: node.id, at: node.updatedAt, chunks: [] }, `vec:${node.id}`);
          }
        };
        await new Promise<void>((resolve) => {
          tx.oncomplete = () => resolve();
        });
        database.close();
      }, pendingId);
      await owner.unroute('**/sync/append');
      const before = await storage(owner),
        root = before.nodes.find((node) => node.kind === 'folder' && !node.parentId);
      const oldRecords = Object.fromEntries(
        Object.entries(before.docs)
          .filter(([key]) => key.startsWith('sync:'))
          .map(([key, value]) => [key.slice(5), value]),
      );
      expect(Object.keys(oldRecords)).toHaveLength(25);
      await owner.goto(baseURL);
      await expect(owner.locator('#docTitle')).toHaveText('Document 0');
      const other = await owner.context().newPage();
      await other.goto(baseURL);
      await expect(other.locator('#docTitle')).toHaveText('Document 0');
      await owner.bringToFront();
      const beginRestart = async () => {
        await owner.getByRole('button', { name: 'Actions for Work', exact: true }).click();
        await owner.getByRole('menuitem', { name: 'Sync', exact: true }).click();
        await owner.getByRole('button', { name: 'Turn on sync', exact: true }).click();
        await expect(owner.getByRole('dialog', { name: 'Sync did not finish', exact: true })).toContainText(
          'orphan-replica',
        );
        await owner.getByRole('button', { name: 'Restart setup', exact: true }).click();
        await expect(owner.getByRole('dialog', { name: 'Restart unfinished Sync setup?', exact: true })).toContainText(
          'Unsent work',
        );
      };
      await beginRestart();
      // An unsuccessful archive transaction must not retire any of the 25 old records.
      await owner.evaluate(() => {
        const add = IDBObjectStore.prototype.add;
        IDBObjectStore.prototype.add = function (value, key) {
          if (this.name === 'meta' && value?.kind === 'mars-unfinished-sync') {
            IDBObjectStore.prototype.add = add;
            throw new DOMException('Archive quota test', 'QuotaExceededError');
          }
          return add.call(this, value, key);
        };
      });
      await owner.getByRole('button', { name: 'Restart setup', exact: true }).click();
      await expect(owner.getByRole('dialog', { name: 'Sync did not finish', exact: true })).toContainText(
        'saving-connections',
        { timeout: 60000 },
      );
      let after = await storage(owner);
      expect(
        Object.fromEntries(
          Object.entries(after.docs)
            .filter(([key]) => key.startsWith('sync:'))
            .map(([key, value]) => [key.slice(5), value]),
        ),
      ).toEqual(oldRecords);
      expect(after.metadata[`sync:unfinished-backups:${root.id}`]).toBeUndefined();
      await owner.getByRole('button', { name: 'Back to Sync', exact: true }).click();
      await owner.getByRole('button', { name: 'Close', exact: true }).click();
      await beginRestart();
      await owner.getByRole('button', { name: 'Restart setup', exact: true }).click();
      await expect(owner.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible({ timeout: 60000 });
      after = await storage(owner);
      expect(after.nodes.filter((node) => node.connection)).toHaveLength(58);
      for (const node of before.nodes) {
        expect(after.nodes.find((current) => current.id === node.id)?.updatedAt).toBe(node.updatedAt);
        expect(after.docs[node.id]).toEqual(before.docs[node.id]);
        expect(after.docs[`vec:${node.id}`]).toEqual(before.docs[`vec:${node.id}`]);
      }
      const index = after.metadata[`sync:unfinished-backups:${root.id}`];
      expect(index).toHaveLength(1);
      const saved = after.metadata[index[0].key];
      expect(saved.snapshot.records).toEqual(oldRecords);
      expect(saved.snapshot.texts[pendingId]).toContain('Newer local text.');
      expect(after.docs[`sync:${pendingId}`].binding.collection).not.toBe(oldRecords[pendingId].binding.collection);
      await owner.getByRole('button', { name: 'Back', exact: true }).click();
      await expect(owner.getByRole('button', { name: 'Previous setup backup', exact: true })).toHaveCount(0);
      await expect(owner.getByRole('button', { name: 'Copy debug report', exact: true })).toHaveCount(0);
      const footer = owner.locator('#syncDialog footer button:visible');
      await expect(footer).toHaveText(['Disconnect this device', 'Sync link']);
      for (const width of [1280, 320]) {
        await owner.setViewportSize({ width, height: 800 });
        const tops = await footer.evaluateAll((buttons) => buttons.map((button) => button.getBoundingClientRect().top));
        expect(Math.max(...tops) - Math.min(...tops)).toBeLessThan(1);
      }
      await owner.setViewportSize({ width: 1280, height: 800 });
      await owner.getByRole('button', { name: 'Close', exact: true }).click();
      await owner.locator('#sbExportAllBtn').click();
      await owner.getByRole('checkbox', { name: 'Include connections and pending changes', exact: true }).check();
      await owner.getByLabel('Password', { exact: true }).fill('short');
      await owner.getByRole('button', { name: 'Download', exact: true }).click();
      await expect(owner.getByRole('alert')).toContainText('at least 12');
      await owner.getByLabel('Password', { exact: true }).fill('previous setup password');
      const downloaded = owner.waitForEvent('download');
      await owner.getByRole('button', { name: 'Download', exact: true }).click();
      const file = await downloaded,
        bytes = await readFile((await file.path())!);
      expect(bytes.toString()).not.toContain('Older unsent work.');
      const peerContext = await browser.newContext(),
        peer = await peerContext.newPage();
      await peer.route('**/sync/index', (route) => route.fulfill({ json: { entries: [], complete: true } }));
      await peer.route('**/sync/append', (route) =>
        route.fulfill({ status: 503, json: { error: 'service-unavailable' } }),
      );
      await openDoc(peer, 'Existing private note', '# Existing private note\n\nMust stay here.');
      await peer.getByRole('button', { name: 'New document, folder, or import', exact: true }).click();
      const choosing = peer.waitForEvent('filechooser');
      await peer.getByRole('menuitem', { name: 'Restore backup…', exact: true }).click();
      await (await choosing).setFiles({ name: file.suggestedFilename(), mimeType: 'application/zip', buffer: bytes });
      await peer.getByLabel('Backup password', { exact: true }).fill('previous setup password');
      await peer.getByRole('button', { name: 'Restore', exact: true }).click();
      await expect(peer.locator('#toast')).toContainText('Backup restored', { timeout: 15000 });
      const restored = await storage(peer),
        oldPending = Object.values(restored.docs).find(
          (value) => value?.kind === 'connected' && value.binding.document === oldRecords[pendingId].binding.document,
        );
      expect(oldPending.outbox).toEqual(oldRecords[pendingId].outbox);
      expect(oldPending.key).toBe(oldRecords[pendingId].key);
      expect(oldPending.projection).toContain('Older unsent work.');
      expect(Object.values(restored.docs)).toContain('# Document 0\n\nNewer local text.');
      expect(Object.values(restored.docs)).toContain('# Existing private note\n\nMust stay here.');
      await peerContext.close();
    } finally {
      await c.close();
    }
  });

  test('debug query survives reload and failed setup stays visible with a safe copyable report', async ({ page }) => {
    await page.goto(`${baseURL}?flags=debug:sync`);
    await expect(page.locator('#emptyState')).toBeVisible();
    await openDoc(
      page,
      'Private diagnostics note',
      '# Private diagnostics note\n\nConfidential Markdown must never enter a debug report.',
    );
    await page.reload();
    await expect(page.locator('#docTitle')).toHaveText('Private diagnostics note');
    await page.route('**/sync/create', (route) =>
      route.fulfill({ status: 503, json: { error: 'service-unavailable' } }),
    );
    await page.getByRole('button', { name: 'Actions for Private diagnostics note', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
    const failure = page.getByRole('dialog', { name: 'Sync did not finish', exact: true });
    await expect(failure.getByRole('alert')).toContainText('service-unavailable');
    await expect(failure.getByRole('alert')).toContainText('creating-collection');
    await expect(page.getByRole('button', { name: 'Turn on sync', exact: true })).toHaveCount(0);
    await page.evaluate(() =>
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: {
          writeText: async (text: string) => {
            (window as unknown as { debugCopy: string }).debugCopy = text;
          },
        },
      }),
    );
    await expect(failure.getByRole('button', { name: 'Copy debug report', exact: true })).toHaveCount(0);
    await failure.getByRole('button', { name: 'Close', exact: true }).click();
    await page.getByRole('button', { name: 'Developer tools', exact: true }).click();
    const developer = page.getByRole('dialog', { name: 'Developer tools', exact: true });
    await developer.getByRole('button', { name: 'Copy debug report', exact: true }).click();
    const raw = await page.evaluate(() => (window as unknown as { debugCopy: string }).debugCopy);
    const report = JSON.parse(raw).channels.sync;
    expect(report.enabled).toBe(true);
    expect(report.events).toContainEqual(
      expect.objectContaining({ event: 'request.finished', action: 'create', status: 503 }),
    );
    expect(report.events).toContainEqual(
      expect.objectContaining({ event: 'setup.failed', stage: 'creating-collection', code: 'service-unavailable' }),
    );
    expect(raw).not.toContain('Private diagnostics note');
    expect(raw).not.toContain('Confidential Markdown');
    expect(raw).not.toContain('#doc=');
    await developer.getByRole('button', { name: 'Close', exact: true }).click();
    await page.unroute('**/sync/create');
    await page.getByRole('button', { name: 'Actions for Private diagnostics note', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
    await expect(page.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible({ timeout: 15000 });
  });

  test('profile debug mode diagnoses orphan records with two tabs without replacing their credentials', async ({
    page,
  }) => {
    await openDoc(page, 'Orphan diagnostics note', '# Orphan diagnostics note\n\nKeep this private content.');
    await page.evaluate(() =>
      (window as unknown as { marsFlags: { set(value: string): void } }).marsFlags.set('debug:sync'),
    );
    const other = await page.context().newPage();
    await other.goto(baseURL);
    await expect(other.locator('#docTitle')).toHaveText('Orphan diagnostics note');
    let creates = 0;
    page.on('request', (request) => {
      if (new URL(request.url()).pathname === '/sync/create') creates++;
    });
    await page.evaluate(async () => {
      const database = await new Promise<IDBDatabase>((resolve) => {
        const open = indexedDB.open('mars');
        open.onsuccess = () => resolve(open.result);
      });
      const tx = database.transaction(['nodes', 'docs'], 'readwrite'),
        nodes = tx.objectStore('nodes').getAll();
      nodes.onsuccess = () =>
        tx.objectStore('docs').put(
          {
            kind: 'connected',
            revision: 1,
            binding: { collection: crypto.randomUUID(), document: crypto.randomUUID(), member: crypto.randomUUID() },
            token: 'DO-NOT-EXPORT-THIS-CREDENTIAL',
            projection: 'DO-NOT-EXPORT-THIS-PROJECTION',
          },
          `sync:${nodes.result[0].id}`,
        );
      await new Promise<void>((resolve) => {
        tx.oncomplete = () => resolve();
      });
      database.close();
    });
    await page.getByRole('button', { name: 'Actions for Orphan diagnostics note', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
    const failure = page.getByRole('dialog', { name: 'Sync did not finish', exact: true });
    await expect(failure.getByRole('alert')).toContainText('orphan-replica');
    await expect(failure.getByRole('alert')).toContainText('checking-selection');
    const report = await page.evaluate(
      () => JSON.parse((window as unknown as { marsDebug: { report(): string } }).marsDebug.report()).channels.sync,
    );
    expect(report.events).toContainEqual(expect.objectContaining({ orphanRecords: 1, items: 1 }));
    expect(report.enabled).toBe(true);
    expect(
      await other.evaluate(
        () =>
          JSON.parse((window as unknown as { marsDebug: { report(): string } }).marsDebug.report()).channels.sync
            .enabled,
      ),
    ).toBe(true);
    expect(JSON.stringify(report)).not.toMatch(/DO-NOT-EXPORT|Orphan diagnostics note|Keep this private content/);
    expect(creates).toBe(0);
    const stored = await page.evaluate(async () => {
      const database = await new Promise<IDBDatabase>((resolve) => {
        const open = indexedDB.open('mars');
        open.onsuccess = () => resolve(open.result);
      });
      const records = await new Promise<{ token: string }[]>((resolve) => {
        const read = database.transaction('docs').objectStore('docs').getAll(IDBKeyRange.bound('sync:', 'sync:\uffff'));
        read.onsuccess = () => resolve(read.result);
      });
      database.close();
      return records[0].token;
    });
    expect(stored).toBe('DO-NOT-EXPORT-THIS-CREDENTIAL');
    await other.close();
  });

  test('debug reports identify a two-tab metadata conflict without disclosing either folder name', async ({ page }) => {
    await page.goto(`${baseURL}?flags=debug:sync`);
    await expect(page.locator('#emptyState')).toBeVisible();
    await page.locator('#importInput').setInputFiles({
      name: 'Private.zip',
      mimeType: 'application/zip',
      buffer: Buffer.from(zipSync({ 'Original confidential folder/Note.md': Buffer.from('# Private body') })),
    });
    await expect(page.locator('#toast')).toContainText('Imported 1 document');
    const other = await page.context().newPage();
    await other.goto(`${baseURL}?flags=debug:sync`);
    await expect(
      other.getByRole('button', { name: 'Actions for Original confidential folder', exact: true }),
    ).toBeVisible();
    let registrations = 0;
    await page.route('**/sync/document', async (route) => {
      if (++registrations === 1) {
        await other.bringToFront();
        await other.getByRole('button', { name: 'Actions for Original confidential folder', exact: true }).click();
        await other.getByRole('menuitem', { name: 'Rename', exact: true }).click();
        await other.getByRole('textbox', { name: 'Name', exact: true }).fill('Changed confidential folder');
        await other.getByRole('textbox', { name: 'Name', exact: true }).press('Enter');
        await expect(
          other.getByRole('button', { name: 'Actions for Changed confidential folder', exact: true }),
        ).toBeVisible();
        await page.bringToFront();
      }
      await route.continue();
    });
    await page.bringToFront();
    await page.getByRole('button', { name: 'Actions for Original confidential folder', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
    const failure = page.getByRole('dialog', { name: 'Sync did not finish', exact: true });
    await expect(failure.getByRole('alert')).toContainText('metadata-changed');
    await expect(failure.getByRole('alert')).toContainText('saving-connections');
    const raw = await page.evaluate(() =>
      (window as unknown as { marsDebug: { report(): string } }).marsDebug.report(),
    );
    const report = JSON.parse(raw).channels.sync;
    expect(report.events).toContainEqual(expect.objectContaining({ event: 'library.refresh', changed: 1 }));
    expect(report.events).toContainEqual(
      expect.objectContaining({ event: 'store.conflict', reason: 'metadata-changed', field: 'name' }),
    );
    expect(raw).not.toMatch(/Original confidential|Changed confidential|Private body/);
    await expect(page.locator('#tree [data-connection]')).toHaveCount(0);
    await other.close();
  });

  test('setup errors remain explicit when debug logging is disabled, including rejected initial uploads', async ({
    page,
  }) => {
    await openDoc(page, 'Upload diagnostics', '# Upload diagnostics\n\nKeep the saved document.');
    await page.route('**/sync/append', (route) => route.fulfill({ status: 507, json: { error: 'sync-storage-full' } }));
    await page.getByRole('button', { name: 'Actions for Upload diagnostics', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
    const failure = page.getByRole('dialog', { name: 'Sync did not finish', exact: true });
    await expect(failure.getByRole('alert')).toContainText('sync-storage-full');
    await expect(failure.getByRole('alert')).toContainText('uploading');
    await expect(failure.getByRole('button', { name: 'Copy debug report', exact: true })).toHaveCount(0);
    const report = await page.evaluate(
      () => JSON.parse((window as unknown as { marsDebug: { report(): string } }).marsDebug.report()).channels.sync,
    );
    expect(report.enabled).toBe(false);
    expect(report.events).toHaveLength(1);
    expect(report.events[0]).toMatchObject({
      event: 'setup.failed',
      code: 'sync-storage-full',
      stage: 'uploading',
      status: 507,
    });
    await failure.getByRole('button', { name: 'Close', exact: true }).click();
    await expect(page.locator('#previewArticle')).toContainText('Keep the saved document.');
    await page.unroute('**/sync/append');
    await page.getByRole('button', { name: 'Actions for Upload diagnostics', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    await page.getByRole('button', { name: 'Try again', exact: true }).click();
    await expect(page.locator('#saveState')).toContainText('Up to date', { timeout: 15000 });
  });

  test('folder setup is atomic across existing tabs and retains all cached document timestamps', async ({
    browser,
  }) => {
    test.setTimeout(120000);
    const c = await clients(browser, 1),
      [owner] = c.pages;
    const snapshot = (page: Page) =>
      page.evaluate(async () => {
        const database = await new Promise<IDBDatabase>((resolve) => {
          const open = indexedDB.open('mars');
          open.onsuccess = () => resolve(open.result);
        });
        try {
          const nodes = await new Promise<{ id: string; kind: string; updatedAt: number; connection?: unknown }[]>(
            (resolve) => {
              const read = database.transaction('nodes').objectStore('nodes').getAll();
              read.onsuccess = () => resolve(read.result);
            },
          );
          return {
            connections: nodes.filter((node) => node.connection).length,
            documents: nodes
              .filter((node) => node.kind === 'doc')
              .map((node) => ({ id: node.id, updatedAt: node.updatedAt }))
              .sort((a, b) => a.id.localeCompare(b.id)),
          };
        } finally {
          database.close();
        }
      });
    try {
      await owner.goto(baseURL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.locator('#importInput').setInputFiles({
        name: 'Library.zip',
        mimeType: 'application/zip',
        buffer: Buffer.from(
          zipSync(
            Object.fromEntries(
              Array.from({ length: 54 }, (_, index) => [
                `Work/Folder ${index % 7}/Document ${index}.md`,
                Buffer.from(`# Document ${index}\n\nOriginal text ${index}.`),
              ]),
            ),
          ),
        ),
      });
      await expect(owner.locator('#toast')).toContainText('Imported 54 documents');
      const other = await c.contexts[0].newPage();
      await other.goto(baseURL);
      await expect(other.locator('#sbUsage')).toContainText('54 documents');
      await owner.bringToFront();
      const before = await snapshot(owner);
      let release!: () => void;
      const held = new Promise<void>((resolve) => {
        release = resolve;
      });
      let registrations = 0;
      await owner.route('**/sync/document', async (route) => {
        if (++registrations === 12) await held;
        await route.continue();
      });
      await owner.getByRole('button', { name: 'Actions for Work', exact: true }).click();
      await owner.getByRole('menuitem', { name: 'Sync', exact: true }).click();
      await owner.getByRole('button', { name: 'Turn on sync', exact: true }).click();
      await expect.poll(() => registrations).toBe(12);
      expect(await snapshot(owner)).toEqual(before);
      expect(await snapshot(other)).toEqual(before);
      release();
      await expect(owner.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible({ timeout: 60000 });
      await expect.poll(async () => (await snapshot(other)).connections).toBe(62);
      expect((await snapshot(owner)).documents).toEqual(before.documents);
      expect((await snapshot(other)).documents).toEqual(before.documents);
      await expect(owner.locator('#toast')).not.toContainText(/Sync did not finish|Could not extend Sync/);
    } finally {
      await c.close();
    }
  });

  test('delete confirmation is simple for local documents, safe by default, and supports Undo', async ({ page }) => {
    await openDoc(page, 'Local note', '# Local note\n\nKeep this text.');
    await page.getByRole('button', { name: 'Actions for Local note', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Delete', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Delete "Local note"?' });
    await expect(dialog).toContainText('This removes the document from this browser.');
    await expect(dialog).not.toContainText(/Sync|Live|snapshot|collection/);
    await expect(dialog.getByRole('button', { name: 'Cancel', exact: true })).toBeFocused();
    await page.keyboard.press('Escape');
    await expect(page.locator('#tree .tree-link')).toHaveCount(1);
    await page.getByRole('button', { name: 'Actions for Local note', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Delete', exact: true }).click();
    await dialog.getByRole('button', { name: 'Delete document', exact: true }).click();
    await expect(page.locator('#tree .tree-link')).toHaveCount(0);
    await page.getByRole('button', { name: 'Undo', exact: true }).click();
    await expect(page.locator('#previewArticle')).toContainText('Keep this text.');
    await page.evaluate(() => {
      const original = IDBObjectStore.prototype.delete;
      IDBObjectStore.prototype.delete = function (key) {
        if (this.name === 'nodes') {
          IDBObjectStore.prototype.delete = original;
          throw new DOMException('Deletion blocked', 'UnknownError');
        }
        return original.call(this, key);
      };
    });
    await page.getByRole('button', { name: 'Actions for Local note', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Delete', exact: true }).click();
    await dialog.getByRole('button', { name: 'Delete document', exact: true }).click();
    await expect(page.locator('#toast')).toContainText('Could not delete');
    await expect(page.locator('#tree .tree-link')).toHaveCount(1);
    await page.reload();
    await expect(page.locator('#previewArticle')).toContainText('Keep this text.');
  });

  test('an incomplete folder from an older release can restart setup without losing local documents', async ({
    page,
  }) => {
    await page.goto(baseURL);
    await expect(page.locator('#emptyState')).toBeVisible();
    await page.locator('#importInput').setInputFiles({
      name: 'Partial.zip',
      mimeType: 'application/zip',
      buffer: Buffer.from(
        zipSync({
          'Partial/One.md': Buffer.from('# One\n\nRetain one.'),
          'Partial/Two.md': Buffer.from('# Two\n\nRetain two.'),
        }),
      ),
    });
    await expect(page.locator('#toast')).toContainText('Imported 2 documents');
    await page.getByRole('button', { name: 'Actions for Partial', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
    await expect(page.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'Close', exact: true }).click();
    await page.goto('about:blank');
    // Rewrite only this isolated test profile to the partial format of the old release.
    await page.goto(`${baseURL}?temporary=1`);
    const root = await page.evaluate(async () => {
      const database = await new Promise<IDBDatabase>((resolve) => {
        const open = indexedDB.open('mars');
        open.onsuccess = () => resolve(open.result);
      });
      const tx = database.transaction(['nodes', 'docs'], 'readwrite');
      const nodes = tx.objectStore('nodes').getAll();
      let root: { id: string; record: { body: string; outbox: unknown[] } };
      nodes.onsuccess = () => {
        for (const node of nodes.result) {
          if (node.kind === 'folder') {
            const read = tx.objectStore('docs').get(`sync:${node.id}`);
            read.onsuccess = () => {
              root = { id: node.id, record: read.result };
            };
          } else {
            delete node.connection;
            tx.objectStore('nodes').put(node);
            // Retain the orphan replica left behind by a stale tree write.
          }
        }
      };
      await new Promise<void>((resolve) => {
        tx.oncomplete = () => resolve();
      });
      database.close();
      return root!;
    });
    const { Doc, applyUpdate, encodeStateAsUpdate } = require('yjs');
    const replica = new Doc();
    applyUpdate(replica, Buffer.from(root.record.body, 'base64url'));
    replica.getMap('nodes').clear();
    root.record.body = Buffer.from(encodeStateAsUpdate(replica)).toString('base64url');
    root.record.outbox = [];
    replica.destroy();
    await page.evaluate(async ({ id, record }) => {
      const database = await new Promise<IDBDatabase>((resolve) => {
        const open = indexedDB.open('mars');
        open.onsuccess = () => resolve(open.result);
      });
      const tx = database.transaction('docs', 'readwrite');
      tx.objectStore('docs').put(record, `sync:${id}`);
      await new Promise<void>((resolve) => {
        tx.oncomplete = () => resolve();
      });
      database.close();
    }, root);
    await page.route('**/sync/index', (route) => route.fulfill({ json: { entries: [], complete: true } }));
    await page.goto(baseURL);
    await page.getByRole('button', { name: 'Actions for Partial', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    await expect(page.getByRole('dialog', { name: 'Finish folder setup', exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'Restart setup', exact: true }).click();
    await expect(page.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible();
    await expect(page.locator('#tree [data-connection]')).toHaveCount(3);
    await expect(page.locator('#sbUsage')).toContainText('2 documents');
  });

  test('ended Sync copies delete locally without offering an unavailable collection action', async ({ page }) => {
    await openDoc(page, 'Sync Test', '# Sync Test\n\nRetained copy.');
    await page.getByRole('button', { name: 'Actions for Sync Test', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
    await expect(page.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'Close', exact: true }).click();
    await page.route('**/sync/index', (route) => route.fulfill({ status: 410, json: { error: 'connection-expired' } }));
    await expect(page.locator('#saveState')).toContainText('connection ended', { timeout: 15000 });
    await page.getByRole('button', { name: 'Actions for Sync Test', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Delete', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Delete "Sync Test"?' });
    await expect(dialog).toContainText('The connection has ended');
    await expect(dialog.getByRole('button', { name: 'Delete from collection', exact: true })).toHaveCount(0);
    await dialog.getByRole('button', { name: 'Delete document', exact: true }).click();
    await expect(page.locator('#tree .tree-link')).toHaveCount(0);
    await page.reload();
    await expect(page.locator('#emptyState')).toBeVisible();
  });

  test('delete explains active Live and snapshot counts without deleting shared snapshots', async ({ page }) => {
    await openDoc(page, 'Shared note', '# Shared note\n\nA shared snapshot.');
    const { copyLink } = await import('./_mars_sharing');
    await copyLink(page, { format: 'short' });
    await live(page);
    await page.getByRole('button', { name: 'Actions for Shared note', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Delete', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Delete "Shared note"?' });
    await expect(dialog).toContainText('1 active Live link');
    await expect(dialog).toContainText('1 active short snapshot link');
    await expect(dialog).toContainText('Deleting from the collection ends Live access');
    await dialog.getByRole('button', { name: 'Delete local copy', exact: true }).click();
    await expect(page.locator('#tree .tree-link')).toHaveCount(0);
    expect(await page.evaluate(() => Object.keys(JSON.parse(localStorage.getItem('mars.shortLinks')!)).length)).toBe(1);
  });

  test('failed remote deletion reports the problem and a retry handles already-ended access', async ({ page }) => {
    await openDoc(page, 'Retry delete', '# Retry delete\n\nNever lose a failed deletion.');
    await live(page);
    let requests = 0;
    await page.route('**/sync/delete-document', (route) =>
      route.fulfill({
        status: ++requests === 1 ? 503 : 410,
        json: { error: requests === 1 ? 'service-unavailable' : 'connection-expired' },
      }),
    );
    const requestDelete = async () => {
      await page.getByRole('button', { name: 'Actions for Retry delete', exact: true }).click();
      await page.getByRole('menuitem', { name: 'Delete', exact: true }).click();
      await page.getByRole('button', { name: 'Delete from collection', exact: true }).click();
    };
    await requestDelete();
    await expect(page.locator('#toast')).toContainText('Could not delete');
    await expect(page.locator('#tree .tree-link')).toHaveCount(1);
    await requestDelete();
    await expect(page.locator('#tree .tree-link')).toHaveCount(0);
  });

  for (const permission of ['view', 'comment', 'edit'] as const)
    test(`opening a ${permission} Live document with an empty retained thread does not publish deletion`, async ({
      browser,
    }) => {
      test.setTimeout(90000);
      const c = await clients(browser),
        [owner, guest] = c.pages,
        published: string[] = [];
      guest.on('request', (request) => {
        if (new URL(request.url()).pathname === '/sync/append') published.push(request.url());
      });
      try {
        await openDoc(
          owner,
          'Retained',
          '# Retained\n\nValue\n\n<!-- su-comments\n[{"id":"legacy-thread","quote":"Value","comments":[{"id":"legacy-comment","author":"Owner","body":"Original comment"}]}]\n-->\n',
        );
        const link = await live(owner, permission);
        await owner.reload();
        await expect(owner.locator('#saveState')).toHaveText('Up to date', { timeout: 15000 });
        await expect(owner.locator('.cm-comment').getByRole('button', { name: 'Delete', exact: true })).toBeVisible({
          timeout: 15000,
        });
        await owner.locator('.cm-comment').getByRole('button', { name: 'Delete', exact: true }).click();
        await expect(owner.locator('.cm-comment')).toHaveCount(0);
        await expect(owner.locator('#saveState')).toHaveText('Up to date', { timeout: 30000 });
        const address = new URL(link);
        address.searchParams.set('temporary', '1');
        await guest.goto(`${baseURL}?temporary=1`);
        await expect(guest.locator('#emptyState')).toBeVisible();
        await guest.waitForFunction(() => !!(window as { __suEditorAPI?: unknown }).__suEditorAPI);
        await guest.goto(address.href);
        await expect(guest.locator('#previewArticle')).toContainText('Value', { timeout: 15000 });
        await expect(guest.locator('#saveState')).toHaveText('Up to date', { timeout: 15000 });
        await expect(guest.locator('#toast')).not.toContainText(/thread-owner-required|reviews-read-only/);
        await guest.reload();
        await expect(guest.locator('#saveState')).toHaveText('Up to date', { timeout: 15000 });
        await guest.getByRole('button', { name: 'Save live document to library', exact: true }).click();
        await guest.getByRole('button', { name: 'Save to library', exact: true }).click();
        await expect(guest.locator('#temporarySession')).toBeHidden();
        await guest.reload();
        await expect(guest.locator('#saveState')).toHaveText('Up to date', { timeout: 15000 });
        expect(published).toEqual([]);
        await owner.getByRole('button', { name: 'Actions for Retained', exact: true }).click();
        await owner.getByRole('menuitem', { name: 'Download', exact: true }).click();
        const pending = owner.waitForEvent('download');
        await owner
          .getByRole('dialog', { name: 'Download', exact: true })
          .getByRole('button', { name: 'Download', exact: true })
          .click();
        const download = await pending,
          text = await readFile((await download.path())!, 'utf8');
        const threads = JSON.parse(text.match(/<!-- su-comments\n([\s\S]*?)\n-->/)![1]);
        expect(threads).toEqual([{ id: 'legacy-thread', quote: 'Value', comments: [] }]);
        if (permission === 'comment') {
          await guest.getByRole('textbox', { name: 'Reply', exact: true }).fill('New permitted reply');
          await guest.getByRole('button', { name: 'Reply', exact: true }).click();
          await expect(guest.locator('#saveState')).toHaveText('Up to date', { timeout: 15000 });
          await owner.bringToFront();
          await expect(owner.locator('#commentsRail')).toContainText('New permitted reply', { timeout: 30000 });
          await expect(guest.locator('#toast')).not.toContainText('thread-owner-required');
        }
      } finally {
        await c.close();
      }
    });

  test('alternating Live comments stay 1, 2, 3, 4 after each sync, editing, reload, and download', async ({
    browser,
  }) => {
    test.setTimeout(120000);
    const c = await clients(browser),
      [owner, guest] = c.pages;
    try {
      await openDoc(owner, 'Order', '# Order\n\nValue');
      await guest.goto(await live(owner, 'comment'));
      await expect(guest.locator('#previewArticle')).toContainText('Value', { timeout: 15000 });
      await expect(guest.locator('#documentAccess')).toHaveText('Can comment');
      for (const page of [owner, guest])
        await page.evaluate(() => {
          const original = crypto.randomUUID.bind(crypto);
          Object.defineProperty(crypto, 'randomUUID', {
            configurable: true,
            value: () => {
              const value = original(),
                prefix = (window as unknown as { __reviewPrefix?: string }).__reviewPrefix;
              return prefix ? `${prefix}${value.slice(8)}` : value;
            },
          });
        });
      const bodies = (page: Page) => page.locator('#commentsRail .cm-comment .cm-body');
      const expected: string[] = [];
      for (const [index, sender] of [owner, guest, owner, guest].entries()) {
        const receiver = sender === owner ? guest : owner;
        await sender.bringToFront();
        await sender.evaluate((prefix) => {
          (window as unknown as { __reviewPrefix: string }).__reviewPrefix = prefix;
        }, ['11111111', '22222222', 'eeeeeeee', '33333333'][index]);
        if (!index) {
          await owner
            .locator('#previewArticle p')
            .filter({ hasText: /^Value$/ })
            .evaluate((paragraph) => {
              const range = document.createRange();
              range.selectNodeContents(paragraph);
              const selection = getSelection()!;
              selection.removeAllRanges();
              selection.addRange(range);
            });
          await owner.locator('#formatBar').getByRole('button', { name: 'Comment', exact: true }).click();
          const draft = owner.locator('.cm-card.draft');
          await draft.getByLabel('Your name', { exact: true }).fill('A');
          await draft.getByRole('textbox', { name: 'Comment', exact: true }).fill('1');
          await draft.getByRole('button', { name: 'Comment', exact: true }).click();
        } else {
          await sender.getByRole('textbox', { name: 'Reply', exact: true }).fill(String(index + 1));
          await sender.getByLabel('Your name', { exact: true }).fill(index % 2 ? 'B' : 'A');
          await sender.getByRole('button', { name: 'Reply', exact: true }).click();
        }
        expected.push(String(index + 1));
        await expect(bodies(sender)).toHaveText(expected, { timeout: 30000 });
        await receiver.bringToFront();
        await expect(bodies(receiver)).toHaveText(expected, { timeout: 30000 });
      }
      await expect(owner.locator('.cm-comment .cm-name')).toHaveText(['A', 'B', 'A', 'B']);
      await owner.screenshot({ path: test.info().outputPath('comment-order.png') });
      await owner.bringToFront();
      const third = owner.locator('.cm-comment').filter({ has: owner.getByText('3', { exact: true }) });
      await third.getByRole('button', { name: 'Edit', exact: true }).click();
      await third.getByRole('textbox', { name: 'Save', exact: true }).fill('3 edited');
      await third.getByRole('button', { name: 'Save', exact: true }).click();
      expected[2] = '3 edited';
      await expect(bodies(owner)).toHaveText(expected, { timeout: 30000 });
      await guest.bringToFront();
      await expect(bodies(guest)).toHaveText(expected, { timeout: 30000 });
      for (const page of [owner, guest]) {
        await page.bringToFront();
        await page.reload();
        await expect(bodies(page)).toHaveText(expected, { timeout: 30000 });
      }
      await owner.bringToFront();
      await owner.getByRole('button', { name: 'Actions for Order', exact: true }).click();
      await owner.getByRole('menuitem', { name: 'Download', exact: true }).click();
      const pending = owner.waitForEvent('download');
      await owner
        .getByRole('dialog', { name: 'Download', exact: true })
        .getByRole('button', { name: 'Download', exact: true })
        .click();
      const download = await pending,
        text = await readFile((await download.path())!, 'utf8');
      const threads = JSON.parse(text.match(/<!-- su-comments\n([\s\S]*?)\n-->/)![1]) as {
        comments: { body: string }[];
      }[];
      expect(threads[0].comments.map((comment) => comment.body)).toEqual(expected);
    } finally {
      await c.close();
    }
  });

  test('a Live editor can undo comment and thread deletion without becoming their author', async ({ browser }) => {
    test.setTimeout(90000);
    const c = await clients(browser),
      [owner, guest] = c.pages;
    try {
      await openDoc(
        owner,
        'Review',
        '# Review\n\nShared body.\n\n<!-- su-comments\n[{"id":"thread","quote":"Shared","comments":[{"id":"original","author":"Original author","body":"Original review text"}]}]\n-->\n',
      );
      await guest.goto(await live(owner, 'edit'));
      await expect(guest.locator('#commentsRail')).toContainText('Original review text', { timeout: 15000 });
      const comment = guest.locator('.cm-comment').filter({ hasText: 'Original review text' });
      await comment.getByRole('button', { name: 'Delete', exact: true }).click();
      await guest.getByRole('button', { name: 'Undo', exact: true }).click();
      await expect(comment).toContainText('Original author');
      await expect(owner.locator('#commentsRail')).toContainText('Original review text', { timeout: 15000 });
      await expect(guest.locator('#toast')).not.toContainText('comment-author-mismatch');
      await guest.getByRole('button', { name: 'Delete thread', exact: true }).click();
      await guest.getByRole('button', { name: 'Undo', exact: true }).click();
      await expect(comment).toContainText('Original author');
      await guest.reload();
      await expect(guest.locator('#commentsRail')).toContainText('Original review text', { timeout: 15000 });
    } finally {
      await c.close();
    }
  });

  test('disabling the final Live-only link deletes the remote copy and shows recipients as disconnected', async ({
    browser,
  }) => {
    test.setTimeout(120000);
    const c = await clients(browser, 3),
      [owner, first, second] = c.pages;
    try {
      await openDoc(owner, 'Shared', '# Shared\n\nRetain this local content.');
      const a = await live(owner),
        b = await live(owner);
      await first.goto(a);
      await second.goto(b);
      await expect(first.locator('#previewArticle')).toContainText('Retain this local content.', { timeout: 15000 });
      await expect(second.locator('#previewArticle')).toContainText('Retain this local content.', { timeout: 15000 });
      await second.getByRole('button', { name: 'Save live document to library', exact: true }).click();
      await second.getByRole('button', { name: 'Save to library', exact: true }).click();
      await expect(second.locator('#temporarySession')).toBeHidden();
      await connections(owner, 'Shared');
      await expect(owner.getByRole('button', { name: 'Stop sync', exact: true })).toHaveCount(0);
      await expect(owner.getByRole('button', { name: 'Stop sharing', exact: true })).toBeVisible();
      await owner.getByRole('button', { name: 'Disable link 1', exact: true }).click();
      await owner.getByRole('button', { name: 'Disable link', exact: true }).click();
      await expect(owner.getByRole('dialog', { name: 'Live links', exact: true })).toBeVisible({ timeout: 30000 });
      await first.bringToFront();
      await expect(first.locator('#tree [data-phase="ended"]')).toHaveCount(1, { timeout: 30000 });
      await expect(first.locator('#saveState')).toContainText('Local copy');
      await expect(first.locator('#temporaryHelp')).toContainText('Reload cannot restore it');
      await expect(second.locator('#saveState')).not.toContainText('connection ended');
      await owner.bringToFront();
      await owner.getByRole('button', { name: /^Disable link \d/ }).click();
      await owner.getByRole('button', { name: 'Disable link', exact: true }).click();
      await expect(owner.locator('#tree [data-connection]')).toHaveCount(0, { timeout: 30000 });
      await second.bringToFront();
      await expect(second.locator('#tree [data-phase="ended"]')).toHaveCount(1, { timeout: 30000 });
      await expect(second.locator('#temporaryHelp')).toContainText('Your saved copy');
      await second.getByRole('button', { name: 'Actions for Shared', exact: true }).click();
      const localDownload = second.waitForEvent('download');
      await second.getByRole('menuitem', { name: 'Download', exact: true }).click();
      expect((await localDownload).suggestedFilename()).toBe('Shared.md');
      await expect(second.locator('#downloadDialog')).toHaveCount(0);
      await expect(owner.locator('#previewArticle')).toContainText('Retain this local content.');
      await expect(owner.locator('#liveLinksDialog')).toHaveCount(0);
      await owner.reload();
      await expect(owner.locator('#saveState')).toHaveText('Saved');
      await second.reload();
      await expect(second.locator('#previewArticle')).toContainText('Retain this local content.', { timeout: 15000 });
      await expect(second.locator('#tree [data-phase="ended"]')).toHaveCount(1, { timeout: 15000 });
      await first.reload();
      await expect(first.locator('#tree [data-phase="ended"]')).toHaveCount(0);
      // A temporary recipient cannot reopen a revoked link, but its already open copy was retained.
      await expect(first.locator('#toast')).toContainText('Could not open the live link', { timeout: 15000 });
      await owner.bringToFront();
      await live(owner);
      await live(owner);
      await connections(owner, 'Shared');
      await owner.route('**/sync/purge-live-document', (route) =>
        route.fulfill({
          status: 507,
          contentType: 'application/json',
          body: JSON.stringify({ error: 'sync-storage-full' }),
        }),
      );
      await owner.getByRole('button', { name: 'Stop sharing', exact: true }).click();
      await owner.getByRole('button', { name: 'Stop sharing', exact: true }).click();
      await expect(owner.getByRole('button', { name: 'Try again', exact: true })).toBeVisible({ timeout: 30000 });
      await owner.getByRole('button', { name: 'Close', exact: true }).click();
      await connections(owner, 'Shared');
      await expect(owner.getByRole('button', { name: 'Try again', exact: true })).toBeVisible();
      await expect(owner.locator('#previewArticle')).toContainText('Retain this local content.');
      await owner.unroute('**/sync/purge-live-document');
      await owner.getByRole('button', { name: 'Try again', exact: true }).click();
      await expect(owner.locator('#tree [data-connection]')).toHaveCount(0, { timeout: 30000 });
    } finally {
      await c.close();
    }
  });

  test('Download and Export all retain normal output and optionally restore encrypted connections with local documents', async ({
    browser,
  }) => {
    test.setTimeout(120000);
    const c = await clients(browser),
      [owner, restored] = c.pages;
    try {
      await openDoc(owner, 'Private', '# Private\n\nA normal local document.');
      await owner.getByRole('button', { name: 'Actions for Private', exact: true }).click();
      const ordinary = owner.waitForEvent('download');
      await owner.getByRole('menuitem', { name: 'Download', exact: true }).click();
      expect((await ordinary).suggestedFilename()).toBe('Private.md');
      await expect(owner.locator('#downloadDialog')).toHaveCount(0);
      await openDoc(owner, 'Shared', '# Shared\n\nSaved connected content.');
      await live(owner);
      await owner.getByRole('button', { name: 'Actions for Shared', exact: true }).click();
      await owner.getByRole('menuitem', { name: 'Download', exact: true }).click();
      await expect(
        owner.getByRole('checkbox', { name: 'Include connections and pending changes', exact: true }),
      ).not.toBeChecked();
      const plain = owner.waitForEvent('download');
      await owner.locator('#downloadDialog').getByRole('button', { name: 'Download', exact: true }).click();
      expect((await plain).suggestedFilename()).toBe('Shared.md');
      await owner.getByRole('button', { name: 'Export all', exact: true }).click();
      await owner.getByRole('checkbox', { name: 'Include connections and pending changes', exact: true }).check();
      const explanation = owner.locator('#downloadDialog .download-documents-only');
      await expect(explanation).toHaveCSS('margin-top', '16px');
      const gap = await explanation.evaluate(
        (element) =>
          element.getBoundingClientRect().top - element.previousElementSibling!.getBoundingClientRect().bottom,
      );
      expect(gap).toBeGreaterThanOrEqual(15);
      await owner.getByRole('textbox', { name: 'Password', exact: true }).fill('short');
      await owner.locator('#downloadDialog').getByRole('button', { name: 'Download', exact: true }).click();
      await expect(owner.locator('#downloadDialog')).toContainText('Use at least 12 characters');
      await owner.getByRole('textbox', { name: 'Password', exact: true }).fill('separate recovery password');
      const pending = owner.waitForEvent('download');
      await owner.locator('#downloadDialog').getByRole('button', { name: 'Download', exact: true }).click();
      const file = await pending,
        bytes = await readFile((await file.path())!),
        entries = unzipSync(bytes);
      expect(Buffer.from(entries['Private.md']).toString()).toContain('A normal local document.');
      expect(Buffer.from(entries['Shared.md']).toString()).toContain('Saved connected content.');
      const backup = Buffer.from(entries['MARS connections/mars-encrypted-recovery.json']).toString();
      expect(backup).not.toContain('Saved connected content.');
      expect(backup).not.toContain('signing');
      await owner.goto('about:blank');
      await openDoc(restored, 'Existing', '# Existing\n\nDo not overwrite this library.');
      await restored
        .locator('#importInput')
        .setInputFiles({ name: file.suggestedFilename(), mimeType: 'application/zip', buffer: bytes });
      await restored.getByRole('button', { name: 'Restore connections', exact: true }).click();
      await restored.getByRole('textbox', { name: 'Backup password', exact: true }).fill('wrong recovery password');
      await restored.getByRole('button', { name: 'Restore', exact: true }).click();
      await expect(restored.locator('#toast')).toContainText('Your library has not changed');
      await expect(restored.locator('#tree .tree-link')).toHaveCount(1);
      await restored
        .locator('#importInput')
        .setInputFiles({ name: file.suggestedFilename(), mimeType: 'application/zip', buffer: bytes });
      await restored.getByRole('button', { name: 'Restore connections', exact: true }).click();
      await restored.getByRole('textbox', { name: 'Backup password', exact: true }).fill('separate recovery password');
      await restored.getByRole('button', { name: 'Restore', exact: true }).click();
      await expect(restored.locator('#toast')).toContainText('Backup restored', { timeout: 15000 });
      await expect(restored.locator('#tree .tree-link')).toHaveCount(3);
      await restored
        .locator('#tree .tree-link')
        .filter({ hasText: /^Shared$/ })
        .click();
      await expect(restored.locator('#previewArticle')).toContainText('Saved connected content.');
      await expect(restored.locator('#saveState')).toContainText('Up to date', { timeout: 15000 });
      await restored
        .locator('#tree .tree-link')
        .filter({ hasText: /^Private$/ })
        .click();
      await expect(restored.locator('#previewArticle')).toContainText('A normal local document.');
      await restored
        .locator('#tree .tree-link')
        .filter({ hasText: /^Existing$/ })
        .click();
      await expect(restored.locator('#previewArticle')).toContainText('Do not overwrite this library.');
    } finally {
      await c.close();
    }
  });

  test('a document download recovers only that document, not the rest of its synced folder', async ({ browser }) => {
    test.setTimeout(90000);
    const c = await clients(browser),
      [owner, restored] = c.pages;
    try {
      await owner.goto(baseURL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      const bytes = zipSync({
        'Folder/Selected.md': Buffer.from('# Selected\n\nOnly this document.'),
        'Folder/Outside.md': Buffer.from('# Outside\n\nUnselected private content.'),
      });
      await owner
        .locator('#importInput')
        .setInputFiles({ name: 'Folder.zip', mimeType: 'application/zip', buffer: Buffer.from(bytes) });
      await expect(owner.locator('#tree')).toContainText('Outside');
      await owner.getByRole('button', { name: 'Actions for Folder', exact: true }).click();
      await owner.getByRole('menuitem', { name: 'Sync', exact: true }).click();
      await owner.getByRole('button', { name: 'Turn on sync', exact: true }).click();
      await expect(owner.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible({ timeout: 15000 });
      await owner.getByRole('button', { name: 'Close', exact: true }).click();
      await owner
        .locator('#tree .tree-link')
        .filter({ hasText: /^Selected$/ })
        .click();
      await owner.getByRole('button', { name: 'Actions for Selected', exact: true }).click();
      await owner.getByRole('menuitem', { name: 'Download', exact: true }).click();
      await owner.getByRole('checkbox', { name: 'Include connections and pending changes', exact: true }).check();
      await owner.getByRole('textbox', { name: 'Password', exact: true }).fill('separate recovery password');
      const pending = owner.waitForEvent('download');
      await owner.locator('#downloadDialog').getByRole('button', { name: 'Download', exact: true }).click();
      const file = await pending,
        archive = await readFile((await file.path())!);
      expect(Object.keys(unzipSync(archive)).sort()).toEqual([
        'MARS connections/mars-encrypted-recovery.json',
        'Selected.md',
      ]);
      await owner.goto('about:blank');
      await restored.goto(baseURL);
      await expect(restored.locator('#emptyState')).toBeVisible();
      await restored
        .locator('#importInput')
        .setInputFiles({ name: file.suggestedFilename(), mimeType: 'application/zip', buffer: archive });
      await restored.getByRole('button', { name: 'Restore connections', exact: true }).click();
      await restored.getByRole('textbox', { name: 'Backup password', exact: true }).fill('separate recovery password');
      await restored.getByRole('button', { name: 'Restore', exact: true }).click();
      await expect(restored.locator('#toast')).toContainText('Backup restored', { timeout: 15000 });
      await expect(restored.locator('#tree .tree-link')).toHaveCount(1);
      await expect(restored.locator('#tree')).not.toContainText('Outside');
      await restored.locator('#tree .tree-link').click();
      await expect(restored.locator('#previewArticle')).toContainText('Only this document.');
      await expect(restored.locator('#saveState')).toContainText('Up to date', { timeout: 15000 });
    } finally {
      await c.close();
    }
  });
});
