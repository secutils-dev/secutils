import { readFile } from 'node:fs/promises';
import { deflateRawSync, inflateRawSync } from 'node:zlib';

import type { Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

const MARS_BASE_URL = process.env.MARS_BASE_URL ?? 'https://mars.secutils.dev';
test.use({ baseURL: MARS_BASE_URL });
const DOC =
  '# Temporary ✓\n\nOriginal paragraph.\n\n<!-- su-table {"v":1,"cols":[0.3,0.7]} -->\n| Name | State |\n| --- | --- |\n| Alice | Ready |\n';
const state = (text: string) => {
  const bytes = Buffer.from(text);
  const out = Buffer.concat([Buffer.alloc(4), deflateRawSync(bytes)]);
  out.writeUInt32LE(bytes.length);
  return out.toString('base64url');
};
const link = (text: string, temporary = false) =>
  `/#${new URLSearchParams({ name: 'Temporary document', md: state(text), o: 'temporary-test', w: '1100', ...(temporary ? { t: '1' } : {}) })}`;
const source = async (page: Page) => {
  await page.waitForFunction(() => !!(window as { __suEditorAPI?: unknown }).__suEditorAPI);
  return page.evaluate(() => (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText());
};
async function open(page: Page, url: string) {
  await page.goto(url);
  await expect(page.locator('#previewArticle')).toContainText('Original paragraph.');
  await page.waitForFunction(() => !!(window as { __suEditorAPI?: unknown }).__suEditorAPI);
}
async function edit(page: Page) {
  await page.getByRole('button', { name: 'Source', exact: true }).click();
  await page.evaluate(
    (text) => (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(text),
    DOC.replace('Original', 'Temporary edit'),
  );
  await page.keyboard.press('Escape');
  await expect(page.locator('#previewArticle')).toContainText('Temporary edit paragraph.');
  await expect
    .poll(async () => {
      const encoded = await page.evaluate(() => new URLSearchParams(location.hash.slice(1)).get('md'));
      if (!encoded) return null;
      try {
        return inflateRawSync(Buffer.from(encoded, 'base64url').subarray(4)).toString();
      } catch {
        return null;
      }
    })
    .toBe(DOC.replace('Original', 'Temporary edit'));
}

test.describe('temporary sessions', () => {
  test.setTimeout(90000);

  test('New temporary document starts a named source editor in an isolated session', async ({ page }) => {
    await page.goto('/');
    await page.getByRole('button', { name: 'New document, folder, or import' }).click();
    await page.getByRole('menuitem', { name: /^New temporary document/ }).click();
    await expect(page.locator('#temporarySession')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Source', exact: true })).toHaveAttribute('aria-pressed', 'true');
    await expect(page.getByRole('textbox', { name: 'Markdown source', exact: true })).toBeVisible();
    await expect(page.locator('#saveState')).toHaveText('Temporary');
  });

  test('explicit temporary links never open IndexedDB and edits reopen in another profile', async ({
    page,
    browser,
  }) => {
    await page.addInitScript(() => {
      Object.defineProperty(window, 'indexedDB', {
        get() {
          throw new Error('Temporary mode must not read IndexedDB');
        },
      });
    });
    await open(page, link(DOC, true));
    await expect(page.locator('#temporarySession')).toBeVisible();
    await expect(page.locator('#saveState')).toHaveText('Temporary');
    await edit(page);
    const address = page.url();
    const params = new URLSearchParams(new URL(address).hash.slice(1));
    expect(params.get('t')).toBe('1');
    expect(inflateRawSync(Buffer.from(params.get('md')!, 'base64url').subarray(4)).toString()).toBe(
      DOC.replace('Original', 'Temporary edit'),
    );
    const fresh = await browser.newContext({ baseURL: MARS_BASE_URL });
    try {
      const reopened = await fresh.newPage();
      await reopened.goto(address);
      await expect(reopened.locator('#previewArticle')).toContainText('Temporary edit paragraph.');
      await expect(reopened.locator('#saveState')).toHaveText('Temporary');
      await expect(reopened.locator('#previewArticle')).toHaveCSS('max-width', '1100px');
    } finally {
      await fresh.close();
    }
  });

  test('storage denial falls back automatically and still supports Markdown, HTML, and PDF output', async ({
    page,
  }) => {
    await page.addInitScript(() => {
      Object.defineProperty(window, 'indexedDB', {
        get() {
          throw new DOMException('Storage denied', 'SecurityError');
        },
      });
      Object.defineProperty(window, 'BroadcastChannel', {
        value: class {
          constructor() {
            throw new DOMException('Storage denied', 'SecurityError');
          }
        },
      });
    });
    await open(page, link(DOC));
    await expect(page.locator('#temporaryStatus')).toContainText('Storage unavailable');
    await expect(page.locator('#saveState')).toHaveText('Temporary');
    await page.getByRole('button', { name: 'Export document', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: /^Export / });
    await expect(dialog.getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled({ timeout: 20000 });
    const pending = page.waitForEvent('download');
    await dialog.getByRole('button', { name: 'Download Markdown', exact: true }).click();
    expect(await readFile(await (await pending).path(), 'utf8')).toBe(DOC);
    await dialog.getByRole('tab', { name: 'PDF', exact: true }).click();
    await expect(dialog.getByRole('button', { name: 'Print / Save PDF', exact: true })).toBeEnabled({ timeout: 60000 });
    await dialog.getByRole('button', { name: 'Close export', exact: true }).click();
    await edit(page);
  });

  test('temporary reading leaves the existing library unchanged until explicit promotion', async ({
    page,
    context,
  }) => {
    await open(page, link(DOC));
    await expect(page.locator('#saveState')).toHaveText('Saved');
    const original = page.url();
    await page.getByRole('button', { name: 'Document options', exact: true }).click();
    await page.getByRole('button', { name: 'Read temporarily', exact: true }).click();
    await expect(page.locator('#temporarySession')).toBeVisible();
    await edit(page);
    const library = await context.newPage();
    try {
      await open(library, original);
      expect(await source(library)).toBe(DOC);
      await expect(library.locator('#temporarySession')).toBeHidden();
    } finally {
      await library.close();
    }
    await page.getByRole('button', { name: 'Save current to library', exact: true }).click();
    await expect(page.locator('#temporarySession')).toBeHidden();
    await expect(page.locator('#saveState')).toHaveText('Saved');
    expect(await source(page)).toBe(DOC.replace('Original', 'Temporary edit'));
    await page.reload();
    await expect(page.locator('#previewArticle')).toContainText('Temporary edit paragraph.');
  });

  test('a write quota failure preserves the edit in a temporary document without replacing the stored copy', async ({
    page,
    context,
  }) => {
    await open(page, link(DOC));
    await expect(page.locator('#saveState')).toHaveText('Saved');
    const original = page.url();
    await page.evaluate(() => {
      const transaction = IDBDatabase.prototype.transaction;
      IDBDatabase.prototype.transaction = function (...args: Parameters<IDBDatabase['transaction']>) {
        if (args[1] === 'readwrite') throw new DOMException('Storage full', 'QuotaExceededError');
        return transaction.apply(this, args);
      };
    });
    await edit(page);
    await expect(page.locator('#temporaryStatus')).toContainText('Storage unavailable');
    await expect(page.locator('#saveState')).toHaveText('Temporary');
    const library = await context.newPage();
    try {
      await open(library, original);
      expect(await source(library)).toBe(DOC);
    } finally {
      await library.close();
    }
  });

  for (const kind of ['full', 'short']) {
    test(`${kind} temporary shares restore the mode and never persist short-link secrets`, async ({
      page,
      browser,
    }) => {
      await page.addInitScript(() =>
        Object.defineProperty(navigator, 'clipboard', {
          value: {
            writeText: async (text: string) => {
              document.documentElement.dataset.temporaryLink = text;
            },
          },
        }),
      );
      await open(page, link(DOC, true));
      await page.getByRole('button', { name: 'Share', exact: true }).click();
      await page.getByRole('menuitem', { name: kind === 'full' ? /^Copy full link/ : /^Copy short link/ }).click();
      await expect.poll(() => page.locator('html').getAttribute('data-temporary-link')).toContain('t=1');
      const address = (await page.locator('html').getAttribute('data-temporary-link'))!;
      expect(await page.evaluate(() => localStorage.getItem('mars.shortLinks'))).toBeNull();
      const fresh = await browser.newContext({ baseURL: MARS_BASE_URL });
      try {
        const reopened = await fresh.newPage();
        await open(reopened, address);
        await expect(reopened.locator('#temporarySession')).toBeVisible();
        expect(await source(reopened)).toBe(DOC);
      } finally {
        await fresh.close();
      }
    });
  }
});
