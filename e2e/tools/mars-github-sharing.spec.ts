import { createHash } from 'node:crypto';
import { inflateRawSync } from 'node:zlib';

import { expect } from '@playwright/test';

import { configureCopyLink } from './_mars_sharing';
import { test } from './_mars_test';

const baseURL = process.env.MARS_BASE_URL ?? 'http://localhost:7373';
test.use({ baseURL });
test.skip(
  !process.env.MARS_BASE_URL ||
    !['localhost', '127.0.0.1', '[::1]', 'mars-dev.secutils.dev'].includes(new URL(baseURL).hostname),
  'GitHub preview requires an explicitly selected local or dev host',
);

test('GitHub snapshot links omit parent folders by default and use concise Full and Short help', async ({
  page,
  browser,
}) => {
  const commit = '1'.repeat(40);
  const tree = '2'.repeat(40);
  const files = new Map([
    ['Confidential/Guide.md', '# Guide\n\nA shared guide.\n'],
    ['Outside.md', '# Outside\n\nAn unrelated document.\n'],
  ]);
  const headers = { 'access-control-allow-origin': '*' };
  await page.route('https://api.github.com/**', (route) => {
    const request = route.request();
    expect(request.method()).toBe('GET');
    expect(request.headers().authorization).toBeUndefined();
    const path = new URL(request.url()).pathname;
    if (path === '/repos/example/notes')
      return route.fulfill({ json: { full_name: 'example/notes', default_branch: 'main', private: false }, headers });
    if (path.endsWith('/branches/main'))
      return route.fulfill({ json: { commit: { sha: commit, commit: { tree: { sha: tree } } } }, headers });
    if (path.includes('/trees/'))
      return route.fulfill({
        headers,
        json: {
          tree: [...files].map(([path, text]) => ({
            path,
            type: 'blob',
            mode: '100644',
            size: Buffer.byteLength(text),
            sha: createHash('sha1')
              .update(`blob ${Buffer.byteLength(text)}\0${text}`)
              .digest('hex'),
          })),
        },
      });
    if (path.includes('/blobs/')) {
      const text = [...files.values()].find((text) =>
        path.endsWith(
          createHash('sha1')
            .update(`blob ${Buffer.byteLength(text)}\0${text}`)
            .digest('hex'),
        ),
      )!;
      return route.fulfill({
        json: { sha: path.split('/').at(-1), encoding: 'base64', content: Buffer.from(text).toString('base64') },
        headers,
      });
    }
    return route.abort();
  });
  await page.route(`https://raw.githubusercontent.com/example/notes/${commit}/**`, (route) => {
    const path = new URL(route.request().url()).pathname.split('/').slice(4).join('/');
    return route.fulfill({ body: files.get(path)!, headers });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'New document, folder, or import' }).click();
  await page.getByRole('menuitem', { name: 'Import from GitHub…' }).click();
  const importer = page.getByRole('dialog', { name: 'Import from GitHub' });
  await importer.getByLabel('Repository (required)').fill('example/notes');
  await importer.getByRole('button', { name: 'Import folder' }).click();
  await expect(importer).not.toBeVisible();
  await page.getByRole('link', { name: 'Guide', exact: true }).click();
  for (const kind of ['document', 'folder'] as const) {
    await page
      .getByRole('button', { name: `Actions for ${kind === 'document' ? 'Guide' : 'Confidential'}`, exact: true })
      .click();
    await page.getByRole('menuitem', { name: 'Copy link', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Copy link', exact: true });
    const subject = kind === 'document' ? 'this document' : 'this folder and its contents';
    const parents = dialog.getByRole('checkbox', { name: 'Include parent folders', exact: true });
    await expect(parents).not.toBeChecked();
    const viewport = page.viewportSize()!;
    for (const width of [1280, 320]) {
      await page.setViewportSize({ width, height: viewport.height });
      await expect(parents).toBeInViewport();
      expect(await dialog.evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
    }
    await page.setViewportSize(viewport);
    await expect(dialog.locator('#copyModeHelp')).toHaveText(
      `A copy of ${subject}, without later updates. Full links do not expire or upload content.`,
    );
    const flat = await configureCopyLink(page, { format: 'full', close: false });
    const encoded = new URLSearchParams(new URL(flat).hash.slice(1));
    const state = encoded.get(kind === 'document' ? 'p' : 'b')!;
    const decoded = JSON.parse(inflateRawSync(Buffer.from(state, 'base64url').subarray(4)).toString());
    expect(kind === 'document' ? decoded : (decoded.path ?? [])).toEqual([]);
    const nested = await configureCopyLink(page, { format: 'full', includeParentFolders: true, close: false });
    expect(nested).not.toBe(flat);
    const short = await configureCopyLink(page, { format: 'short', includeParentFolders: false, close: false });
    await expect(dialog.locator('#copyModeHelp')).toHaveText(
      `A copy of ${subject}, without later updates. Short links upload an encrypted copy to Secutils.`,
    );
    await expect(dialog).not.toContainText('grant GitHub access');
    await dialog.getByRole('button', { name: 'Close', exact: true }).click();
    const context = await browser.newContext({ baseURL });
    try {
      const recipient = await context.newPage();
      await recipient.goto(short);
      await expect(recipient.locator('#previewArticle')).toContainText('A shared guide.');
      await expect(recipient.locator('#crumbs')).toHaveText(
        kind === 'document' ? 'Shared with me' : 'Shared with meConfidential',
      );
      await expect(recipient.getByRole('link', { name: 'Outside', exact: true })).toHaveCount(0);
      await expect(recipient.locator('#documentAccess')).toBeHidden();
    } finally {
      await context.close();
    }
  }
});
