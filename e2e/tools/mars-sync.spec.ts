import { createRequire } from 'node:module';
import { resolve } from 'node:path';

import { type Browser, expect, type Page, test } from '@playwright/test';

import { mockWorker as mockSmartSearchWorker } from './_mars_indexing';

// Responder-only synchronization and live-session browser coverage.
const MARS_BASE_URL = process.env.MARS_BASE_URL ?? 'https://mars.secutils.dev';
test.use({ baseURL: MARS_BASE_URL });
const require = createRequire(resolve(process.cwd(), '../dev/tools/js/mars/package.json'));
const { zipSync } = require('fflate') as { zipSync(files: Record<string, Uint8Array>): Uint8Array };
const { build } = require('esbuild') as {
  build(options: Record<string, unknown>): Promise<{ outputFiles: { text: string }[] }>;
};
let safetyBundle = '';
let liveBundle = '';
let syncBundle = '';
test.beforeAll(async () => {
  const result = await build({
    entryPoints: [resolve(process.cwd(), '../dev/tools/js/mars/src/sync/preview-safety.ts')],
    bundle: true,
    format: 'iife',
    globalName: 'MARS_SYNC_SAFETY',
    write: false,
  });
  safetyBundle = result.outputFiles[0].text;
  liveBundle = (
    await build({
      entryPoints: [resolve(process.cwd(), '../dev/tools/js/mars/src/sync/live-link.ts')],
      bundle: true,
      format: 'iife',
      globalName: 'MARS_LIVE_LINK',
      write: false,
    })
  ).outputFiles[0].text;
  syncBundle = (
    await build({
      entryPoints: [resolve(process.cwd(), '../dev/tools/js/mars/src/sync/sync-link.ts')],
      bundle: true,
      format: 'iife',
      globalName: 'MARS_SYNC_LINK',
      write: false,
    })
  ).outputFiles[0].text;
});

