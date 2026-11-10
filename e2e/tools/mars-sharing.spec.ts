import { deflateRawSync, inflateRawSync } from 'node:zlib';

import { type Browser, expect, type Page } from '@playwright/test';

import { configureCopyLink, copyLink } from './_mars_sharing';
import { test } from './_mars_test';

const baseURL = process.env.MARS_BASE_URL ?? 'http://localhost:7373';
test.use({ baseURL });
async function clients(browser: Browser, count = 2) {
  const key = crypto.randomUUID(),
    contexts = await Promise.all(Array.from({ length: count }, () => browser.newContext()));
  for (const context of contexts) {
    await context.addInitScript(() =>
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: {
          write: async (items: ClipboardItem[]) => {
            document.documentElement.dataset.copiedLink = await (await items[0].getType('text/plain')).text();
          },
          writeText: async (text: string) => {
            document.documentElement.dataset.copiedLink = text;
          },
        },
      }),
    );
    await context.route(`${new URL(baseURL).origin}/**`, (route) =>
      route.continue({ headers: { ...route.request().headers(), 'x-mars-test-client': key } }),
    );
  }
  const pages = await Promise.all(contexts.map((context) => context.newPage()));
  return {
    pages,
    close: async () => {
      await Promise.all(pages.map((page) => page.goto('about:blank').catch(() => {})));
      await Promise.all(contexts.map((context) => context.close()));
    },
  };
}
async function documentAction(page: Page, action: string) {
  await page
    .getByRole('treeitem', { selected: true })
    .getByRole('button', { name: /^Actions for/ })
    .click();
  await page.getByRole('menuitem', { name: action, exact: true }).click();
}
async function edit(page: Page, text: string) {
  await page.getByRole('button', { name: 'Source', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Markdown source', exact: true })).toBeVisible();
  await page.evaluate(
    (text) => (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(text),
    text,
  );
  await page.getByRole('button', { name: 'Preview', exact: true }).click();
}
test.describe('sync user experience sharing', () => {
  test('links require separate wrapping keys before requests and saved unsupported replicas retain local content', async ({
    browser,
  }) => {
    const c = await clients(browser),
      [owner, guest] = c.pages;
    try {
      await owner.goto(baseURL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.locator('#fileInput').setInputFiles({
        name: 'Keys.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from('# Keys\n\nRetained private content.'),
      });
      await expect(owner.locator('#previewArticle')).toContainText('Retained private content.');
      const link = await copyLink(owner, { mode: 'live', format: 'full' });
      const encoded = new URLSearchParams(new URL(link).hash.slice(1)).get('live')!;
      const payload = JSON.parse(inflateRawSync(Buffer.from(encoded, 'base64url').subarray(4)).toString());
      let requests = 0;
      guest.on('request', (request) => {
        if (new URL(request.url()).pathname.startsWith('/sync/')) requests++;
      });
      for (const kind of ['live', 'sync'])
        for (const mode of ['missing', 'reused']) {
          const value = { ...payload };
          if (mode === 'missing') delete value.wrappingKey;
          else value.wrappingKey = value.secret;
          const raw = Buffer.from(JSON.stringify(value)),
            bytes = Buffer.concat([Buffer.alloc(4), deflateRawSync(raw)]);
          bytes.writeUInt32LE(raw.length, 0);
          await guest.goto('about:blank');
          requests = 0;
          await guest.goto(`${baseURL}/?temporary=1#${kind}=${bytes.toString('base64url')}`);
          await expect(guest.locator('#toast')).toContainText('Unsupported link key format');
          expect(requests).toBe(0);
        }
      await guest.goto('about:blank');
      await guest.goto(link);
      await expect(guest.locator('#previewArticle')).toContainText('Retained private content.');
      await guest.getByRole('button', { name: 'Save live document to library', exact: true }).click();
      await guest
        .getByRole('dialog', { name: 'Save live document to library', exact: true })
        .getByRole('button', { name: 'Save to library', exact: true })
        .click();
      await expect(guest.locator('#temporarySession')).toBeHidden();
      await expect(guest).toHaveURL(/#doc=[\w-]+$/);
      const saved = guest.url();
      // Stop the live app before changing its stored fixture. A pending activity
      // renewal can otherwise save its in-memory scope over this test mutation.
      await guest.goto('about:blank');
      const storagePage = new URL('/?mars-storage-test=1', baseURL).href;
      await guest.route(
        storagePage,
        (route) => route.fulfill({ contentType: 'text/html', body: '<!doctype html><title>Storage fixture</title>' }),
        { times: 1 },
      );
      await guest.goto(storagePage);
      const changed = await guest.evaluate(
        () =>
          new Promise<number>((resolve, reject) => {
            const open = indexedDB.open('mars');
            open.onerror = () => reject(open.error);
            open.onsuccess = () => {
              const db = open.result,
                tx = db.transaction('meta', 'readwrite'),
                store = tx.objectStore('meta'),
                cursor = store.openCursor();
              let changed = 0;
              cursor.onsuccess = () => {
                const entry = cursor.result;
                if (!entry) return;
                if (String(entry.key).startsWith('sync:scope:') && entry.value.live) {
                  const value = entry.value;
                  delete value.live.wrappingKey;
                  entry.update(value);
                  changed++;
                }
                entry.continue();
              };
              tx.oncomplete = () => {
                db.close();
                resolve(changed);
              };
              tx.onerror = () => {
                db.close();
                reject(tx.error);
              };
            };
          }),
      );
      expect(changed).toBe(1);
      await guest.goto('about:blank');
      requests = 0;
      await guest.goto(saved);
      await expect(guest.locator('#previewArticle')).toContainText('Retained private content.');
      await expect(guest.locator('#saveState')).toContainText('Needs attention');
      expect(requests).toBe(0);
    } finally {
      await c.close();
    }
  });

  test('Plausible keeps pageviews and engagement without document URLs or link secrets', async ({ page, request }) => {
    const response = await request.get(baseURL);
    expect(response.ok()).toBe(true);
    const html = await response.text();
    const origin = 'https://mars.analytics.test';
    const events: Record<string, unknown>[] = [];
    const scripts: string[] = [];
    await page.addInitScript(() => {
      (window as unknown as { __plausible: boolean }).__plausible = true;
      Object.defineProperty(document, 'hasFocus', { value: () => false });
      Object.defineProperty(document, 'referrer', { value: 'https://referral.example/private?token=private-referrer' });
    });
    await page.route('**/*', async (route) => {
      const req = route.request();
      const url = new URL(req.url());
      if (req.resourceType() === 'script') scripts.push(url.href);
      if (url.protocol === 'blob:') {
        await route.continue();
      } else if (url.href === 'https://tools.secutils.dev/api/event') {
        expect(req.headers().referer).toBeUndefined();
        events.push(req.postDataJSON());
        await route.fulfill({ status: 202, headers: { 'Access-Control-Allow-Origin': '*' }, body: '' });
      } else if (url.origin === origin && req.isNavigationRequest()) {
        await route.fulfill({
          contentType: 'text/html',
          headers: { 'Cross-Origin-Opener-Policy': 'same-origin', 'Cross-Origin-Embedder-Policy': 'credentialless' },
          body: html,
        });
      } else if (url.origin === origin) {
        await route.fulfill({ status: 410, contentType: 'application/json', body: '{"error":"test-link-expired"}' });
      } else {
        await route.abort();
      }
    });
    for (const fragment of [
      's=private-snapshot-secret',
      'live=private-live-secret',
      'sync=private-sync-secret',
      'invite=private-legacy-secret',
      'name=Private-name&md=private-document',
      'b=private-folder',
      'doc=private-local-id',
    ]) {
      await page.goto('about:blank');
      events.length = 0;
      await page.goto(`${origin}/?temporary=1&url=https://source.example/private#${fragment}`, {
        referer: 'https://referral.example/private?token=private-referrer',
      });
      await expect.poll(() => events.filter((event) => event.n === 'pageview').length).toBe(1);
      await page.evaluate(() => window.dispatchEvent(new Event('blur')));
      await expect.poll(() => events.some((event) => event.n === 'engagement')).toBe(true);
      for (const event of events) {
        expect(event.u).toBe(`${origin}/`);
        expect(event.d).toBe('mars.analytics.test');
        if (event.n === 'pageview') expect(event.r).toBe('https://referral.example/');
        expect(JSON.stringify(event)).not.toMatch(/private|source\.example/);
        expect(Object.keys(event).every((key) => ['n', 'u', 'd', 'r', 'v', 'e', 'sd'].includes(key))).toBe(true);
      }
      expect(await page.evaluate(() => 'plausible' in window)).toBe(false);
    }
    expect(scripts.some((url) => url.includes('/js/script'))).toBe(false);
  });

  test('Share help and wider Privacy/Credits dialogs remain readable on desktop and phones', async ({ page }) => {
    await page.goto(baseURL);
    await expect(page.locator('#emptyState')).toBeVisible();
    await page.locator('#fileInput').setInputFiles({
      name: 'Help.md',
      mimeType: 'text/markdown',
      buffer: Buffer.from('# Help\n\nA local document.'),
    });
    await expect(page.locator('#previewArticle')).toContainText('A local document.');
    await page.getByRole('button', { name: 'Share', exact: true }).click();
    const copy = page.getByRole('menuitem', { name: 'Copy link', exact: true });
    await expect(copy).toHaveAccessibleDescription('Share a copy or keep others up to date.');
    await expect(copy).toContainText('Share a copy or keep others up to date.');
    await page.getByRole('button', { name: 'Share', exact: true }).click();
    for (const width of [1280, 320])
      for (const theme of ['light', 'dark']) {
        await page.setViewportSize({ width, height: 900 });
        await page.evaluate((theme) => {
          document.documentElement.dataset.theme = theme;
        }, theme);
        if (width === 320 && theme === 'light')
          await expect(page.locator('#navToggle')).toHaveAttribute('aria-expanded', 'false');
        if (!(await page.getByRole('button', { name: 'Privacy', exact: true }).isVisible()))
          await page.getByRole('button', { name: 'Toggle documents sidebar', exact: true }).click();
        for (const name of ['Privacy', 'Credits']) {
          await page.getByRole('button', { name, exact: true }).click();
          const dialog = page.getByRole('dialog', { name, exact: true });
          if (name === 'Privacy') {
            await expect(dialog).toContainText('End-to-end encryption.');
            await expect(dialog).toContainText('Secutils.dev has no master decryption key or recovery key.');
            await expect(dialog).toContainText('unencrypted');
            await expect(dialog).toContainText('MARS does not encrypt this local library');
            await expect(dialog).toContainText('MARS redacts event URLs');
            await expect(dialog).toContainText('malicious page code');
            await expect(dialog).not.toContainText('Earlier analytics versions');
            await expect(dialog).not.toContainText('older links');
            await expect(dialog).not.toContainText('Unsupported saved connections');
          }
          await expect(dialog).toHaveCSS('max-width', '960px');
          const box = (await dialog.boundingBox())!;
          expect(box.width).toBeCloseTo(width === 1280 ? 960 : 288, 0);
          expect(box.x).toBeGreaterThanOrEqual(15);
          expect(box.x + box.width).toBeLessThanOrEqual(width - 15);
          expect(box.y).toBeGreaterThanOrEqual(15);
          expect(box.y + box.height).toBeLessThanOrEqual(885);
          expect(await dialog.evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
          await dialog.getByRole('button', { name: 'Close', exact: true }).click();
        }
      }
  });

  test('active recipients forward unchanged Live links through Copy link, while ended links offer snapshots only', async ({
    browser,
  }) => {
    test.setTimeout(120000);
    const c = await clients(browser, 3),
      [owner, guest, probe] = c.pages;
    let created = 0;
    for (const page of c.pages)
      page.on('request', (request) => {
        if (new URL(request.url()).pathname === '/sync/invite') created++;
      });
    try {
      await owner.goto(baseURL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.locator('#fileInput').setInputFiles({
        name: 'Forward.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from('# Forward\n\nOriginal Live content.'),
      });
      await expect(owner.locator('#previewArticle')).toContainText('Original Live content.');
      for (const format of ['short', 'full'] as const)
        for (const permission of ['view', 'comment', 'edit'] as const) {
          const original = await copyLink(owner, { mode: 'live', format, permission, ttl: '86400' });
          await Promise.all([guest, probe].map((page) => page.goto('about:blank')));
          await guest.goto(original);
          await expect(guest.locator('#previewArticle')).toContainText('Original Live content.', { timeout: 15000 });
          await documentAction(guest, 'Live document');
          const details = guest.getByRole('dialog', { name: 'Live document', exact: true });
          await expect(details.getByRole('button', { name: 'Copy original link', exact: true })).toHaveCount(0);
          await details.getByRole('button', { name: 'Close', exact: true }).click();
          const before = created,
            forwarded = await copyLink(guest, { mode: 'live', close: false });
          expect(forwarded).toBe(original);
          expect(created).toBe(before);
          const dialog = guest.getByRole('dialog', { name: 'Copy link', exact: true });
          await expect(dialog).toContainText("Forward the owner's Live link. Access and expiry stay unchanged.");
          await expect(dialog.locator('#copyForwardInfo')).toContainText(
            permission === 'view' ? 'View only' : `Can ${permission}`,
          );
          await expect(dialog.locator('#copyForwardInfo')).toContainText('Expires');
          await expect(dialog.locator('#copyFormatFields')).toBeHidden();
          await expect(dialog.locator('#copyAccessFields')).toBeHidden();
          await expect(dialog.getByLabel('Expires', { exact: true })).toBeHidden();
          await expect(dialog.getByRole('button', { name: 'Live links', exact: true })).toBeHidden();
          await dialog.getByRole('button', { name: 'Close', exact: true }).click();
          await probe.goto(forwarded);
          await expect(probe.locator('#previewArticle')).toContainText('Original Live content.', { timeout: 15000 });
          await expect(probe.locator('#documentAccess')).toHaveText(
            permission === 'view' ? 'View only' : `Can ${permission}`,
          );
        }
      expect(created).toBe(6);
      await probe.goto('about:blank');
      await copyLink(guest, { mode: 'live', close: false });
      const open = guest.getByRole('dialog', { name: 'Copy link', exact: true });
      await owner.bringToFront();
      await owner.getByRole('button', { name: 'Share', exact: true }).click();
      await owner.getByRole('menuitem', { name: 'Copy link', exact: true }).click();
      await owner
        .getByRole('dialog', { name: 'Copy link', exact: true })
        .getByRole('button', { name: 'Live links', exact: true })
        .click();
      await owner.getByRole('button', { name: 'Disable link 6', exact: true }).click();
      await owner.getByRole('button', { name: 'Disable link', exact: true }).click();
      await expect(owner.getByRole('dialog', { name: 'Live links', exact: true })).toBeVisible({ timeout: 30000 });
      await guest.bringToFront();
      await expect(open.getByRole('radio', { name: 'Live', exact: true })).toBeDisabled({ timeout: 15000 });
      await expect(open.getByRole('button', { name: 'Copy link', exact: true })).toBeDisabled();
      await expect(open.getByRole('textbox', { name: 'Link', exact: true })).toBeHidden();
      await expect(open).toContainText('The Live link ended. Choose Snapshot');
      await open.getByRole('radio', { name: 'Snapshot', exact: true }).check();
      await expect(open.getByRole('button', { name: 'Copy link', exact: true })).toBeEnabled();
      await open.getByRole('button', { name: 'Close', exact: true }).click();
      await documentAction(guest, 'Live document');
      const ended = guest.getByRole('dialog', { name: 'Live document', exact: true });
      await expect(ended).toContainText('The Live link ended');
      await expect(ended.getByRole('button', { name: 'Copy original link', exact: true })).toHaveCount(0);
      await ended.getByRole('button', { name: 'Close', exact: true }).click();
      const snapshot = await copyLink(guest, { format: 'full', close: false });
      expect(new URL(snapshot).hash).toMatch(/^#name=/);
      await expect(
        guest.getByRole('dialog', { name: 'Copy link', exact: true }).getByRole('radio', { name: 'Live', exact: true }),
      ).toBeDisabled();
      expect(created).toBe(6);
    } finally {
      await c.close();
    }
  });

  test('view and comment access hide mutation controls and show a persistent document-access badge', async ({
    browser,
  }) => {
    test.setTimeout(120000);
    const c = await clients(browser),
      [owner, guest] = c.pages;
    try {
      await owner.goto(baseURL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.locator('#fileInput').setInputFiles({
        name: 'Access.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from('# Access\n\nShared paragraph.\n\n- [ ] Task\n'),
      });
      await expect(owner.locator('#docTitle')).toHaveText('Access');
      for (const permission of ['view', 'comment', 'edit'] as const) {
        await guest.goto(await copyLink(owner, { mode: 'live', permission }));
        await expect(guest.locator('#previewArticle')).toContainText('Shared paragraph.', { timeout: 15000 });
        await expect(guest.locator('#documentAccess')).toHaveText(
          permission === 'view' ? 'View only' : `Can ${permission}`,
        );
        await guest.locator('#previewArticle').getByText('Shared paragraph.', { exact: true }).hover();
        if (permission !== 'edit') {
          await expect(guest.locator('#blockHandle')).toBeHidden();
          await guest.locator('#blockGripBtn').evaluate((button: HTMLButtonElement) => button.click());
          await expect(guest.locator('#blockMenu')).toBeHidden();
          await expect(guest.locator('#previewArticle input[type="checkbox"]')).toBeDisabled();
        } else await expect(guest.locator('#previewArticle input[type="checkbox"]')).toBeEnabled();
        await guest.getByRole('button', { name: 'Save live document to library', exact: true }).click();
        await guest.getByRole('button', { name: 'Save to library', exact: true }).click();
        await guest
          .locator('#tree .tree-link[aria-current="page"]')
          .locator('..')
          .getByRole('button', { name: 'Actions for Access', exact: true })
          .click();
        await guest.getByRole('menuitem', { name: 'Live document', exact: true }).click();
        const info = guest.getByRole('dialog', { name: 'Live document', exact: true });
        await expect(info.getByRole('button', { name: 'Disconnect this device', exact: true })).toHaveCount(0);
        await expect(info.getByRole('button', { name: 'Keep as local copy', exact: true })).toBeVisible();
        await info.getByRole('button', { name: 'Close', exact: true }).click();
      }
    } finally {
      await c.close();
    }
  });
  test('one Copy link dialog handles snapshot and Live formats without copy feedback moving the dialog', async ({
    page,
    context,
  }) => {
    await context.addInitScript(() =>
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: { write: async () => {}, writeText: async () => {} },
      }),
    );
    await page.goto(baseURL);
    await expect(page.locator('#emptyState')).toBeVisible();
    await page.locator('#fileInput').setInputFiles({
      name: 'Share.md',
      mimeType: 'text/markdown',
      buffer: Buffer.from('# Share\n\nA shared document.'),
    });
    await expect(page.locator('#docTitle')).toHaveText('Share');
    await page.getByRole('button', { name: 'Share', exact: true }).click();
    await expect(page.getByRole('menuitem', { name: 'Copy full link', exact: true })).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Snapshot', exact: true })).toHaveCount(0);
    await page.getByRole('menuitem', { name: 'Copy link', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Copy link', exact: true });
    await expect(dialog).not.toContainText(/encrypt|decryption|credentials/i);
    await expect(dialog.getByRole('radio', { name: 'Snapshot', exact: true })).toBeChecked();
    await expect(dialog.getByRole('radio', { name: 'Full', exact: true })).toBeChecked();
    expect(
      await dialog
        .locator('input[name="copyFormat"]')
        .evaluateAll((inputs) => inputs.map((input) => (input as HTMLInputElement).value)),
    ).toEqual(['full', 'short']);
    await expect(dialog.getByRole('textbox', { name: 'Link', exact: true })).toBeHidden();
    await expect(dialog).toContainText('Click Copy link to create and copy your link.');
    await expect(dialog.getByLabel('Expires', { exact: true })).toBeHidden();
    await expect(dialog.getByRole('button', { name: 'Live links', exact: true })).toBeHidden();
    const before = await dialog.boundingBox();
    const full = await configureCopyLink(page, { format: 'full', close: false });
    expect(new URL(full).hash).toMatch(/^#name=/);
    await expect(dialog.getByRole('textbox', { name: 'Link', exact: true })).toBeVisible();
    await expect(dialog.locator('#copyLinkHelp')).toBeHidden();
    expect(await dialog.boundingBox()).toEqual(before);
    await expect(page.locator('.su-tooltip')).toHaveText('Copied');
    await dialog.getByRole('radio', { name: 'Short', exact: true }).check();
    await expect(dialog.getByRole('textbox', { name: 'Link', exact: true })).toBeHidden();
    await expect(page.locator('.su-tooltip')).toHaveCount(0);
    const expiry = dialog.getByLabel('Expires', { exact: true });
    expect(await expiry.locator('option').allTextContents()).toEqual(['1 day', '7 days', '14 days', '30 days']);
    const shortBefore = await dialog.boundingBox();
    const short = await configureCopyLink(page, { format: 'short', ttl: '2592000', close: false });
    expect(new URL(short).hash).toMatch(/^#s=/);
    expect(await dialog.boundingBox()).toEqual(shortBefore);
    const exp = await page.evaluate((secret) => {
      const records = JSON.parse(localStorage.getItem('mars.shortLinks')!) as Record<
        string,
        { secret: string; exp: number; ttl: number }
      >;
      return Object.values(records).find((link) => link.secret === secret)!;
    }, new URL(short).hash.slice(3));
    expect(exp.ttl).toBe(2592000);
    expect(exp.exp - Date.now() / 1000).toBeGreaterThan(2592000 - 10);
    const live = await configureCopyLink(page, { mode: 'live', format: 'full', ttl: '86400', close: false });
    expect(await expiry.locator('option').allTextContents()).toEqual(['1 day', '7 days', '14 days', '30 days']);
    expect(new URL(live).hash).toMatch(/^#live=/);
    expect(new URL(live).hash.length).toBeGreaterThan(100);
    expect(await configureCopyLink(page, { mode: 'live', format: 'full', ttl: '86400', close: false })).toBe(live);
    const manage = dialog.getByRole('button', { name: 'Live links', exact: true });
    await expect(manage).toBeVisible();
    const manageBox = (await manage.boundingBox())!,
      copyBox = (await dialog.getByRole('button', { name: 'Copy link', exact: true }).boundingBox())!;
    expect(manageBox.x).toBeLessThan(copyBox.x);
    await manage.click();
    await expect(page.locator('#copyLinkDialog')).toHaveCount(0);
    const links = page.getByRole('dialog', { name: 'Live links', exact: true });
    await expect(links).toBeVisible();
    await expect(links.locator('.sync-access-item')).toHaveCount(1);
    await links.getByRole('button', { name: 'Copy link', exact: true }).click();
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Live links', exact: true }).click();
    await expect(links).toHaveCount(1);
    await expect(page.locator('#copyLinkDialog')).toHaveCount(0);
    await links.getByRole('button', { name: 'Close', exact: true }).click();
    await page.getByRole('button', { name: 'Share', exact: true }).click();
    await expect(page.getByRole('menuitem', { name: 'Live links', exact: true })).toHaveCount(0);
    await page.getByRole('button', { name: 'Share', exact: true }).click();
    await page.getByRole('button', { name: 'Actions for Share', exact: true }).click();
    await expect(page.getByRole('menuitem', { name: 'Live links', exact: true })).toHaveCount(0);
    await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Turn on sync', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Stop sharing', exact: true })).toHaveCount(0);
  });
  test('a blocked clipboard shows the generated link for manual copying', async ({ page, context }) => {
    await context.addInitScript(() =>
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: {
          write: async () => {
            throw new Error('Clipboard blocked');
          },
          writeText: async () => {
            throw new Error('Clipboard blocked');
          },
        },
      }),
    );
    await page.goto(baseURL);
    await expect(page.locator('#emptyState')).toBeVisible();
    await page.locator('#fileInput').setInputFiles({
      name: 'Manual.md',
      mimeType: 'text/markdown',
      buffer: Buffer.from('# Manual\n\nCopy this document.'),
    });
    await expect(page.locator('#previewArticle')).toContainText('Copy this document.');
    const link = await copyLink(page, { format: 'full', close: false });
    expect(new URL(link).hash).toMatch(/^#name=/);
    const dialog = page.getByRole('dialog', { name: 'Copy link', exact: true });
    await expect(dialog.getByRole('textbox', { name: 'Link', exact: true })).toBeVisible();
    await expect(dialog).toContainText('Could not copy. Select the link above and copy it manually.');
    await expect(dialog.getByRole('button', { name: 'Copy link', exact: true })).toBeEnabled();
    await expect(dialog.getByRole('radio', { name: 'Live', exact: true })).toBeEnabled();
    await expect(dialog.getByRole('button', { name: 'Live links', exact: true })).toBeHidden();
  });
  test('recipient panel saves then detaches a local copy without disabling the original link', async ({ browser }) => {
    test.setTimeout(90000);
    const c = await clients(browser, 3),
      [owner, guest, other] = c.pages;
    try {
      await owner.goto(baseURL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.locator('#fileInput').setInputFiles({
        name: 'Independent.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from('# Independent\n\nOriginal content.'),
      });
      await expect(owner.locator('#previewArticle')).toContainText('Original content.');
      const link = await copyLink(owner, { mode: 'live' });
      await guest.goto(link);
      await expect(guest.locator('#previewArticle')).toContainText('Original content.', { timeout: 15000 });
      await documentAction(guest, 'Live document');
      let info = guest.getByRole('dialog', { name: 'Live document', exact: true });
      await expect(info.getByRole('button', { name: 'Copy original link', exact: true })).toHaveCount(0);
      await info.getByRole('button', { name: 'Close', exact: true }).click();
      expect(await copyLink(guest, { mode: 'live' })).toBe(link);
      await expect(guest.locator('html')).toHaveAttribute('data-copied-link', link);
      await documentAction(guest, 'Live document');
      info = guest.getByRole('dialog', { name: 'Live document', exact: true });
      await info.getByRole('button', { name: 'Save to library', exact: true }).click();
      await guest
        .getByRole('dialog', { name: 'Save live document to library', exact: true })
        .getByRole('button', { name: 'Save to library', exact: true })
        .click();
      await expect(guest.locator('#temporarySession')).toBeHidden();
      info = guest.getByRole('dialog', { name: 'Live document', exact: true });
      await info.getByRole('button', { name: 'Keep as local copy', exact: true }).click();
      await guest.getByRole('button', { name: 'Keep local copy', exact: true }).click();
      await expect(guest.locator('#liveDocumentDialog')).toHaveCount(0);
      await expect(guest.locator('#tree [data-connection]')).toHaveCount(0);
      await expect(guest.locator('#documentAccess')).toBeHidden();
      await edit(owner, '# Independent\n\nLater shared content.');
      await other.goto(link);
      await expect(other.locator('#previewArticle')).toContainText('Later shared content.', { timeout: 15000 });
      await guest.reload();
      await expect(guest.locator('#previewArticle')).toContainText('Original content.');
      await expect(guest.locator('#previewArticle')).not.toContainText('Later shared content.');
    } finally {
      await c.close();
    }
  });
  test('personal Sync disconnection leaves paired devices and Live links connected', async ({ browser }) => {
    test.setTimeout(90000);
    const c = await clients(browser, 3),
      [owner, peer, guest] = c.pages;
    try {
      await owner.goto(baseURL);
      await expect(owner.locator('#emptyState')).toBeVisible();
      await owner.locator('#fileInput').setInputFiles({
        name: 'Separate.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from('# Separate\n\nOriginal content.'),
      });
      await expect(owner.locator('#previewArticle')).toContainText('Original content.');
      const live = await copyLink(owner, { mode: 'live' });
      await guest.goto(live);
      await expect(guest.locator('#previewArticle')).toContainText('Original content.', { timeout: 15000 });
      await documentAction(owner, 'Sync');
      await owner.getByRole('button', { name: 'Turn on sync', exact: true }).click();
      const invitation = owner.getByRole('textbox', { name: 'Sync link', exact: true });
      await expect(invitation).toBeVisible({ timeout: 15000 });
      await peer.goto(await invitation.inputValue());
      await peer.getByRole('textbox', { name: 'Name', exact: true }).fill('Other device');
      await peer.getByRole('button', { name: 'Connect', exact: true }).click();
      await expect(owner.getByRole('button', { name: 'Codes match', exact: true })).toBeVisible({ timeout: 15000 });
      expect(await owner.getByRole('group', { name: 'Confirmation code', exact: true }).textContent()).toBe(
        await peer.getByRole('group', { name: 'Confirmation code', exact: true }).textContent(),
      );
      await owner.getByRole('button', { name: 'Codes match', exact: true }).click();
      await expect(owner.getByRole('dialog', { name: 'Sync', exact: true })).toBeVisible({ timeout: 15000 });
      await owner.getByRole('button', { name: 'Close', exact: true }).click();
      await expect(peer.locator('#previewArticle')).toContainText('Original content.', { timeout: 15000 });
      await documentAction(peer, 'Sync');
      await expect(peer.getByRole('dialog', { name: 'Sync', exact: true })).toBeVisible();
      await peer.getByRole('button', { name: 'Close', exact: true }).click();
      await documentAction(owner, 'Sync');
      await owner.getByRole('button', { name: 'Disconnect this device', exact: true }).click();
      await expect(owner.getByRole('dialog', { name: 'Disconnect this device?', exact: true })).toContainText(
        'active Live links remain connected',
      );
      await owner.getByRole('button', { name: 'Disconnect', exact: true }).click();
      await expect(owner.getByRole('button', { name: 'Turn on sync', exact: true })).toBeVisible();
      await owner.getByRole('button', { name: 'Close', exact: true }).click();
      await expect(owner.locator('#tree [data-connection]')).toHaveCount(0);
      await edit(peer, '# Separate\n\nPaired device changed it.');
      await expect(guest.locator('#previewArticle')).toContainText('Paired device changed it.', { timeout: 15000 });
      await owner.reload();
      await expect(owner.locator('#previewArticle')).toContainText('Original content.');
      await expect(owner.locator('#previewArticle')).not.toContainText('Paired device changed it.');
    } finally {
      await c.close();
    }
  });
  test('folder menus use Copy link and Download, and do not offer unsupported Live-folder links', async ({ page }) => {
    await page.goto(baseURL);
    await expect(page.locator('#emptyState')).toBeVisible();
    await page.getByRole('button', { name: 'New document, folder, or import' }).click();
    await page.getByRole('menuitem', { name: 'New folder', exact: true }).click();
    await page.getByRole('textbox', { name: 'Name', exact: true }).fill('Folder');
    await page.getByRole('textbox', { name: 'Name', exact: true }).press('Enter');
    await page.getByRole('button', { name: 'Actions for Folder', exact: true }).click();
    await expect(page.getByRole('menuitem', { name: 'Download', exact: true })).toBeVisible();
    await expect(page.getByRole('menuitem', { name: 'Download .zip', exact: true })).toHaveCount(0);
    await page.getByRole('menuitem', { name: 'Copy link', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Copy link', exact: true });
    await expect(dialog.getByRole('radio', { name: 'Live', exact: true })).toBeDisabled();
  });
});
