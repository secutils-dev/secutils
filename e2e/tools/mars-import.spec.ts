import { readFile } from 'node:fs/promises';
import { deflateRawSync } from 'node:zlib';

import type { Page, Route } from '@playwright/test';
import { expect } from '@playwright/test';

import { test } from './_mars_test';

const MARS_BASE_URL = process.env.MARS_BASE_URL ?? 'https://mars.secutils.dev';
test.use({ baseURL: MARS_BASE_URL });
const RAW =
  '---\r\ntitle: "Preferred title"\r\n---\r\n# Body heading\r\n\r\nOriginal source text.\r\n\r\n<!-- su-comments\r\n[ { "id":"thread", "quote":"Original source text.", "comments":[{"id":"comment","body":"Private review"}] } ]\r\n-->\r\n';
const BODY = '# Reopened reader\n\nEditable **Markdown** and literal `</script>` text.\n';
const exported = (body: string) =>
  `<!DOCTYPE html><html><head><title>Exported HTML</title></head><body><h1>Rendered article</h1><script type="text/markdown" id="su-md-source">\n${body.replace(/<\/(script)/gi, '<\\/$1')}\n</script></body></html>`;

async function boot(page: Page, path = '/') {
  await page.goto(path);
  await page.waitForFunction(() => !document.documentElement.classList.contains('mars-boot'));
}
async function file(page: Page, text: string, name = 'File.md') {
  await page.locator('#fileInput').setInputFiles({
    name,
    mimeType: name.endsWith('html') ? 'text/html' : 'text/markdown',
    buffer: Buffer.from(text),
  });
  await expect(page.locator('#saveState')).toHaveText('Saved');
}
async function remote(page: Page, url: string, body: string) {
  await page.route(url, (route) =>
    route.fulfill({ body, contentType: 'text/plain', headers: { 'access-control-allow-origin': '*' } }),
  );
}
async function form(page: Page, url: string) {
  await page.locator('#urlInput').fill(url);
  await page.getByRole('button', { name: 'Load URL', exact: true }).click();
}
async function stored(page: Page) {
  return page.evaluate(async () => {
    const database = await new Promise<IDBDatabase>((resolve, reject) => {
      const request = indexedDB.open('mars');
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    const result = await new Promise<{
      nodes: { id: string; parentId: string | null; name: string; kind: string; originId?: string; handle?: unknown }[];
      texts: unknown[];
    }>((resolve, reject) => {
      const transaction = database.transaction(['nodes', 'docs'], 'readonly');
      const nodes = transaction.objectStore('nodes').getAll();
      const texts = transaction.objectStore('docs').getAll();
      transaction.oncomplete = () => resolve({ nodes: nodes.result, texts: texts.result });
      transaction.onerror = () => reject(transaction.error);
    });
    database.close();
    return { docs: result.nodes.filter((node) => node.kind === 'doc'), texts: result.texts };
  });
}
async function downloadMarkdown(page: Page) {
  await page.getByRole('button', { name: 'Export document', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: /^Export / });
  await expect(dialog.getByRole('button', { name: 'Download Markdown', exact: true })).toBeEnabled();
  const pending = page.waitForEvent('download');
  await dialog.getByRole('button', { name: 'Download Markdown', exact: true }).click();
  const download = await pending;
  const text = await readFile(await download.path(), 'utf8');
  await dialog.getByRole('button', { name: 'Close export', exact: true }).click();
  return text;
}
async function edit(page: Page, text: string) {
  await page.getByRole('button', { name: 'Source', exact: true }).click();
  await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
  await page.evaluate(
    (text) => (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(text),
    text,
  );
  await page.getByRole('button', { name: 'Preview', exact: true }).click();
  await expect(page.locator('#saveState')).toHaveText('Saved');
}

test.describe('source import', () => {
  test('default destination survives reload, follows profile changes across tabs, and keeps explicit import targets', async ({
    page,
  }, testInfo) => {
    await boot(page);
    const newFolder = async (name: string) => {
      await boot(page);
      await page.getByRole('button', { name: 'New document, folder, or import', exact: true }).click();
      await page.getByRole('menuitem', { name: 'New folder', exact: true }).click();
      await page.getByRole('textbox', { name: 'Name', exact: true }).fill(name);
      await page.getByRole('textbox', { name: 'Name', exact: true }).press('Enter');
      return (await page
        .getByRole('treeitem')
        .filter({ has: page.getByRole('button', { name: `Actions for ${name}`, exact: true }) })
        .getAttribute('data-id'))!;
    };
    await newFolder('Shared with me');
    await expect(page.getByTitle('Shared with me · Default destination', { exact: true })).toBeVisible();
    const work = await newFolder('Work');
    await page.getByRole('button', { name: 'Actions for Work', exact: true }).click();
    const useDefault = page.getByRole('menuitem', { name: 'Use as default', exact: true });
    await expect(useDefault.locator('.item-sub')).toHaveCount(0);
    await expect(page.getByRole('menuitem', { name: /^Clear/ })).toHaveCount(0);
    await useDefault.press('Enter');
    const workRow = page.locator(`.tree-row[data-id="${work}"]`);
    await expect(workRow).toHaveAttribute('aria-description', 'Default destination');
    await expect(page.locator('.tree-icon-default')).toHaveCount(1);
    await expect(workRow.locator('.tree-default-star')).toBeVisible();
    const openGlyph = await workRow.locator('.tree-icon > svg path').first().getAttribute('d');
    await workRow.getByText('Work', { exact: true }).click();
    await expect(workRow).toHaveAttribute('aria-expanded', 'false');
    await expect(workRow.locator('.tree-default-star')).toBeVisible();
    expect(await workRow.locator('.tree-icon > svg path').first().getAttribute('d')).not.toBe(openGlyph);
    await workRow.getByText('Work', { exact: true }).click();
    await expect(workRow).toHaveAttribute('aria-expanded', 'true');
    await page.getByRole('button', { name: 'Actions for Work', exact: true }).click();
    await expect(page.getByRole('menuitem', { name: 'Use as default', exact: true })).toBeDisabled();
    await page.keyboard.press('Escape');
    for (const theme of ['light', 'dark']) {
      await page.evaluate((theme) => {
        document.documentElement.dataset.theme = theme;
      }, theme);
      for (const expanded of [true, false]) {
        if ((await workRow.getAttribute('aria-expanded')) !== String(expanded))
          await workRow.getByText('Work', { exact: true }).click();
        await expect(workRow.locator('.tree-default-star')).toBeVisible();
        const geometry = await workRow.locator('.tree-icon-default').evaluate((element) => {
          const star = element.querySelector<SVGGElement>('.tree-default-star')!,
            matrix = star.transform.baseVal.consolidate()!.matrix,
            center = new DOMPoint(8, 8).matrixTransform(matrix);
          return {
            width: 16 * matrix.a,
            height: 16 * matrix.d,
            centerX: center.x,
            centerY: center.y,
          };
        });
        expect(geometry).toEqual({ width: 6, height: 6, centerX: 8, centerY: expanded ? 10 : 9 });
        await expect(workRow.locator('.tree-icon-default svg')).toHaveCount(1);
        for (const zoom of [1, 1.2, 1.3]) {
          await page.evaluate((zoom) => {
            document.documentElement.style.zoom = String(zoom);
          }, zoom);
          await page.locator('#sidebar').screenshot({
            path: testInfo.outputPath(`default-folder-${theme}-${expanded ? 'open' : 'closed'}-${zoom}.png`),
          });
        }
        await page.evaluate(() => {
          document.documentElement.style.zoom = '';
        });
      }
    }
    await page.setViewportSize({ width: 320, height: 800 });
    const navigation = page.getByRole('button', { name: 'Toggle documents sidebar', exact: true });
    await expect(navigation).toHaveAttribute('aria-expanded', 'false');
    await navigation.click();
    await expect(navigation).toHaveAttribute('aria-expanded', 'true');
    await expect(workRow.locator('.tree-default-star')).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath('default-folder-phone.png') });
    await page.setViewportSize({ width: 1280, height: 800 });
    const other = await newFolder('Other');
    const link = (name: string, text: string) => {
      const raw = Buffer.from(text),
        encoded = Buffer.concat([Buffer.alloc(4), deflateRawSync(raw)]);
      encoded.writeUInt32LE(raw.length, 0);
      return `/#${new URLSearchParams({ name, md: encoded.toString('base64url'), o: crypto.randomUUID() })}`;
    };
    await boot(page, link('From CLI', '# From CLI\n\nIncoming from a command.'));
    await expect(page.locator('#docTitle')).toHaveText('From CLI');
    expect((await stored(page)).docs.find((doc) => doc.name === 'From CLI')?.parentId).toBe(work);
    await page
      .getByRole('treeitem')
      .filter({ has: page.getByRole('button', { name: 'Actions for Other', exact: true }) })
      .getByText('Other', { exact: true })
      .click();
    await file(page, '# Picked\n\nA unique file from Open.', 'Picked.md');
    await expect(page.locator('#docTitle')).toHaveText('Picked');
    expect((await stored(page)).docs.find((doc) => doc.name === 'Picked')?.parentId).toBe(work);
    const url = 'https://destination.test/Remote.md';
    await remote(page, url, '# Remote\n\nFetched into the selected default.');
    await boot(page, `/?url=${encodeURIComponent(url)}`);
    await expect(page.locator('#docTitle')).toHaveText('Remote');
    expect((await stored(page)).docs.find((doc) => doc.name === 'Remote')?.parentId).toBe(work);
    await page
      .getByRole('treeitem')
      .filter({ has: page.getByRole('button', { name: 'Actions for Other', exact: true }) })
      .getByText('Other', { exact: true })
      .click();
    await page.locator('#importInput').setInputFiles({
      name: 'Explicit.md',
      mimeType: 'text/markdown',
      buffer: Buffer.from('# Explicit\n\nAn explicit import target.'),
    });
    await expect
      .poll(async () => (await stored(page)).docs.find((doc) => doc.name === 'Explicit')?.parentId)
      .toBe(other);
    const second = await page.context().newPage();
    try {
      await boot(second);
      await expect(second.getByTitle('Work · Default destination', { exact: true })).toBeVisible();
      await second.getByRole('button', { name: 'Actions for Other', exact: true }).click();
      await second.getByRole('menuitem', { name: 'Use as default', exact: true }).click();
      await expect(page.getByTitle('Other · Default destination', { exact: true })).toBeVisible();
      await file(page, '# Cross tab\n\nThe other tab changed the default.', 'Cross tab.md');
      await expect(page.locator('#docTitle')).toHaveText('Cross tab');
      expect((await stored(page)).docs.find((doc) => doc.name === 'Cross tab')?.parentId).toBe(other);
      await page.getByRole('button', { name: 'Actions for Shared with me', exact: true }).click();
      await page.getByRole('menuitem', { name: 'Use as default', exact: true }).click();
      await boot(page, link('Shared again', '# Shared again\n\nThe original destination selected again.'));
      await expect(page.locator('#docTitle')).toHaveText('Shared again');
      await expect(page.getByRole('button', { name: 'Actions for Shared with me', exact: true })).toBeVisible();
    } finally {
      await second.close();
    }
  });

  test.setTimeout(60000);
  for (const entry of ['file', 'drop', 'form', 'query', 'library']) {
    test(`${entry} preserves Markdown bytes and uses the frontmatter title`, async ({ page }) => {
      const url = 'https://source.test/Filename.HTML';
      await remote(page, url, RAW);
      await boot(page, entry === 'query' ? `/?url=${encodeURIComponent(url)}` : '/');
      if (entry === 'file') await file(page, RAW, 'Filename.md');
      else if (entry === 'library')
        await page
          .locator('#importInput')
          .setInputFiles({ name: 'Filename.md', mimeType: 'text/markdown', buffer: Buffer.from(RAW) });
      else if (entry === 'drop')
        await page.evaluate((text) => {
          const transfer = new DataTransfer();
          transfer.items.add(new File([text], 'Filename.md', { type: 'text/markdown' }));
          document
            .getElementById('previewWrap')!
            .dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: transfer }));
        }, RAW);
      else if (entry === 'form') await form(page, url);
      await expect(page.locator('#docTitle')).toHaveText('Preferred title');
      await expect(page.locator('#previewArticle')).toContainText('Original source text.');
      await expect(page.locator('#saveState')).toHaveText('Saved');
      expect((await stored(page)).texts).toContain(RAW);
      expect(await downloadMarkdown(page)).toBe(RAW);
    });

    test(`${entry} reopens embedded source as editable Markdown`, async ({ page }) => {
      const html = exported(BODY),
        url = 'https://source.test/Reader.html';
      await remote(page, url, html);
      await boot(page, entry === 'query' ? `/?url=${encodeURIComponent(url)}` : '/');
      if (entry === 'file') await file(page, html, 'Reader.html');
      else if (entry === 'library')
        await page
          .locator('#importInput')
          .setInputFiles({ name: 'Reader.html', mimeType: 'text/html', buffer: Buffer.from(html) });
      else if (entry === 'drop')
        await page.evaluate((text) => {
          const transfer = new DataTransfer();
          transfer.items.add(new File([text], 'Reader.html', { type: 'text/html' }));
          document
            .getElementById('previewWrap')!
            .dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: transfer }));
        }, html);
      else if (entry === 'form') await form(page, url);
      await expect(page.locator('#docTitle')).toHaveText('Reopened reader');
      await expect(page.locator('#previewArticle')).toContainText('Editable Markdown');
      await expect(page.locator('#saveState')).toHaveText('Saved');
      const data = await stored(page);
      expect(data.texts).toContain(BODY);
      expect(data.texts).not.toContain(html);
      expect(data.docs[0].handle).toBeUndefined();
      expect(await downloadMarkdown(page)).toBe(BODY);
      await edit(page, '# Reopened reader\n\nChanged through Source.');
      await expect(page.locator('#previewArticle')).toContainText('Changed through Source.');
    });
  }

  test('explicit incoming names override titles and strip HTML/Markdown extensions', async ({ page }) => {
    const utf8 = Buffer.from(RAW),
      encoded = Buffer.concat([Buffer.alloc(4), deflateRawSync(utf8)]);
    encoded.writeUInt32LE(utf8.length, 0);
    await boot(
      page,
      `/#${new URLSearchParams({ name: 'Explicit.HTM', md: encoded.toString('base64url'), o: 'explicit-source' })}`,
    );
    await expect(page.locator('#docTitle')).toHaveText('Explicit');
    expect(await downloadMarkdown(page)).toBe(RAW);
  });

  test('an actual exported HTML file reopens with body-only source in a fresh library', async ({ page, browser }) => {
    await boot(page);
    await file(page, RAW, 'Source.md');
    await page.getByRole('button', { name: 'Export document', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: /^Export / });
    await dialog.getByLabel('Embed Markdown source', { exact: true }).check();
    await expect(dialog.getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled();
    const pending = page.waitForEvent('download');
    await dialog.getByRole('button', { name: 'Download HTML', exact: true }).click();
    const html = await readFile(await (await pending).path(), 'utf8');
    const context = await browser.newContext({ baseURL: MARS_BASE_URL });
    try {
      const reopened = await context.newPage();
      await boot(reopened);
      await file(reopened, html, 'Exported.html');
      await expect(reopened.locator('#docTitle')).toHaveText('Body heading');
      await expect(reopened.locator('#previewArticle')).toContainText('Original source text.');
      const source = await downloadMarkdown(reopened);
      expect(source).toBe('# Body heading\n\nOriginal source text.\n');
      expect(source).not.toContain('Private review');
      expect(source).not.toContain('title:');
    } finally {
      await context.close();
    }
  });

  for (const entry of ['form', 'query', 'file']) {
    test(`${entry} retains raw HTML when there is no embedded source`, async ({ page }) => {
      const html = '<!DOCTYPE html><html><body><h1>Raw reader</h1><p>Rendered HTML without source.</p></body></html>';
      const url = 'https://source.test/Raw%20reader.HTM';
      await remote(page, url, html);
      await boot(page, entry === 'query' ? `/?url=${encodeURIComponent(url)}` : '/');
      if (entry === 'form') await form(page, url);
      else if (entry === 'file') await file(page, html, 'Raw reader.HTM');
      await expect(page.locator('#docTitle')).toHaveText('Raw reader');
      await expect(page.locator('#saveState')).toHaveText('Saved');
      expect((await stored(page)).texts).toContain(html);
      expect(await downloadMarkdown(page)).toBe(html);
    });
  }

  test('form and query imports reuse the URL origin and retain locally edited text', async ({ page }) => {
    const url = 'https://source.test/Document.md';
    await remote(page, url, RAW);
    await boot(page);
    await form(page, url);
    await expect(page.locator('#docTitle')).toHaveText('Preferred title');
    await expect(page.locator('#saveState')).toHaveText('Saved');
    const original = (await stored(page)).docs[0];
    const changed = RAW.replace('Original source text.', 'Locally edited source text.');
    await edit(page, changed.replace(/\r\n/g, '\n').replace(/\n<!-- su-comments[\s\S]*$/, ''));
    await boot(page, `/?url=${encodeURIComponent(url)}`);
    await expect(page.locator('#previewArticle')).toContainText('Locally edited source text.');
    await expect(page.locator('#toast')).toContainText("source hasn't changed");
    const data = await stored(page);
    expect(data.docs).toHaveLength(1);
    expect(data.docs[0].id).toBe(original.id);
    expect(data.docs[0].originId).toBe(`url:${url}`);
  });

  test('a changed URL copy requires a decision when both copies changed', async ({ page }) => {
    const url = 'https://source.test/Conflict.md';
    let body = '# Conflict\n\nInitial paragraph.';
    await page.route(url, (route) => route.fulfill({ body, contentType: 'text/plain' }));
    await boot(page);
    await form(page, url);
    await expect(page.locator('#saveState')).toHaveText('Saved');
    await edit(page, '# Conflict\n\nLocal paragraph.');
    body = '# Conflict\n\nRemote paragraph.';
    await boot(page, `/?url=${encodeURIComponent(url)}`);
    const conflict = page.getByRole('dialog', { name: 'Both copies changed', exact: true });
    await expect(conflict).toBeVisible();
    await expect(conflict.getByRole('button', { name: 'Save as a copy', exact: true })).toBeFocused();
    await conflict.getByRole('button', { name: 'Keep my text', exact: true }).click();
    await expect(page.locator('#previewArticle')).toContainText('Local paragraph.');
    expect((await stored(page)).docs).toHaveLength(1);
  });

  test('reopening the same exported HTML asks to update rather than silently duplicating', async ({ page }) => {
    await boot(page);
    await file(page, exported(BODY), 'Reader.html');
    const original = (await stored(page)).docs[0];
    await page
      .locator('#fileInput')
      .setInputFiles({ name: 'Reader.html', mimeType: 'text/html', buffer: Buffer.from(exported(BODY)) });
    const match = page.getByRole('dialog', { name: 'Is this an updated copy?', exact: true });
    await expect(match).toBeVisible();
    await match.getByRole('button', { name: 'Update "Reopened reader"', exact: true }).click();
    await expect(page.locator('#toast')).toContainText('already up to date');
    expect((await stored(page)).docs.map((doc) => doc.id)).toEqual([original.id]);
  });

  test('HTTP and network failures retain the previous document and useful error feedback', async ({ page }) => {
    await boot(page);
    await file(page, '# Retained\n\nThe previous document.', 'Retained.md');
    const original = (await stored(page)).docs[0];
    const url = 'https://source.test/Unavailable.md';
    await page.route(url, (route) => route.fulfill({ status: 503, body: 'Unavailable' }));
    await boot(page, `/?url=${encodeURIComponent(url)}`);
    await expect(page.locator('#toast')).toContainText('HTTP 503');
    await expect(page.locator('#previewArticle')).toContainText('The previous document.');
    expect((await stored(page)).docs[0].id).toBe(original.id);
    await page.unroute(url);
    await page.route(url, (route) => route.abort());
    await boot(page, `/?url=${encodeURIComponent(url)}`);
    await expect(page.locator('#toast')).toContainText('CORS or network');
    await expect(page.locator('#previewArticle')).toContainText('The previous document.');
  });

  for (const next of ['URL', 'file', 'document']) {
    test(`a new ${next} supersedes a pending URL without applying stale results`, async ({ page }) => {
      const slow = 'https://source.test/Slow.md',
        fast = 'https://source.test/Fast.md';
      let release!: () => void, requested!: () => void, handled!: () => void;
      const pending = new Promise<void>((resolve) => {
        release = resolve;
      });
      const started = new Promise<void>((resolve) => {
        requested = resolve;
      });
      const finished = new Promise<void>((resolve) => {
        handled = resolve;
      });
      await page.route(slow, async (route: Route) => {
        requested();
        await pending;
        await route
          .fulfill({ body: '# Stale result\n\nDo not import this.', contentType: 'text/plain' })
          .catch(() => {});
        handled();
      });
      await remote(page, fast, '# Winning URL\n\nWinning text.');
      await boot(page);
      if (next === 'document') {
        await file(page, '# Stored winner\n\nStored text.', 'Stored.md');
        await page.getByRole('button', { name: 'New document, folder, or import' }).click();
        await page.getByRole('menuitem', { name: 'New document', exact: true }).click();
        await expect(page.getByRole('button', { name: 'Source', exact: true })).toHaveAttribute('aria-pressed', 'true');
        await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
        await page.evaluate(() =>
          (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(
            '# Other document\n\nOther text.',
          ),
        );
        await page.getByRole('button', { name: 'Preview', exact: true }).click();
        await expect(page.locator('#saveState')).toHaveText('Saved');
      }
      await page.evaluate((url) => {
        (document.getElementById('urlInput') as HTMLInputElement).value = url;
        document.getElementById('urlForm')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      }, slow);
      await started;
      if (next === 'URL') await form(page, fast);
      else if (next === 'file') await file(page, '# Winning file\n\nWinning text.', 'Winner.md');
      else {
        const id = (await stored(page)).docs.find((doc) => doc.name === 'Stored winner')!.id;
        await page.evaluate((id) => {
          location.hash = `doc=${id}`;
        }, id);
      }
      const name = next === 'URL' ? 'Winning URL' : next === 'file' ? 'Winning file' : 'Stored winner';
      await expect(page.locator('#docTitle')).toHaveText(name);
      release();
      await finished;
      await expect(page.locator('#previewArticle')).not.toContainText('Do not import this.');
      const data = await stored(page);
      expect(data.docs).toHaveLength(next === 'document' ? 2 : 1);
      expect(data.docs.some((doc) => doc.name === name)).toBe(true);
      expect(data.texts.some((text) => typeof text === 'string' && text.includes('Do not import this.'))).toBe(false);
    });
  }
});