async function pair(owner: Page, peer: Page, name: string, browserName?: string, legacy = false) {
  if (!(await owner.locator('#syncDialog').count())) {
    await owner.getByRole('button', { name: `Actions for ${name}`, exact: true }).click();
    await owner.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
    await owner.getByRole('button', { name: 'Sync link', exact: true }).click();
  }
  await expect(owner.getByRole('textbox', { name: 'Sync link' })).toBeVisible({ timeout: 15000 });
  const url = await owner.getByRole('textbox', { name: 'Sync link' }).inputValue();
  expect(new URL(url).hash).toMatch(/^#sync=[A-Za-z0-9_-]{22}$/);
  await owner.getByRole('button', { name: 'Copy link', exact: true }).click();
  let address = url;
  if (legacy) {
    await owner.addScriptTag({ content: syncBundle });
    address = await owner.evaluate(async (url) => {
      const api = (
        window as unknown as {
          MARS_SYNC_LINK: {
            syncAccess(secret: string): Promise<{ id: string; token: string }>;
            openSyncBootstrap(value: unknown, secret: string): Promise<unknown>;
          };
        }
      ).MARS_SYNC_LINK;
      const secret = new URL(url).hash.slice(6),
        access = await api.syncAccess(secret);
      const response = await fetch('/sync/invite-read', {
        method: 'POST',
        headers: { Authorization: `Bearer ${access.token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ invitation: access.id }),
      });
      const data = await response.json(),
        payload = await api.openSyncBootstrap(data.bootstrap, secret),
        bytes = new TextEncoder().encode(JSON.stringify(payload));
      const compressed = new Uint8Array(
          await new Response(
            new Blob([bytes]).stream().pipeThrough(new CompressionStream('deflate-raw')),
          ).arrayBuffer(),
        ),
        out = new Uint8Array(4 + compressed.length);
      new DataView(out.buffer).setUint32(0, bytes.length, true);
      out.set(compressed, 4);
      return `${location.origin}/#invite=${btoa(String.fromCharCode(...out))
        .replace(/\+/g, '-')
        .replace(/\//g, '_')
        .replace(/=+$/, '')}`;
    }, url);
  }
  await peer.goto(address);
  if (browserName) await peer.getByRole('textbox', { name: 'Name', exact: true }).fill(browserName);
  await peer.getByRole('button', { name: 'Connect', exact: true }).click();
  await expect(owner.getByRole('button', { name: 'Codes match', exact: true })).toBeVisible({ timeout: 15000 });
  const ownerCode = owner.getByRole('group', { name: 'Confirmation code', exact: true });
  const peerCode = peer.getByRole('group', { name: 'Confirmation code', exact: true });
  expect(await ownerCode.locator('span').allTextContents()).toEqual(await peerCode.locator('span').allTextContents());
  for (const code of [ownerCode, peerCode]) {
    const boxes = await code
      .locator('span')
      .evaluateAll((elements) => elements.map((element) => element.getBoundingClientRect().toJSON()));
    expect(Math.max(...boxes.map((box) => box.y)) - Math.min(...boxes.map((box) => box.y))).toBeLessThanOrEqual(1);
    expect(Math.max(...boxes.map((box) => box.width)) - Math.min(...boxes.map((box) => box.width))).toBeLessThanOrEqual(
      1,
    );
  }
  await owner.getByRole('button', { name: 'Codes match', exact: true }).click({ timeout: 15000 });
  await expect(owner.getByRole('dialog', { name: /^(Sync|Connections)$/, exact: true })).toBeVisible({
    timeout: 15000,
  });
  await owner.getByRole('button', { name: 'Close', exact: true }).click();
}
async function isolatedContext(browser: Browser, headers: Record<string, string>) {
  const context = await browser.newContext();
  const origin = new URL(MARS_BASE_URL).origin;
  await context.route(`${origin}/**`, (route) =>
    route.continue({ headers: { ...route.request().headers(), ...headers } }),
  );
  return context;
}
async function syncedDocument(page: Page, name: string, text: string) {
  await page.goto(MARS_BASE_URL);
  await expect(page.locator('#emptyState')).toBeVisible();
  await page
    .locator('#fileInput')
    .setInputFiles({ name: `${name}.md`, mimeType: 'text/markdown', buffer: Buffer.from(text) });
  await expect(page.locator('#docTitle')).toHaveText(name);
  await page.getByRole('button', { name: `Actions for ${name}`, exact: true }).click();
  await page.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
  await page
    .getByRole('dialog', { name: /^(Sync|Connections)$/, exact: true })
    .getByRole('button', { name: 'Turn on sync', exact: true })
    .click();
  await expect(page.locator('#saveState')).toContainText('Up to date', { timeout: 15000 });
  await expect(page.getByRole('textbox', { name: 'Sync link' })).toBeVisible();
  await page.getByRole('button', { name: 'Close', exact: true }).click();
}
async function sourceText(page: Page, text: string, action: 'replace' | 'prepend' | 'append' = 'replace') {
  await page.getByRole('button', { name: 'Source', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Source', exact: true })).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByRole('textbox', { name: 'Markdown source', exact: true })).toBeVisible();
  await page.evaluate(
    ({ text, action }) => {
      const editor = (window as unknown as { __suEditorAPI: { setText(text: string): void; getText(): string } })
        .__suEditorAPI;
      editor.setText(
        action === 'prepend' ? text + editor.getText() : action === 'append' ? editor.getText() + text : text,
      );
    },
    { text, action },
  );
  await page.getByRole('button', { name: 'Preview', exact: true }).click();
}

test.describe('sync protocol', () => {
  test('previously issued pairing fragments still connect through the Sync link flow', async ({ browser }) => {
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const ownerContext = await isolatedContext(browser, headers),
      peerContext = await isolatedContext(browser, headers);
    const owner = await ownerContext.newPage(),
      peer = await peerContext.newPage();
    try {
      await syncedDocument(owner, 'Legacy link', '# Legacy link\n\nAn existing pairing link.');
      await pair(owner, peer, 'Legacy link', undefined, true);
      await expect(peer.locator('#previewArticle')).toContainText('An existing pairing link.', { timeout: 15000 });
    } finally {
      await owner.goto('about:blank').catch(() => {});
      await peer.goto('about:blank').catch(() => {});
      await ownerContext.close();
      await peerContext.close();
    }
  });
  test('sync enrollment and acknowledgements retain the existing smart-search index', async ({ browser }) => {
    test.setTimeout(90000);
    const context = await isolatedContext(browser, { 'x-mars-test-client': crypto.randomUUID() });
    await mockSmartSearchWorker(context);
    const owner = await context.newPage();
    const calls = (page: Page) =>
      page.evaluate(() => (window as unknown as { __marsWorker: { embeds: string[][] } }).__marsWorker.embeds.length);
    const snapshot = (page: Page) =>
      page.evaluate(async () => {
        const database = await new Promise<IDBDatabase>((resolve, reject) => {
          const open = indexedDB.open('mars');
          open.onsuccess = () => resolve(open.result);
          open.onerror = () => reject(open.error);
        });
        try {
          const transaction = database.transaction(['nodes', 'docs'], 'readonly');
          const nodes = transaction.objectStore('nodes').getAll();
          const vectors = transaction.objectStore('docs').getAll(IDBKeyRange.bound('vec:', 'vec:\uffff'));
          await new Promise<void>((resolve, reject) => {
            transaction.oncomplete = () => resolve();
            transaction.onerror = () => reject(transaction.error);
          });
          return nodes.result
            .filter((node) => node.kind === 'doc')
            .map((node) => ({
              id: node.id,
              name: node.name,
              updatedAt: node.updatedAt,
              vectorAt: vectors.result.find((entry) => entry.id === node.id)?.at,
            }))
            .sort((a, b) => a.name.localeCompare(b.name));
        } finally {
          database.close();
        }
      });
    try {
      await owner.goto(MARS_BASE_URL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.getByRole('button', { name: 'New document, folder, or import', exact: true }).click();
      await owner.getByRole('menuitem', { name: 'New folder', exact: true }).click();
      await owner.getByRole('textbox', { name: 'Name', exact: true }).fill('Indexed folder');
      await owner.getByRole('textbox', { name: 'Name', exact: true }).press('Enter');
      const files = Object.fromEntries(
        Array.from({ length: 3 }, (_, index) => [
          `Cached ${index}.md`,
          Buffer.from(`# Cached ${index}\n\nA cached document with enough searchable words ${index}.`),
        ]),
      );
      await owner
        .locator('#importInput')
        .setInputFiles({ name: 'Indexed.zip', mimeType: 'application/zip', buffer: Buffer.from(zipSync(files)) });
      await expect(owner.locator('#toast')).toContainText('Imported 3 documents');
      await owner.getByRole('button', { name: 'Smart search', exact: true }).click();
      await owner.getByRole('radio', { name: /Best speed/ }).check();
      await owner.locator('#marsModalFoot').getByRole('button', { name: 'Turn on', exact: true }).click();
      await expect.poll(() => calls(owner)).toBe(3);
      await expect
        .poll(async () => (await snapshot(owner)).every((node) => node.vectorAt === node.updatedAt))
        .toBe(true);
      const before = await snapshot(owner);
      await owner.evaluate(() => {
        (window as unknown as { __marsWorker: { embeds: string[][] } }).__marsWorker.embeds.length = 0;
      });
      await owner.getByRole('button', { name: 'Actions for Indexed folder', exact: true }).click();
      await owner.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
      await owner.getByRole('button', { name: 'Turn on sync', exact: true }).click();
      await expect(owner.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible({
        timeout: 15000,
      });
      await owner.getByRole('button', { name: 'Close', exact: true }).click();
      const other = await context.newPage();
      await other.goto(MARS_BASE_URL);
      await expect(other.locator('#tree .tree-label').getByText('Cached 2', { exact: true })).toBeVisible();
      await owner.bringToFront();
      await expect(other.locator('#tree [data-connection]')).toHaveCount(4);
      await owner.waitForTimeout(3500); // Past both startup and change indexing delays.
      expect(await calls(owner)).toBe(0);
      expect(await calls(other)).toBe(0);
      expect(await snapshot(owner)).toEqual(before);
      await owner.locator('#tree .tree-link').filter({ hasText: 'Cached 1' }).click();
      await sourceText(owner, '# Cached 1\n\nOnly this document changed its searchable content.');
      await expect.poll(() => calls(owner)).toBe(1);
      await expect
        .poll(async () => (await snapshot(owner)).every((node) => node.vectorAt === node.updatedAt))
        .toBe(true);
      const after = await snapshot(owner);
      expect(after.filter((node) => node.name !== 'Cached 1')).toEqual(
        before.filter((node) => node.name !== 'Cached 1'),
      );
      expect(after.find((node) => node.name === 'Cached 1')!.updatedAt).toBeGreaterThan(
        before.find((node) => node.name === 'Cached 1')!.updatedAt,
      );
      await owner.getByRole('button', { name: 'Actions for Cached 2', exact: true }).click();
      await owner.getByRole('menuitem', { name: 'Rename', exact: true }).click();
      await owner.getByRole('textbox', { name: 'Name', exact: true }).fill('Cached 2 renamed');
      await owner.getByRole('textbox', { name: 'Name', exact: true }).press('Enter');
      await expect.poll(() => calls(owner)).toBe(2);
      await expect
        .poll(async () => (await snapshot(owner)).every((node) => node.vectorAt === node.updatedAt))
        .toBe(true);
      expect((await snapshot(owner)).filter((node) => node.name !== 'Cached 2 renamed')).toEqual(
        after.filter((node) => node.name !== 'Cached 2'),
      );
    } finally {
      for (const page of context.pages()) await page.goto('about:blank').catch(() => {});
      await context.close();
    }
  });
  test('an imported folder starts uploading and pairs even when no document is open', async ({ browser }) => {
    test.setTimeout(120000);
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const ownerContext = await isolatedContext(browser, headers),
      peerContext = await isolatedContext(browser, headers);
    const owner = await ownerContext.newPage(),
      peer = await peerContext.newPage();
    const files = Object.fromEntries(
      Array.from({ length: 24 }, (_, index) => [
        `Folder ${index % 7}/Document ${index}.md`,
        Buffer.from(`# Document ${index}\n\nOriginal text ${index}.`),
      ]),
    );
    try {
      await owner.goto(MARS_BASE_URL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.getByRole('button', { name: 'New document, folder, or import', exact: true }).click();
      await owner.getByRole('menuitem', { name: 'New folder', exact: true }).click();
      await owner.getByRole('textbox', { name: 'Name', exact: true }).fill('Imported folder');
      await owner.getByRole('textbox', { name: 'Name', exact: true }).press('Enter');
      await owner
        .locator('#importInput')
        .setInputFiles({ name: 'Imported.zip', mimeType: 'application/zip', buffer: Buffer.from(zipSync(files)) });
      await expect(owner.locator('#toast')).toContainText('Imported 24 documents');
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.getByRole('button', { name: 'Actions for Imported folder', exact: true }).click();
      await owner.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
      await owner.getByRole('button', { name: 'Turn on sync', exact: true }).click();
      await expect(owner.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible({
        timeout: 30000,
      });
      await expect(owner.locator('#tree [data-connection]')).toHaveCount(32);
      await pair(owner, peer, 'Imported folder');
      await expect(peer.locator('#toast')).toContainText('Sync connected', { timeout: 60000 });
      for (const page of [peer, owner]) {
        const folder = page
          .getByRole('treeitem')
          .filter({ has: page.getByRole('button', { name: 'Actions for Folder 2', exact: true }) });
        await expect(folder).toBeVisible({ timeout: 30000 });
        if ((await folder.getAttribute('aria-expanded')) !== 'true')
          await folder.getByText('Folder 2', { exact: true }).click();
      }
      await expect(peer.locator('#tree .tree-link').filter({ hasText: 'Document 23' })).toBeVisible({ timeout: 30000 });
      await peer.locator('#tree .tree-link').filter({ hasText: 'Document 23' }).click();
      await expect(peer.locator('#previewArticle')).toContainText('Original text 23.');
      await sourceText(peer, '# Document 23\n\nEdited from the paired browser.');
      await owner.locator('#tree .tree-link').filter({ hasText: 'Document 23' }).click();
      await expect(owner.locator('#previewArticle')).toContainText('Edited from the paired browser.', {
        timeout: 15000,
      });
    } finally {
      await owner.goto('about:blank').catch(() => {});
      await peer.goto('about:blank').catch(() => {});
      await ownerContext.close();
      await peerContext.close();
    }
  });
  test('named browser removal stays beside its connection and retains that browser’s local work', async ({
    browser,
  }) => {
    test.setTimeout(60000);
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const ownerContext = await isolatedContext(browser, headers),
      peerContext = await isolatedContext(browser, headers);
    const owner = await ownerContext.newPage(),
      peer = await peerContext.newPage();
    try {
      await syncedDocument(owner, 'Report', '# Report\n\nShared text.');
      await pair(owner, peer, 'Report', 'My work browser');
      await expect(peer.locator('#previewArticle')).toContainText('Shared text.');
      await peerContext.setOffline(true);
      await sourceText(peer, '# Report\n\nMy pending work.');
      await owner.reload();
      await expect(owner.locator('#previewArticle')).toContainText('Shared text.');
      await owner.getByRole('button', { name: 'Actions for Report', exact: true }).click();
      await owner.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
      await owner.getByRole('button', { name: 'Devices', exact: true }).click();
      const item = owner.locator('.sync-access-item').filter({ hasText: 'My work browser' });
      const disconnect = item.getByRole('button', { name: /^Disconnect / });
      await disconnect.hover();
      await expect(owner.locator('.su-tooltip')).toHaveText('Disconnect device');
      expect((await owner.locator('.su-tooltip').boundingBox())!.height).toBeLessThanOrEqual(36);
      await expect(item).toHaveCount(1);
      await expect(item.getByRole('button')).toHaveCount(1);
      await item.getByRole('button', { name: /^Disconnect / }).click();
      await expect(owner.locator('#syncDialog')).toContainText('Local copies and unsent changes stay on that device.');
      await owner.getByRole('button', { name: 'Back', exact: true }).click();
      await expect(item).toBeVisible();
      await item.getByRole('button', { name: /^Disconnect / }).click();
      await owner.getByRole('button', { name: 'Disconnect device', exact: true }).click();
      await expect(owner.locator('#syncDialog')).toContainText('No other devices connected yet.');
      await expect(owner.locator('#syncDialog').getByRole('button', { name: /^Disconnect/ })).toHaveCount(0);
      await owner.getByRole('button', { name: 'Close', exact: true }).click();
      await peerContext.setOffline(false);
      await expect(peer.locator('#saveState')).toContainText('Local copy · connection ended', { timeout: 15000 });
      await expect(peer.locator('#previewArticle')).toContainText('My pending work.');
      await peer.reload();
      await expect(peer.locator('#previewArticle')).toContainText('My pending work.');
      await expect(owner.locator('#previewArticle')).toContainText('Shared text.');
    } finally {
      await owner.goto('about:blank').catch(() => {});
      await peer.goto('about:blank').catch(() => {});
      await ownerContext.close();
      await peerContext.close();
    }
  });
  test('large encrypted recovery restores oversized pending work beside a private library', async ({ browser }) => {
    test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Requires the changed local responder');
    test.setTimeout(120000);
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const a = await isolatedContext(browser, headers),
      b = await isolatedContext(browser, headers);
    const owner = await a.newPage(),
      restored = await b.newPage();
    try {
      await syncedDocument(owner, 'Large recovery', 'Acknowledged small document.');
      await owner.getByRole('button', { name: 'Source', exact: true }).click();
      await expect(owner.getByRole('textbox', { name: 'Markdown source', exact: true })).toBeVisible();
      const length = await owner.evaluate(() => {
        const text = '# Large recovery\n\n' + 'x'.repeat(6 * 1024 * 1024);
        (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(text);
        return text.length;
      });
      await expect(owner.locator('#saveState')).toContainText('Needs attention', { timeout: 30000 });
      await owner.getByRole('button', { name: 'Actions for Large recovery', exact: true }).click();
      await owner.getByRole('menuitem', { name: 'Download', exact: true }).click();
      await owner.getByRole('checkbox', { name: 'Include connections and pending changes', exact: true }).check();
      await owner.getByRole('textbox', { name: 'Password', exact: true }).fill('separate recovery password');
      const pending = owner.waitForEvent('download');
      await owner.locator('#downloadDialog').getByRole('button', { name: 'Download', exact: true }).click();
      const file = await pending,
        bytes = await (await import('node:fs/promises')).readFile((await file.path())!);
      expect(file.suggestedFilename()).toMatch(/\.zip$/);
      expect(bytes.length).toBeGreaterThan(16 * 1024 * 1024);
      const archive = (require('fflate') as { unzipSync(bytes: Uint8Array): Record<string, Uint8Array> }).unzipSync(
        bytes,
      );
      expect(
        Buffer.from(archive['MARS connections/mars-encrypted-recovery.mars-recovery']).subarray(0, 8).toString(),
      ).toBe('MARSREC2');
      await owner.goto('about:blank');
      await restored.goto(MARS_BASE_URL);
      await expect(restored.locator('#emptyState')).toBeVisible();
      await restored.locator('#fileInput').setInputFiles({
        name: 'Private.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from('Private local content.'),
      });
      await expect(restored.locator('#previewArticle')).toContainText('Private local content.');
      await restored.getByRole('button', { name: 'New document, folder, or import' }).click();
      const chooser = restored.waitForEvent('filechooser');
      await restored.getByRole('menuitem', { name: 'Restore backup…', exact: true }).click();
      await (
        await chooser
      ).setFiles({ name: file.suggestedFilename(), mimeType: 'application/octet-stream', buffer: bytes });
      await restored.getByRole('textbox', { name: 'Backup password', exact: true }).fill('separate recovery password');
      await restored.getByRole('button', { name: 'Restore', exact: true }).click();
      await expect(restored.locator('#toast')).toContainText('Backup restored', { timeout: 30000 });
      const saved = await restored.evaluate(async () => {
        const database = await new Promise<IDBDatabase>((resolve) => {
          const request = indexedDB.open('mars');
          request.onsuccess = () => resolve(request.result);
        });
        const nodes = await new Promise<{ id: string; name: string }[]>((resolve) => {
          const request = database.transaction('nodes').objectStore('nodes').getAll();
          request.onsuccess = () => resolve(request.result);
        });
        const node = nodes.find((node) => node.name === 'Large recovery')!;
        const text = await new Promise<string>((resolve) => {
          const request = database.transaction('docs').objectStore('docs').get(node.id);
          request.onsuccess = () => resolve(request.result);
        });
        const record = await new Promise<{ outbox: unknown[] }>((resolve) => {
          const request = database.transaction('docs').objectStore('docs').get(`sync:${node.id}`);
          request.onsuccess = () => resolve(request.result);
        });
        database.close();
        return {
          length: text.length,
          pending: record.outbox.length,
          private: nodes.some((node) => node.name === 'Private'),
        };
      });
      expect(saved.length).toBe(length);
      expect(saved.pending).toBeGreaterThan(0);
      expect(saved.private).toBe(true);
      await expect(restored.locator('#previewArticle')).toContainText('Private local content.');
    } finally {
      await owner.goto('about:blank').catch(() => {});
      await restored.goto('about:blank').catch(() => {});
      await a.close();
      await b.close();
    }
  });
  for (const failure of ['key-write', 'live-key-write', 'rotate-document'])
    test(`interrupted ${failure} resumes after reload without losing local edits or another live link`, async ({
      browser,
    }) => {
      test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Requires the changed local responder');
      test.setTimeout(90000);
      const headers = { 'x-mars-test-client': crypto.randomUUID() };
      const a = await isolatedContext(browser, headers),
        b = await isolatedContext(browser, headers);
      const owner = await a.newPage(),
        guest = await b.newPage();
      try {
        await syncedDocument(owner, 'Interrupted', 'Before rotation.');
        const links: string[] = [];
        for (let index = 0; index < 2; index++) {
          links.push(await (await import('./_mars_sharing')).copyLink(owner, { mode: 'live' }));
        }
        await guest.goto(links[1]);
        await expect(guest.locator('#previewArticle')).toContainText('Before rotation.');
        let interrupt = true;
        const attempts: string[] = [];
        const endpoint = `${new URL(MARS_BASE_URL).origin}/sync/${failure}`;
        await a.route(endpoint, async (route) => {
          attempts.push(route.request().postData()!);
          if (interrupt) {
            const response = await route.fetch();
            expect(response.ok()).toBe(true);
            interrupt = false;
            await route.fulfill({
              status: 503,
              contentType: 'application/json',
              body: JSON.stringify({ error: 'lost-key-update-acknowledgement' }),
            });
          } else await route.continue();
        });
        await owner.getByRole('button', { name: 'Actions for Interrupted', exact: true }).click();
        await owner.getByRole('menuitem', { name: 'Copy link', exact: true }).click();
        await owner
          .getByRole('dialog', { name: 'Copy link', exact: true })
          .getByRole('button', { name: 'Live links', exact: true })
          .click();
        await owner
          .locator('#liveLinksDialog')
          .getByRole('button', { name: /^Disable link \d/ })
          .first()
          .click();
        await owner.getByRole('button', { name: 'Disable link', exact: true }).click();
        await expect(owner.locator('#toast')).toContainText('Revocation needs attention');
        await owner.reload();
        await expect(owner.locator('#saveState')).toContainText('Needs attention');
        await sourceText(owner, 'Local work while rotation is interrupted.');
        await guest.reload();
        await expect(guest.locator('#previewArticle')).toContainText('Before rotation.');
        await owner.getByRole('button', { name: 'Actions for Interrupted', exact: true }).click();
        await owner.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
        await owner.getByRole('button', { name: 'Try again', exact: true }).click();
        await owner.getByRole('button', { name: 'Close', exact: true }).click();
        await expect(guest.locator('#previewArticle')).toContainText('Local work while rotation is interrupted.', {
          timeout: 15000,
        });
        expect(attempts.length).toBeGreaterThanOrEqual(2);
        expect(attempts[1]).toBe(attempts[0]);
        await guest.reload();
        await expect(guest.locator('#previewArticle')).toContainText('Local work while rotation is interrupted.');
      } finally {
        await owner.goto('about:blank').catch(() => {});
        await guest.goto('about:blank').catch(() => {});
        await a.close();
        await b.close();
      }
    });
  test('snapshot reopening enters the connected replica and unbound imported review authors stay in a local copy', async ({
    browser,
  }) => {
    test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Requires the changed local responder');
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const a = await isolatedContext(browser, headers),
      b = await isolatedContext(browser, headers);
    const owner = await a.newPage(),
      peer = await b.newPage();
    try {
      const threads = [
        {
          id: 'legacy-thread',
          quote: 'body',
          comments: [{ id: 'legacy-comment', author: 'Owner', body: 'Seed review' }],
        },
      ];
      const source = (body: string, reviews = threads) =>
        `# Reopen\n\nShared paragraph.\n\n${body}\n\n<!-- su-comments\n${JSON.stringify(reviews)}\n-->\n`;
      await syncedDocument(owner, 'Reopen', source('Original body.'));
      await pair(owner, peer, 'Reopen');
      await expect(peer.locator('#commentsRail')).toContainText('Seed review');
      const reopen = async (text: string) => {
        await owner
          .locator('#fileInput')
          .setInputFiles({ name: 'Reopen.md', mimeType: 'text/markdown', buffer: Buffer.from(text) });
        await owner.getByRole('button', { name: 'Update "Reopen"', exact: true }).click();
      };
      await reopen(source('Reopened body.'));
      await expect(peer.locator('#previewArticle')).toContainText('Reopened body.', { timeout: 15000 });
      const imported = [
        {
          ...threads[0],
          comments: [
            ...threads[0].comments,
            { id: 'unbound-import', author: 'Imported author', body: 'Unbound import' },
          ],
        },
      ];
      await reopen(source('Reopened body.', imported));
      await owner
        .getByRole('dialog', { name: 'Keep the incoming copy?', exact: true })
        .getByRole('button', { name: 'Save as a copy', exact: true })
        .click();
      await expect(owner.locator('#commentsRail')).toContainText('Unbound import');
      await expect(owner.locator('#docTitle')).toContainText('incoming copy');
      await expect(peer.locator('#commentsRail')).not.toContainText('Unbound import');
      await owner.getByRole('link', { name: 'Reopen', exact: true }).click();
      await sourceText(owner, '# Reopen\n\nShared paragraph.\n\nFurther authorized body edit.');
      await expect(peer.locator('#previewArticle')).toContainText('Further authorized body edit.', { timeout: 15000 });
      await owner.reload();
      await expect(owner.locator('#previewArticle')).toContainText('Further authorized body edit.');
    } finally {
      await owner.goto('about:blank').catch(() => {});
      await peer.goto('about:blank').catch(() => {});
      await a.close();
      await b.close();
    }
  });
  test('private review acknowledgement reaches the other paired client and resolution Undo stays shared', async ({
    browser,
  }) => {
    test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Requires the changed local responder');
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const contexts = await Promise.all([
      isolatedContext(browser, headers),
      isolatedContext(browser, headers),
      isolatedContext(browser, headers),
    ]);
    const [owner, first, second] = await Promise.all(contexts.map((context) => context.newPage()));
    try {
      const text =
        '# Reviewed\n\nOriginal body.\n\n<!-- su-comments\n[{"id":"legacy-thread","quote":"body","comments":[{"id":"legacy-comment","author":"Owner","body":"Seed review"}]}]\n-->\n';
      await syncedDocument(owner, 'Reviewed', text);
      for (const peer of [first, second]) {
        await pair(owner, peer, 'Reviewed');
        await expect(peer.locator('#commentsRail')).toContainText('Seed review');
      }
      await sourceText(owner, '# Reviewed\n\nChanged body.');
      for (const peer of [first, second]) await expect(peer.locator('#changesBar')).toBeVisible({ timeout: 15000 });
      await first.getByRole('button', { name: 'Mark reviewed', exact: true }).click();
      for (const peer of [first, second]) await expect(peer.locator('#changesBar')).toBeHidden({ timeout: 15000 });
      await first.getByRole('button', { name: 'Resolve thread', exact: true }).click();
      await expect(owner.locator('#commentsRail .cm-card.resolved')).toHaveCount(1, { timeout: 15000 });
      await first.locator('#toast').getByRole('button', { name: 'Undo', exact: true }).click();
      for (const peer of [owner, second]) {
        await expect(peer.locator('#commentsRail .cm-card.resolved')).toHaveCount(0, { timeout: 15000 });
        await expect(peer.locator('#commentsRail')).toContainText('Seed review');
      }
    } finally {
      await Promise.all([owner, first, second].map((page) => page.goto('about:blank').catch(() => {})));
      await Promise.all(contexts.map((context) => context.close()));
    }
  });
  for (const choice of ['separate', 'connect'])
    test(`a populated paired library can ${choice} matching copies without losing local content`, async ({
      browser,
    }) => {
      test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Requires the changed local responder');
      const headers = { 'x-mars-test-client': crypto.randomUUID() };
      const a = await isolatedContext(browser, headers),
        b = await isolatedContext(browser, headers);
      const owner = await a.newPage(),
        peer = await b.newPage();
      try {
        const text = '# Match\n\nShared first paragraph.\n\nRemote last paragraph.';
        await syncedDocument(owner, 'Match', text);
        await peer.goto(MARS_BASE_URL);
        await expect(peer.locator('#emptyState')).toBeVisible();
        await peer.locator('#fileInput').setInputFiles({
          name: 'Match.md',
          mimeType: 'text/markdown',
          buffer: Buffer.from(text.replace('Remote last paragraph.', 'Local last paragraph.')),
        });
        await expect(peer.locator('#previewArticle')).toContainText('Local last paragraph.');
        const originalId = new URL(peer.url()).hash;
        await pair(owner, peer, 'Match');
        if (choice === 'separate') {
          await peer.getByRole('button', { name: 'Keep copies separate', exact: true }).click();
          await expect(peer.locator('#previewArticle')).toContainText('Remote last paragraph.');
          await expect(peer.locator('#tree .tree-link').filter({ hasText: /^Match$/ })).toHaveCount(2);
          await peer.locator(`#tree .tree-link[href$="${originalId}"]`).click();
          await expect(peer.locator('#previewArticle')).toContainText('Local last paragraph.');
          await peer.reload();
          await expect(peer.locator('#previewArticle')).toContainText('Local last paragraph.');
          return;
        }
        await peer.getByRole('button', { name: 'Connect with Sync text', exact: true }).click();
        await expect(peer.locator('#previewArticle')).toContainText('Remote last paragraph.');
        expect(new URL(peer.url()).hash).toBe(originalId);
        await expect(peer.locator('#tree .tree-link').filter({ hasText: /^Match$/ })).toHaveCount(1);
        await peer.locator('#tree .tree-link').filter({ hasText: 'before Sync' }).click();
        await expect(peer.locator('#previewArticle')).toContainText('Local last paragraph.');
        await peer.reload();
        await expect(peer.locator('#previewArticle')).toContainText('Local last paragraph.');
        await peer
          .locator('#tree .tree-link')
          .filter({ hasText: /^Match$/ })
          .click();
        await expect(peer.locator('#previewArticle')).toContainText('Remote last paragraph.');
      } finally {
        await owner.goto('about:blank').catch(() => {});
        await peer.goto('about:blank').catch(() => {});
        await a.close();
        await b.close();
      }
    });
  test('unlink retains offline work as a local copy without revoking the other client', async ({ browser }) => {
    test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Requires the changed local responder');
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const a = await isolatedContext(browser, headers),
      b = await isolatedContext(browser, headers);
    const owner = await a.newPage(),
      peer = await b.newPage();
    try {
      await syncedDocument(owner, 'Unlink', 'Original connected text');
      await pair(owner, peer, 'Unlink');
      await expect(peer.locator('#previewArticle')).toContainText('Original connected text');
      await b.setOffline(true);
      await sourceText(peer, 'Retained offline work');
      await peer.getByRole('button', { name: 'Actions for Unlink', exact: true }).click();
      await peer.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
      await peer.getByRole('button', { name: 'Disconnect this device', exact: true }).click();
      await peer.getByRole('button', { name: 'Disconnect', exact: true }).click();
      await peer.getByRole('button', { name: 'Close', exact: true }).click();
      await expect(peer.locator('[data-connection]')).toHaveCount(0);
      await b.setOffline(false);
      await peer.reload();
      await expect(peer.locator('#previewArticle')).toContainText('Retained offline work');
      await sourceText(owner, 'The owner remains connected');
      await expect(owner.locator('#saveState')).toContainText('Up to date', { timeout: 15000 });
      await peer.reload();
      await expect(peer.locator('#previewArticle')).toContainText('Retained offline work');
      await expect(peer.locator('[data-connection]')).toHaveCount(0);
    } finally {
      await owner.goto('about:blank').catch(() => {});
      await peer.goto('about:blank').catch(() => {});
      await a.close();
      await b.close();
    }
  });
  test('remote deletion retains another client’s offline replica and prevents implicit resurrection', async ({
    browser,
  }) => {
    test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Requires the changed local responder');
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const a = await isolatedContext(browser, headers),
      b = await isolatedContext(browser, headers);
    const owner = await a.newPage(),
      peer = await b.newPage();
    try {
      await syncedDocument(owner, 'Deleted', 'Original connected text');
      await pair(owner, peer, 'Deleted');
      await expect(peer.locator('#previewArticle')).toContainText('Original connected text');
      await b.setOffline(true);
      await sourceText(peer, 'Retained offline branch');
      await owner.getByRole('button', { name: 'Actions for Deleted', exact: true }).click();
      await owner.getByRole('menuitem', { name: 'Delete', exact: true }).click();
      await owner.getByRole('button', { name: 'Delete from collection', exact: true }).click();
      await expect(owner.locator('#tree .tree-link')).toHaveCount(0);
      await b.setOffline(false);
      await expect(peer.locator('#saveState')).toContainText('Local copy · connection ended', { timeout: 15000 });
      await peer.reload();
      await expect(peer.locator('#previewArticle')).toContainText('Retained offline branch');
      await expect(peer.locator('#saveState')).toContainText('Local copy · connection ended', { timeout: 15000 });
      await sourceText(peer, 'Further local recovery work');
      await expect(peer.locator('#previewArticle')).toContainText('Further local recovery work');
      await expect(peer.locator('#saveState')).toContainText('Local copy · connection ended', { timeout: 15000 });
    } finally {
      await owner.goto('about:blank').catch(() => {});
      await peer.goto('about:blank').catch(() => {});
      await a.close();
      await b.close();
    }
  });
  test('linked files receive remote edits without disk writes until explicit Save, including after unlink', async ({
    browser,
    browserName,
  }) => {
    test.skip(browserName !== 'chromium', 'File System Access handles require Chromium');
    test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Requires the changed local responder');
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const a = await isolatedContext(browser, headers),
      b = await isolatedContext(browser, headers);
    const owner = await a.newPage(),
      peer = await b.newPage();
    try {
      await owner.goto(MARS_BASE_URL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.evaluate(async () => {
        const directory = await navigator.storage.getDirectory(),
          file = await directory.getFileHandle('Linked.md', { create: true });
        const writer = await file.createWritable();
        await writer.write('# Linked\n\nOriginal disk text.');
        await writer.close();
        const state = window as unknown as {
          __linkedHandle: FileSystemFileHandle;
          __linkedWrites: number;
          showOpenFilePicker: () => Promise<FileSystemFileHandle[]>;
        };
        state.__linkedHandle = file;
        state.__linkedWrites = 0;
        state.showOpenFilePicker = async () => [file];
        const original = FileSystemFileHandle.prototype.createWritable;
        FileSystemFileHandle.prototype.createWritable = function (...args) {
          if (this.name === 'Linked.md') state.__linkedWrites++;
          return original.apply(this, args);
        };
      });
      await owner.getByRole('button', { name: 'Open file', exact: true }).click();
      await expect(owner.locator('#docTitle')).toHaveText('Linked');
      await owner.getByRole('button', { name: 'Actions for Linked', exact: true }).click();
      await owner.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
      await owner
        .getByRole('dialog', { name: /^(Sync|Connections)$/, exact: true })
        .getByRole('button', { name: 'Turn on sync', exact: true })
        .click();
      await expect(owner.locator('#saveState')).toContainText('Up to date', { timeout: 15000 });
      await pair(owner, peer, 'Linked');
      await expect(peer.locator('#previewArticle')).toContainText('Original disk text.');
      await sourceText(peer, '# Linked\n\nRemote edit.');
      await expect(owner.locator('#previewArticle')).toContainText('Remote edit.', { timeout: 15000 });
      const diskText = () =>
        owner.evaluate(async () =>
          (await (window as unknown as { __linkedHandle: FileSystemFileHandle }).__linkedHandle.getFile()).text(),
        );
      expect(await diskText()).toContain('Original disk text.');
      expect(await owner.evaluate(() => (window as unknown as { __linkedWrites: number }).__linkedWrites)).toBe(0);
      await owner.keyboard.press('ControlOrMeta+s');
      await expect.poll(diskText).toContain('Remote edit.');
      expect(await owner.evaluate(() => (window as unknown as { __linkedWrites: number }).__linkedWrites)).toBe(1);
      await owner.getByRole('button', { name: 'Actions for Linked', exact: true }).click();
      await owner.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
      await owner.getByRole('button', { name: 'Disconnect this device', exact: true }).click();
      await owner.getByRole('button', { name: 'Disconnect', exact: true }).click();
      await owner.getByRole('button', { name: 'Close', exact: true }).click();
      await sourceText(owner, '# Linked\n\nLocal unlinked edit.');
      expect(await diskText()).toContain('Remote edit.');
      expect(await owner.evaluate(() => (window as unknown as { __linkedWrites: number }).__linkedWrites)).toBe(1);
    } finally {
      await owner.goto('about:blank').catch(() => {});
      await peer.goto('about:blank').catch(() => {});
      await a.close();
      await b.close();
    }
  });
  test('encrypted recovery retains queued work and restores beside an existing private library', async ({
    browser,
  }) => {
    test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Requires the changed local responder');
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const a = await isolatedContext(browser, headers),
      b = await isolatedContext(browser, headers);
    const owner = await a.newPage(),
      restored = await b.newPage();
    try {
      await syncedDocument(owner, 'Recovery', 'Acknowledged original');
      await a.route(`${new URL(MARS_BASE_URL).origin}/sync/append`, (route) =>
        route.fulfill({
          status: 507,
          contentType: 'application/json',
          body: JSON.stringify({ error: 'sync-storage-full' }),
        }),
      );
      await sourceText(owner, 'Queued recovery work');
      await owner.getByRole('button', { name: 'Actions for Recovery', exact: true }).click();
      await owner.getByRole('menuitem', { name: 'Download', exact: true }).click();
      await owner.getByRole('checkbox', { name: 'Include connections and pending changes', exact: true }).check();
      await owner.getByRole('textbox', { name: 'Password', exact: true }).fill('separate recovery password');
      const download = owner.waitForEvent('download');
      await owner.locator('#downloadDialog').getByRole('button', { name: 'Download', exact: true }).click();
      const file = await download,
        bytes = await (await import('node:fs/promises')).readFile((await file.path())!);
      const contents = (require('fflate') as { unzipSync(bytes: Uint8Array): Record<string, Uint8Array> }).unzipSync(
        bytes,
      );
      expect(
        Buffer.from(contents['MARS connections/mars-encrypted-recovery.json']).includes(
          Buffer.from('Queued recovery work'),
        ),
      ).toBe(false);
      expect(Buffer.from(contents['Recovery.md']).toString()).toContain('Queued recovery work');
      await owner.goto('about:blank');
      await restored.goto(MARS_BASE_URL);
      await expect(restored.locator('#emptyState')).toBeVisible();
      await restored
        .locator('#fileInput')
        .setInputFiles({ name: 'Private.md', mimeType: 'text/markdown', buffer: Buffer.from('Private local content') });
      await expect(restored.locator('#previewArticle')).toContainText('Private local content');
      await restored.getByRole('button', { name: 'New document, folder, or import' }).click();
      const chooser = restored.waitForEvent('filechooser');
      await restored.getByRole('menuitem', { name: 'Restore backup…', exact: true }).click();
      await (await chooser).setFiles({ name: file.suggestedFilename(), mimeType: 'application/zip', buffer: bytes });
      await restored.getByRole('textbox', { name: 'Backup password', exact: true }).fill('separate recovery password');
      await restored.getByRole('button', { name: 'Restore', exact: true }).click();
      await expect(restored.locator('#toast')).toContainText('Backup restored', { timeout: 15000 });
      await restored
        .locator('#tree .tree-link')
        .filter({ hasText: /^Recovery$/ })
        .click();
      await expect(restored.locator('#previewArticle')).toContainText('Queued recovery work');
      await restored.reload();
      await expect(restored.locator('#previewArticle')).toContainText('Queued recovery work');
      await restored
        .locator('#tree .tree-link')
        .filter({ hasText: /^Private$/ })
        .click();
      await expect(restored.locator('#previewArticle')).toContainText('Private local content');
    } finally {
      await restored.goto('about:blank').catch(() => {});
      await a.close();
      await b.close();
    }
  });
  test('same-profile tabs merge offline edits and refresh their displayed replicas', async ({ browser }) => {
    test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Requires the changed local responder');
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const a = await isolatedContext(browser, headers),
      b = await isolatedContext(browser, headers);
    const first = await a.newPage(),
      peer = await b.newPage();
    try {
      await syncedDocument(first, 'Tabs', 'one two three');
      await pair(first, peer, 'Tabs');
      await expect(peer.locator('#previewArticle')).toContainText('one two three');
      const second = await a.newPage();
      await second.goto(first.url());
      await expect(second.locator('#previewArticle')).toContainText('one two three');
      await a.setOffline(true);
      await Promise.all([sourceText(first, 'first ', 'prepend'), sourceText(second, ' second', 'append')]);
      await expect(first.locator('#previewArticle')).toContainText('first one two three second');
      await expect(second.locator('#previewArticle')).toContainText('first one two three second');
      await a.setOffline(false);
      await expect(peer.locator('#previewArticle')).toContainText('first one two three second', { timeout: 15000 });
      await second.reload();
      await expect(second.locator('#previewArticle')).toContainText('first one two three second');
    } finally {
      await Promise.all(a.pages().map((page) => page.goto('about:blank').catch(() => {})));
      await a.close();
      await b.close();
    }
  });
  test('failed connected storage retains the visible edit and pauses publication until retry', async ({ browser }) => {
    test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Requires the changed local responder');
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const a = await isolatedContext(browser, headers),
      b = await isolatedContext(browser, headers);
    const owner = await a.newPage(),
      peer = await b.newPage();
    try {
      await syncedDocument(owner, 'Storage', 'Durable original');
      await pair(owner, peer, 'Storage');
      await expect(peer.locator('#previewArticle')).toContainText('Durable original');
      await owner.evaluate(() => {
        const original = IDBObjectStore.prototype.put;
        const state = window as unknown as { __failSyncStorage: boolean };
        state.__failSyncStorage = true;
        IDBObjectStore.prototype.put = function (...args) {
          if (state.__failSyncStorage && typeof args[1] === 'string' && args[1].startsWith('sync:'))
            throw new DOMException('Test storage quota', 'QuotaExceededError');
          return original.apply(this, args);
        };
      });
      await sourceText(owner, 'Visible unpublished edit');
      await expect(owner.locator('#previewArticle')).toContainText('Visible unpublished edit');
      await expect(owner.locator('#saveState')).toContainText('Saving locally');
      await expect(peer.locator('#previewArticle')).toContainText('Durable original');
      await expect(owner.locator('#temporarySession')).toBeHidden();
      await owner.evaluate(() => {
        (window as unknown as { __failSyncStorage: boolean }).__failSyncStorage = false;
      });
      await owner.getByRole('button', { name: 'Actions for Storage', exact: true }).click();
      await owner.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
      await owner.getByRole('button', { name: 'Try again', exact: true }).click();
      await owner.getByRole('button', { name: 'Close', exact: true }).click();
      await expect(peer.locator('#previewArticle')).toContainText('Visible unpublished edit', { timeout: 15000 });
      await owner.reload();
      await expect(owner.locator('#previewArticle')).toContainText('Visible unpublished edit');
    } finally {
      await owner.goto('about:blank').catch(() => {});
      await peer.goto('about:blank').catch(() => {});
      await a.close();
      await b.close();
    }
  });
  test('overlapping remote cell edits retain the draft and offer an independent recovery copy', async ({ browser }) => {
    test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Requires the changed local responder');
    test.setTimeout(90000);
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const a = await isolatedContext(browser, headers),
      b = await isolatedContext(browser, headers);
    const owner = await a.newPage(),
      peer = await b.newPage();
    try {
      await owner.goto(MARS_BASE_URL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      const text = '# Draft\n\n| A | B |\n| --- | --- |\n| Old | Kept |\n';
      await owner
        .locator('#fileInput')
        .setInputFiles({ name: 'Draft.md', mimeType: 'text/markdown', buffer: Buffer.from(text) });
      await expect(owner.locator('#previewArticle')).toContainText('Old');
      await owner.getByRole('button', { name: 'Actions for Draft', exact: true }).click();
      await owner.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
      await owner
        .getByRole('dialog', { name: /^(Sync|Connections)$/, exact: true })
        .getByRole('button', { name: 'Turn on sync', exact: true })
        .click();
      await expect(owner.locator('#saveState')).toContainText('Up to date', { timeout: 15000 });
      await pair(owner, peer, 'Draft');
      await expect(peer.locator('#previewArticle')).toContainText('Old');
      await owner.locator('#previewArticle td').filter({ hasText: /^Old$/ }).dblclick();
      await owner.getByRole('textbox', { name: 'Edit table cell', exact: true }).fill('Local draft');
      await peer.getByRole('button', { name: 'Source', exact: true }).click();
      await peer.evaluate(
        (value) =>
          (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(value),
        text.replace('Old', 'Remote'),
      );
      await peer.getByRole('button', { name: 'Preview', exact: true }).click();
      await expect(owner.locator('#connectionDraftRecovery')).toBeVisible({ timeout: 15000 });
      await expect(owner.getByRole('textbox', { name: 'Edit table cell', exact: true })).toHaveValue('Local draft');
      await owner.getByRole('button', { name: 'Save draft as copy', exact: true }).click();
      await expect(owner.locator('#previewArticle')).toContainText('Local draft');
      await expect(owner.locator('#previewArticle')).toContainText('Kept');
      await expect(
        owner
          .locator('#tree .tree-link')
          .filter({ hasText: 'recovered draft' })
          .locator('..')
          .locator('[data-connection]'),
      ).toHaveCount(0);
      await owner
        .locator('#tree .tree-link')
        .filter({ hasText: /^Draft$/ })
        .click();
      await expect(owner.locator('#previewArticle')).toContainText('Remote');
    } finally {
      await a.close();
      await b.close();
    }
  });
  test('saving a temporary live document preserves an existing private library', async ({ browser }) => {
    test.setTimeout(90000);
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const ownerContext = await isolatedContext(browser, headers),
      guestContext = await isolatedContext(browser, headers);
    await mockSmartSearchWorker(guestContext);
    const owner = await ownerContext.newPage(),
      guest = await guestContext.newPage();
    try {
      await owner.goto(MARS_BASE_URL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.locator('#fileInput').setInputFiles({
        name: 'Shared.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from('# Shared\n\nLive content.'),
      });
      await expect(owner.locator('#previewArticle')).toContainText('Live content.');
      const link = await (await import('./_mars_sharing')).copyLink(owner, { mode: 'live' });
      await guest.goto(MARS_BASE_URL);
      await expect(guest.locator('#emptyState')).toBeVisible();
      await guest.locator('#fileInput').setInputFiles({
        name: 'Private.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from('# Private\n\nPrivate library content.'),
      });
      await expect(guest.locator('#saveState')).toHaveText('Saved');
      await guest.locator('#importInput').setInputFiles({
        name: 'Private2.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from('# Private2\n\nAnother private library document.'),
      });
      await expect(guest.locator('#tree .tree-label').getByText('Private2', { exact: true })).toBeVisible();
      await guest.getByRole('button', { name: 'Smart search', exact: true }).click();
      await guest.getByRole('radio', { name: /Best speed/ }).check();
      await guest.locator('#marsModalFoot').getByRole('button', { name: 'Turn on', exact: true }).click();
      const libraryIndex = () =>
        guest.evaluate(async () => {
          const database = await new Promise<IDBDatabase>((resolve, reject) => {
            const open = indexedDB.open('mars');
            open.onsuccess = () => resolve(open.result);
            open.onerror = () => reject(open.error);
          });
          try {
            const tx = database.transaction(['nodes', 'docs'], 'readonly'),
              nodes = tx.objectStore('nodes').getAll(),
              vectors = tx.objectStore('docs').getAll(IDBKeyRange.bound('vec:', 'vec:\uffff'));
            await new Promise<void>((resolve, reject) => {
              tx.oncomplete = () => resolve();
              tx.onerror = () => reject(tx.error);
            });
            return (nodes.result as { id: string; name: string; updatedAt: number }[]).map((node) => ({
              ...node,
              vector: (vectors.result as { id: string; at: number; chunks: { v: Float32Array }[] }[]).find(
                (entry) => entry.id === node.id,
              ),
            }));
          } finally {
            database.close();
          }
        });
      await expect
        .poll(async () => (await libraryIndex()).filter((node) => node.vector?.at === node.updatedAt).length)
        .toBe(2);
      const before = await libraryIndex();
      await guest.goto(link);
      await expect(guest.locator('#previewArticle')).toContainText('Live content.', { timeout: 15000 });
      await expect(guest.locator('#tree')).not.toContainText('Private');
      await expect
        .poll(() =>
          guest.evaluate(
            () => (window as unknown as { __marsWorker: { embeds: string[][] } }).__marsWorker.embeds.length,
          ),
        )
        .toBeGreaterThan(0);
      await expect(guest.locator('#sbSmartStatus')).toBeHidden();
      const temporaryCalls = await guest.evaluate(
        () => (window as unknown as { __marsWorker: { embeds: string[][] } }).__marsWorker.embeds.length,
      );
      await guest.getByRole('button', { name: 'Save live document to library', exact: true }).click();
      await expect(guest.getByRole('dialog', { name: 'Save live document to library', exact: true })).not.toContainText(
        /decryption|credentials/,
      );
      await guest
        .getByRole('dialog', { name: 'Save live document to library', exact: true })
        .getByRole('button', { name: 'Save to library', exact: true })
        .click();
      await expect(guest.locator('#temporarySession')).toBeHidden();
      await expect(guest.locator('#tree .tree-link').filter({ hasText: 'Private' }).first()).toBeVisible();
      await expect
        .poll(async () => (await libraryIndex()).filter((node) => node.vector?.at === node.updatedAt).length)
        .toBe(3);
      // Let the promotion's two-second indexing debounce run before checking calls.
      await guest.waitForTimeout(2500);
      const after = await libraryIndex();
      expect(after.filter((node) => node.name.startsWith('Private'))).toEqual(before);
      expect(
        await guest.evaluate(
          () => (window as unknown as { __marsWorker: { embeds: string[][] } }).__marsWorker.embeds.length,
        ),
      ).toBe(temporaryCalls);
      await guest.reload();
      await expect(guest.locator('#previewArticle')).toContainText('Live content.');
      await guest.locator('#tree .tree-link').filter({ hasText: 'Private' }).first().click();
      await expect(guest.locator('#previewArticle')).toContainText('Private library content.');
    } finally {
      await owner.goto('about:blank').catch(() => {});
      await guest.goto('about:blank').catch(() => {});
      await ownerContext.close();
      await guestContext.close();
    }
  });
  test('short Live bootstrap stays private and the long Live receiving path remains compatible', async ({
    browser,
  }) => {
    const headers = { 'x-mars-test-client': crypto.randomUUID() },
      ownerContext = await isolatedContext(browser, headers),
      guestContext = await isolatedContext(browser, headers);
    const owner = await ownerContext.newPage(),
      guest = await guestContext.newPage();
    try {
      await syncedDocument(owner, 'Compatibility', '# Compatibility\n\nEncrypted live content.');
      const link = await (await import('./_mars_sharing')).copyLink(owner, { mode: 'live' });
      expect(new URL(link).hash).toMatch(/^#live=[A-Za-z0-9_-]{22}$/);
      await owner.addScriptTag({ content: liveBundle });
      const long = await owner.evaluate(async (short) => {
        const secret = new URL(short).hash.slice(6),
          api = (
            window as unknown as {
              MARS_LIVE_LINK: {
                liveAccess(secret: string): Promise<{ id: string; token: string }>;
                openLiveBootstrap(value: unknown, secret: string): Promise<unknown>;
              };
            }
          ).MARS_LIVE_LINK;
        const access = await api.liveAccess(secret);
        const response = await fetch('/sync/invite-read', {
          method: 'POST',
          headers: { Authorization: `Bearer ${access.token}`, 'Content-Type': 'application/json' },
          body: JSON.stringify({ invitation: access.id }),
        });
        const incoming = await response.json(),
          payload = await api.openLiveBootstrap(incoming.bootstrap, secret);
        const bytes = new TextEncoder().encode(JSON.stringify(payload)),
          compressed = new Uint8Array(
            await new Response(
              new Blob([bytes]).stream().pipeThrough(new CompressionStream('deflate-raw')),
            ).arrayBuffer(),
          );
        const encoded = new Uint8Array(4 + compressed.length);
        new DataView(encoded.buffer).setUint32(0, bytes.length, true);
        encoded.set(compressed, 4);
        return `${location.origin}/#live=${btoa(String.fromCharCode(...encoded))
          .replace(/\+/g, '-')
          .replace(/\//g, '_')
          .replace(/=+$/, '')}`;
      }, link);
      const requests: string[] = [];
      guest.on('request', (request) => {
        if (request.url().includes('/sync/'))
          requests.push(JSON.stringify({ url: request.url(), headers: request.headers(), body: request.postData() }));
      });
      await guest.goto(link);
      await expect(guest.locator('#previewArticle')).toContainText('Encrypted live content.', { timeout: 15000 });
      expect(requests.join('')).not.toContain(new URL(link).hash.slice(6));
      await guest.goto(long);
      await expect(guest.locator('#previewArticle')).toContainText('Encrypted live content.', { timeout: 15000 });
    } finally {
      await owner.goto('about:blank').catch(() => {});
      await guest.goto('about:blank').catch(() => {});
      await ownerContext.close();
      await guestContext.close();
    }
  });
  test('revocation rotates keys while another authorized live link continues and reloads', async ({ browser }) => {
    test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Requires the changed local responder');
    test.setTimeout(90000);
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const contexts = await Promise.all([
      isolatedContext(browser, headers),
      isolatedContext(browser, headers),
      isolatedContext(browser, headers),
    ]);
    const [owner, revoked, retained] = await Promise.all(contexts.map((context) => context.newPage()));
    try {
      await owner.goto(MARS_BASE_URL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.locator('#fileInput').setInputFiles({
        name: 'Access.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from('# Access\n\nBefore revocation.'),
      });
      await expect(owner.locator('#previewArticle')).toContainText('Before revocation.');
      const makeLink = async () => {
        return (await import('./_mars_sharing')).copyLink(owner, { mode: 'live' });
      };
      const a = await makeLink(),
        b = await makeLink();
      await revoked.goto(a);
      await retained.goto(b);
      await expect(revoked.locator('#previewArticle')).toContainText('Before revocation.');
      await expect(retained.locator('#previewArticle')).toContainText('Before revocation.');
      await owner.getByRole('button', { name: 'Actions for Access', exact: true }).click();
      await owner.getByRole('menuitem', { name: 'Copy link', exact: true }).click();
      await owner
        .getByRole('dialog', { name: 'Copy link', exact: true })
        .getByRole('button', { name: 'Live links', exact: true })
        .click();
      await owner
        .locator('#liveLinksDialog')
        .getByRole('button', { name: /^Disable link \d/ })
        .first()
        .click();
      await owner.getByRole('button', { name: 'Disable link', exact: true }).click();
      await expect(owner.locator('#toast')).toContainText('keys rotated', { timeout: 15000 });
      await owner.getByRole('button', { name: 'Close', exact: true }).click();
      await owner.getByRole('button', { name: 'Source', exact: true }).click();
      await owner.evaluate(() =>
        (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(
          '# Access\n\nProtected after revocation.',
        ),
      );
      await owner.getByRole('button', { name: 'Preview', exact: true }).click();
      await expect(retained.locator('#previewArticle')).toContainText('Protected after revocation.', {
        timeout: 15000,
      });
      await expect(revoked.locator('#saveState')).toContainText('Local copy · connection ended', { timeout: 15000 });
      await expect(revoked.locator('#previewArticle')).not.toContainText('Protected after revocation.');
      await retained.reload();
      await expect(retained.locator('#previewArticle')).toContainText('Protected after revocation.', {
        timeout: 15000,
      });
      await expect(retained.locator('#previewArticle h1')).toHaveCount(1);
    } finally {
      await Promise.all(contexts.map((context) => context.close()));
    }
  });
  test('live view/comment/edit grants open temporary sessions without exposing sibling content', async ({
    browser,
  }) => {
    test.setTimeout(120000);
    const headers = { 'x-mars-test-client': crypto.randomUUID() },
      context = await isolatedContext(browser, headers);
    const owner = await context.newPage();
    try {
      await owner.goto(MARS_BASE_URL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      const text =
        '# Live\n\nShared body.\n\n<!-- su-comments\n[{"id":"thread","quote":"Shared","comments":[{"id":"old","author":"Owner","body":"Original review"}]}]\n-->\n';
      await owner
        .locator('#fileInput')
        .setInputFiles({ name: 'Live.md', mimeType: 'text/markdown', buffer: Buffer.from(text) });
      await expect(owner.locator('#previewArticle')).toContainText('Shared body.');
      for (const permission of ['view', 'comment', 'edit']) {
        const link = await (
          await import('./_mars_sharing')
        ).copyLink(owner, { mode: 'live', permission: permission as 'view' | 'comment' | 'edit' });
        const guestContext = await isolatedContext(browser, headers);
        const guest = await guestContext.newPage();
        await guest.addInitScript(() => {
          Object.defineProperty(window, 'indexedDB', {
            configurable: true,
            get() {
              throw new Error('A temporary live session must not open IndexedDB');
            },
          });
        });
        try {
          await guest.goto(link);
          await expect(guest.locator('#previewArticle')).toContainText('Shared body.', { timeout: 15000 });
          await expect(guest.locator('#temporarySession')).toContainText('Temporary live session');
          await expect(guest.locator('#commentsRail')).toContainText('Original review');
          await expect(guest.locator('#tree .tree-link')).toHaveCount(1);
          const originalComment = guest.locator('.cm-comment').filter({ hasText: 'Original review' });
          await expect(originalComment.getByRole('button', { name: 'Edit', exact: true })).toBeDisabled();
          if (permission === 'edit')
            await expect(originalComment.getByRole('button', { name: 'Delete', exact: true })).toBeEnabled();
          else await expect(originalComment.getByRole('button', { name: 'Delete', exact: true })).toBeDisabled();
          if (permission === 'view') {
            await expect(guest.locator('.cm-composer')).toHaveCount(0);
            await expect(guest.getByRole('button', { name: 'Resolve thread', exact: true })).toBeDisabled();
          } else {
            await guest.getByRole('textbox', { name: 'Reply', exact: true }).fill(`Reply with ${permission} access`);
            await guest.getByRole('button', { name: 'Reply', exact: true }).click();
            await expect(owner.locator('#commentsRail')).toContainText(`Reply with ${permission} access`, {
              timeout: 15000,
            });
            if (permission === 'edit') {
              await originalComment.getByRole('button', { name: 'Delete', exact: true }).click();
              await expect(owner.locator('.cm-comment').filter({ hasText: 'Original review' })).toHaveCount(0, {
                timeout: 15000,
              });
              await guest.reload();
              await expect(guest.locator('#previewArticle')).toContainText('Shared body.', { timeout: 15000 });
              await expect(guest.locator('.cm-comment').filter({ hasText: 'Original review' })).toHaveCount(0);
            }
          }
          await guest.getByRole('button', { name: 'Source', exact: true }).click();
          const writable = await guest.evaluate(() => {
            const fallback = document.querySelector<HTMLTextAreaElement>('.editor-fallback');
            return fallback
              ? !fallback.readOnly
              : document.querySelector('.cm-content')?.getAttribute('contenteditable') === 'true';
          });
          expect(writable).toBe(permission === 'edit');
          if (permission === 'edit') {
            await guest.evaluate(() =>
              (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(
                '# Live\n\nEdited through a live link.',
              ),
            );
            await guest.getByRole('button', { name: 'Preview', exact: true }).click();
            await expect(owner.locator('#previewArticle')).toContainText('Edited through a live link.', {
              timeout: 15000,
            });
          }
        } finally {
          await guest.goto('about:blank').catch(() => {});
          await guestContext.close();
        }
      }
    } finally {
      await context.close();
    }
  });
  test('selected folders carry comments and new documents without uploading unselected content', async ({
    browser,
  }) => {
    test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Requires the changed local responder');
    test.setTimeout(90000);
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const a = await isolatedContext(browser, headers),
      b = await isolatedContext(browser, headers);
    const owner = await a.newPage(),
      peer = await b.newPage();
    try {
      await owner.goto(MARS_BASE_URL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.locator('#fileInput').setInputFiles({
        name: 'Private.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from('# Private\n\nDo not sync this.'),
      });
      await expect(owner.locator('#previewArticle')).toContainText('Do not sync this.');
      await owner.getByRole('button', { name: 'New document, folder, or import' }).click();
      await owner.getByRole('menuitem', { name: 'New folder', exact: true }).click();
      await owner.getByRole('textbox', { name: 'Name', exact: true }).fill('Root');
      await owner.getByRole('textbox', { name: 'Name', exact: true }).press('Enter');
      const text =
        '# Child\n\nInitial child.\n\n<!-- su-comments\n[{"id":"thread","quote":"Initial","comments":[{"id":"old","author":"Owner","body":"Seed comment"}]}]\n-->\n';
      await owner
        .locator('#fileInput')
        .setInputFiles({ name: 'Child.md', mimeType: 'text/markdown', buffer: Buffer.from(text) });
      await expect(owner.locator('#previewArticle')).toContainText('Initial child.');
      await owner.getByRole('button', { name: 'Actions for Root', exact: true }).click();
      await owner.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
      await owner
        .getByRole('dialog', { name: /^(Sync|Connections)$/, exact: true })
        .getByRole('button', { name: 'Turn on sync', exact: true })
        .click();
      await expect(owner.locator('#saveState')).toContainText('Up to date', { timeout: 15000 });
      await pair(owner, peer, 'Root');
      await expect(peer.locator('#tree .tree-link').filter({ hasText: 'Child' })).toBeVisible({ timeout: 15000 });
      await expect(peer.locator('#tree')).not.toContainText('Private');
      await peer.locator('#tree .tree-link').filter({ hasText: 'Child' }).click();
      await expect(peer.locator('#commentsRail')).toContainText('Seed comment');
      await peer.getByRole('textbox', { name: 'Reply', exact: true }).fill('Peer reply');
      await peer.getByRole('button', { name: 'Reply', exact: true }).click();
      await expect(owner.locator('#commentsRail')).toContainText('Peer reply', { timeout: 15000 });
      await peer.getByRole('button', { name: 'New document, folder, or import' }).click();
      await peer.getByRole('menuitem', { name: 'New document', exact: true }).click();
      await expect(peer.getByRole('button', { name: 'Source', exact: true })).toHaveAttribute('aria-pressed', 'true');
      await peer.waitForFunction(() => !!(window as unknown as { __suEditorAPI?: unknown }).__suEditorAPI);
      await peer.evaluate(() =>
        (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(
          '# New child\n\nCreated by peer.',
        ),
      );
      await peer.getByRole('button', { name: 'Preview', exact: true }).click();
      await expect(owner.locator('#tree .tree-link')).toHaveCount(3, { timeout: 15000 });
      const last = owner.locator('#tree .tree-link').filter({ hasText: 'Untitled' });
      await last.click();
      await expect(owner.locator('#previewArticle')).toContainText('Created by peer.', { timeout: 15000 });
      await expect(owner.locator('#previewArticle h1')).toHaveCount(1);
      await expect(
        owner
          .getByRole('button', { name: 'Actions for Private', exact: true })
          .locator('..')
          .locator('[data-connection]'),
      ).toHaveCount(0);
    } finally {
      await a.close();
      await b.close();
    }
  });
  test('one-time pairing connects isolated libraries and body edits survive reopening', async ({ browser }) => {
    test.skip(!/^http:\/\/(localhost|127\.0\.0\.1):/.test(MARS_BASE_URL), 'Requires the changed local responder');
    test.setTimeout(90000);
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    const ownerContext = await isolatedContext(browser, headers);
    const peerContext = await isolatedContext(browser, headers);
    const owner = await ownerContext.newPage(),
      peer = await peerContext.newPage();
    const source = async (page: Page, text: string) => {
      await page.getByRole('button', { name: 'Source', exact: true }).click();
      await page.waitForFunction(() => !!(window as unknown as { __suEditorAPI?: unknown }).__suEditorAPI);
      await page.evaluate(
        (value) =>
          (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(value),
        text,
      );
      await page.getByRole('button', { name: 'Preview', exact: true }).click();
    };
    try {
      await owner.goto(MARS_BASE_URL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.locator('#fileInput').setInputFiles({
        name: 'Connected.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from('# Connected\n\nFirst body.'),
      });
      await expect(owner.locator('#previewArticle')).toContainText('First body.');
      await owner.getByRole('button', { name: 'Actions for Connected', exact: true }).click();
      await owner.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
      await owner
        .getByRole('dialog', { name: /^(Sync|Connections)$/, exact: true })
        .getByRole('button', { name: 'Turn on sync', exact: true })
        .click();
      await expect(owner.locator('#saveState')).toContainText('Up to date', { timeout: 15000 });
      await expect(owner.getByRole('textbox', { name: 'Sync link' })).toBeVisible();
      const invitation = await owner.getByRole('textbox', { name: 'Sync link' }).inputValue();
      await owner.getByRole('button', { name: 'Copy link', exact: true }).click();
      await peer.goto(invitation);
      await peer.getByRole('textbox', { name: 'Name', exact: true }).fill('Peer');
      await peer.getByRole('button', { name: 'Connect', exact: true }).click();
      await owner.getByRole('button', { name: 'Codes match', exact: true }).click({ timeout: 15000 });
      await expect(owner.getByRole('dialog', { name: /^(Sync|Connections)$/, exact: true })).toBeVisible();
      await owner.getByRole('button', { name: 'Close', exact: true }).click();
      await expect(peer.locator('#previewArticle')).toContainText('First body.', { timeout: 15000 });
      await source(peer, '# Connected\n\nEdited by peer.');
      await expect(owner.locator('#previewArticle')).toContainText('Edited by peer.', { timeout: 15000 });
      await expect(owner.locator('#previewArticle')).not.toContainText('First body.');
      await expect(owner.locator('#previewArticle h1')).toHaveCount(1);
      await peer.reload();
      await expect(peer.locator('#previewArticle')).toContainText('Edited by peer.', { timeout: 15000 });
      await source(owner, '# Connected\n\nOwner after reopening.');
      await expect(peer.locator('#previewArticle')).toContainText('Owner after reopening.', { timeout: 15000 });
      await expect(peer.locator('#previewArticle')).not.toContainText('Edited by peer.');
    } finally {
      await ownerContext.close();
      await peerContext.close();
    }
  });
  test('connected raw-HTML policy removes executable content, spoofed controls, and forged source ranges', async ({
    page,
  }) => {
    await page.goto('/');
    await expect(page.locator('#emptyState')).toBeVisible();
    await page.addScriptTag({ content: safetyBundle });
    const result = await page.evaluate(() => {
      const safety = (window as unknown as { MARS_SYNC_SAFETY: { safeRawHtml(html: string): string } })
        .MARS_SYNC_SAFETY;
      const marker = 'UNRELATED PRIVATE DOCUMENT';
      localStorage.setItem('sync-fixture-private', marker);
      const attack = `
      <script>window.__syncFixtureExecuted = localStorage.getItem('sync-fixture-private')</script>
      <div id="saveState" class="su-block" data-start="0" data-end="999" data-table-start="0"
           style="position:fixed;inset:0" onclick="window.__syncFixtureExecuted = 'event'">Kept text</div>
      <form><input name="location"><button data-act="delete-thread">Spoofed control</button></form>
      <iframe srcdoc="<script>parent.__syncFixtureExecuted = 'iframe'</script>"></iframe>
      <svg onload="window.__syncFixtureExecuted = 'svg'"><foreignObject><script>window.__syncFixtureExecuted = 'foreign'</script></foreignObject></svg>
      <a href="java&#x73;cript:window.__syncFixtureExecuted='link'">Unsafe link</a>
      <b>Readable emphasis</b>
      <math><mtext><table><mglyph><style><!--</style><img title="--><img src=x onerror=window.__syncFixtureExecuted='mutation'>"></math>`;
      const mount = document.createElement('section');
      mount.id = 'sync-safety-fixture';
      mount.innerHTML = safety.safeRawHtml(attack);
      document.body.append(mount);
      mount.querySelector<HTMLAnchorElement>('a')?.click();
      return {
        text: mount.textContent,
        unsafe: mount.querySelectorAll('script,iframe,svg,math,form,input,button,style').length,
        attrs: [...mount.querySelectorAll('*')].flatMap((node) => [...node.attributes].map((attr) => attr.name)),
        href: mount.querySelector('a')?.getAttribute('href'),
        executed: (window as unknown as { __syncFixtureExecuted?: string }).__syncFixtureExecuted ?? null,
        privateValue: localStorage.getItem('sync-fixture-private'),
      };
    });
    expect(result.text).toContain('Kept text');
    expect(result.text).toContain('Readable emphasis');
    expect(result.unsafe).toBe(0);
    expect(result.attrs.some((attr) => /^(on|data-)|^(id|name|style|class)$/.test(attr))).toBe(false);
    expect(result.href).toBeNull();
    expect(result.executed).toBeNull();
    expect(result.privateValue).toBe('UNRELATED PRIVATE DOCUMENT');
    await expect(page.locator('#saveState')).toHaveCount(1);
  });

  test('the rendered policy retains only renderer-supplied source mapping and ordinary Markdown output', async ({
    page,
  }) => {
    await page.goto('/');
    await expect(page.locator('#emptyState')).toBeVisible();
    await page.addScriptTag({ content: safetyBundle });
    const result = await page.evaluate(() => {
      const safety = (
        window as unknown as {
          MARS_SYNC_SAFETY: { safeRawHtml(html: string): string; safeRenderedHtml(html: string): string };
        }
      ).MARS_SYNC_SAFETY;
      const raw = safety.safeRawHtml('<p class="su-block" data-start="400" data-end="500">Ordinary text</p>');
      const mount = document.createElement('div');
      mount.innerHTML = safety.safeRenderedHtml(
        `<div class="su-block" data-start="0" data-end="90">${raw}<strong>Bold</strong><input type="checkbox" disabled checked></div>`,
      );
      return {
        ranges: [...mount.querySelectorAll('[data-start]')].map((node) => node.getAttribute('data-start')),
        rawHasRange: mount.querySelector('p')?.hasAttribute('data-start'),
        text: mount.textContent,
        checked: mount.querySelector<HTMLInputElement>('input')?.checked,
      };
    });
    expect(result.ranges).toEqual(['0']);
    expect(result.rawHasRange).toBe(false);
    expect(result.text).toContain('Ordinary text');
    expect(result.text).toContain('Bold');
    expect(result.checked).toBe(true);
  });
});
