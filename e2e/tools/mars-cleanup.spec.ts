import { randomBytes } from 'node:crypto';

import { expect } from '@playwright/test';

import { recordSnapshotCanary, scopeRequest } from '../mars-test-scopes';
import { copyLink } from './_mars_sharing';
import { test } from './_mars_test';

const baseURL = process.env.MARS_BASE_URL ?? 'https://mars.secutils.dev';
test.use({ baseURL });

test('sync protocol test teardown deletes scoped Sync and snapshots without touching an ordinary short link', async ({
  page,
  browser,
  request,
  marsTestScope,
}) => {
  const { manifest, path } = marsTestScope;
  const canaryId = randomBytes(12).toString('base64url'),
    deleteToken = randomBytes(16).toString('base64url');
  const canaryUrl = `${baseURL}/?s=${canaryId}`,
    canary = Buffer.alloc(32, 17).toString('base64url');
  recordSnapshotCanary(path, canaryId, deleteToken);
  const created = await request.post(canaryUrl, { headers: { 'x-su-delete-token': deleteToken }, data: canary });
  expect(created.status()).toBe(201);
  try {
    await page.goto('/');
    await page.waitForFunction(() => !!(window as { __suEditorAPI?: unknown }).__suEditorAPI);
    await page.locator('#fileInput').setInputFiles({
      name: 'Cleanup.md',
      mimeType: 'text/markdown',
      buffer: Buffer.from('# Cleanup\n\nSynthetic test content.'),
    });
    await expect(page.locator('#saveState')).toHaveText('Saved');
    await page
      .getByRole('treeitem', { selected: true })
      .getByRole('button', { name: /^Actions for/ })
      .click();
    await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
    await expect(page.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible({ timeout: 15000 });
    await page.getByRole('button', { name: 'Close', exact: true }).click();
    const posted = page.waitForRequest(
      (request) => request.method() === 'POST' && new URL(request.url()).searchParams.has('s'),
    );
    const shared = await copyLink(page, { format: 'short' });
    expect(shared).not.toContain(manifest.token);
    expect(shared).not.toContain(manifest.scope);
    const recipientContext = await browser.newContext({ baseURL });
    const recipient = await recipientContext.newPage();
    await recipient.goto(shared);
    await expect(recipient.locator('#previewArticle')).toContainText('Synthetic test content.');
    const id = new URL((await posted).url()).searchParams.get('s')!;
    const scopedUrl = `${baseURL}/?s=${id}`;
    expect((await request.get(scopedUrl)).status()).toBe(404);
    const headers = { 'x-mars-test-scope': manifest.scope, 'x-mars-test-token': manifest.token };
    expect((await request.get(scopedUrl, { headers })).status()).toBe(200);
    await recipientContext.close();
    await page.goto('about:blank');
    let removed = 0,
      complete = false;
    for (let batch = 0; batch < 100 && !complete; batch++) {
      const result = await scopeRequest(manifest.origin, {
        operation: 'close',
        scope: manifest.scope,
        run: manifest.run,
        test: manifest.test,
      });
      removed = result.removed ?? 0;
      complete = result.complete === true;
    }
    expect(complete).toBe(true);
    expect(removed).toBeGreaterThan(10);
    expect((await request.get(scopedUrl, { headers })).status()).toBe(410);
    expect((await request.get(canaryUrl)).status()).toBe(200);
    expect((await (await request.get(canaryUrl)).json()).v).toBe(canary);
  } finally {
    const deleted = await request.delete(canaryUrl, { headers: { 'x-su-delete-token': deleteToken } });
    expect(deleted.status()).toBe(204);
  }
});
