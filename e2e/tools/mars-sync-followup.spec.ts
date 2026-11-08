import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';

import { type Browser, expect, type Page, test } from '@playwright/test';

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
