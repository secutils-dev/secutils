import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { join, resolve } from 'node:path';
import { deflateRawSync } from 'node:zlib';

import type { Page, Route } from '@playwright/test';
import { expect } from '@playwright/test';

import { mockWorker } from './_mars_indexing';
import { copyLink } from './_mars_sharing';
import { test } from './_mars_test';

const MARS_BASE_URL = process.env.MARS_BASE_URL ?? 'https://mars.secutils.dev';
const require = createRequire(resolve(process.cwd(), '../dev/tools/mars/package.json'));
const { zipSync } = require('fflate') as { zipSync(files: Record<string, Uint8Array>): Uint8Array };
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
    return { nodes: result.nodes, docs: result.nodes.filter((node) => node.kind === 'doc'), texts: result.texts };
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

// Read all review/index side records along with the committed tree, not only visible rows.
async function importSnapshot(page: Page) {
  return page.evaluate(async () => {
    const database = await new Promise<IDBDatabase>((resolve) => {
      const request = indexedDB.open('mars');
      request.onsuccess = () => resolve(request.result);
    });
    const result = await new Promise<{
      nodes: {
        id: string;
        kind: string;
        name: string;
        parentId: string | null;
        updatedAt: number;
        pending?: boolean;
        importSource?: { destinationId: string | null; path: string[] };
        connection?: { document: string };
      }[];
      records: Record<string, unknown>;
    }>((resolve) => {
      const tx = database.transaction(['nodes', 'docs'], 'readonly');
      const nodes = tx.objectStore('nodes').getAll(),
        keys = tx.objectStore('docs').getAllKeys(),
        values = tx.objectStore('docs').getAll();
      tx.oncomplete = () =>
        resolve({
          nodes: nodes.result,
          records: Object.fromEntries(keys.result.map((key, i) => [String(key), values.result[i]])),
        });
    });
    database.close();
    return result;
  });
}
async function importInbox(page: Page) {
  await page.getByRole('button', { name: 'New document, folder, or import', exact: true }).click();
  await page.getByRole('menuitem', { name: 'New folder', exact: true }).click();
  await page.getByRole('textbox', { name: 'Name', exact: true }).fill('Import inbox');
  await page.getByRole('textbox', { name: 'Name', exact: true }).press('Enter');
  await expect(page.getByRole('button', { name: 'Actions for Import inbox', exact: true })).toBeVisible();
}
async function importAtInbox(
  page: Page,
  files: string | { name: string; mimeType: string; buffer: Buffer }[],
  folder = false,
) {
  if (
    (await page
      .getByRole('button', { name: 'Toggle documents sidebar', exact: true })
      .getAttribute('aria-expanded')) === 'false'
  )
    await page.getByRole('button', { name: 'Toggle documents sidebar', exact: true }).click();
  await page.getByRole('button', { name: 'Actions for Import inbox', exact: true }).click();
  const pending = page.waitForEvent('filechooser');
  await page.getByRole('menuitem', { name: folder ? 'Import a folder…' : /^Import files…/, exact: folder }).click();
  await (await pending).setFiles(files);
}

test.describe('source import', () => {
  test('batch re-import reuses smart-search vectors and embeds only the changed file', async ({ page, context }) => {
    await mockWorker(context);
    await boot(page);
    await importInbox(page);
    const files = Array.from({ length: 24 }, (_, i) => ({
      name: `Indexed ${i}.md`,
      mimeType: 'text/markdown',
      buffer: Buffer.from(`# Indexed ${i}\n\nOriginal passage with enough words to index.`),
    }));
    await importAtInbox(page, files);
    await expect(page.locator('#toast')).toContainText('Imported 24 documents');
    await page.getByRole('button', { name: 'Smart search', exact: true }).click();
    await page.getByRole('radio', { name: /Best speed/ }).check();
    await page.locator('#marsModalFoot').getByRole('button', { name: 'Turn on', exact: true }).click();
    await expect
      .poll(
        async () => Object.keys((await importSnapshot(page)).records).filter((key) => key.startsWith('vec:')).length,
      )
      .toBe(24);
    const calls = () =>
      page.evaluate(() => (window as unknown as { __marsWorker: { embeds: string[][] } }).__marsWorker.embeds.length);
    const initial = await calls();
    await importAtInbox(page, files);
    await expect(page.locator('#toast')).toContainText('unchanged 24');
    // Past the index debounce: an unchanged import must not merely postpone duplicate inference.
    await page.waitForTimeout(2500);
    expect(await calls()).toBe(initial);
    files[7].buffer = Buffer.from('# Indexed 7\n\nChanged passage with different words to index.');
    await importAtInbox(page, files);
    await expect(page.locator('#toast')).toContainText('updated 1 · unchanged 23');
    await expect.poll(calls).toBe(initial + 1);
    await expect(page.locator('#sbSmartStatus')).toBeHidden();
  });
  test('folder drops consume every directory batch and re-import more than 100 files without duplicates', async ({
    page,
  }) => {
    await boot(page);
    const drop = async (changed: boolean) =>
      page.evaluate((changed) => {
        const files = Array.from({ length: 125 }, (_, i) => ({
          name: `Entry ${i}.md`,
          isFile: true,
          file(resolve: (file: File) => void) {
            resolve(new File([`# Entry ${i}\n\n${changed && i === 124 ? 'Changed.' : 'Original.'}`], `Entry ${i}.md`));
          },
        }));
        const nested = {
          name: 'Nested',
          isFile: false,
          createReader() {
            let offset = 0;
            return {
              readEntries(resolve: (entries: unknown[]) => void) {
                const batch = files.slice(offset, offset + 100);
                offset += 100;
                resolve(batch);
              },
            };
          },
        };
        const root = {
          name: 'Dropped',
          isFile: false,
          createReader() {
            let done = false;
            return {
              readEntries(resolve: (entries: unknown[]) => void) {
                resolve(done ? [] : [nested]);
                done = true;
              },
            };
          },
        };
        const transfer = new DataTransfer();
        // Chromium can return a fresh wrapper for a DataTransferItem on each access.
        // Override the list, not one transient item wrapper, to model directory batches.
        Object.defineProperty(transfer, 'items', { value: [{ webkitGetAsEntry: () => root }] });
        document
          .getElementById('tree')!
          .dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: transfer }));
      }, changed);
    await drop(false);
    await expect(page.locator('#toast')).toContainText('Imported 125 documents', { timeout: 15000 });
    const before = await importSnapshot(page);
    await drop(true);
    await expect(page.locator('#toast')).toContainText('updated 1 · unchanged 124', { timeout: 15000 });
    expect((await importSnapshot(page)).nodes.map((n) => n.id)).toEqual(before.nodes.map((n) => n.id));
  });

  test('indistinguishable loose files remain separate copies and cannot overwrite existing sources', async ({
    page,
  }) => {
    await boot(page);
    await importInbox(page);
    const files = [
      { name: 'README.md', mimeType: 'text/markdown', buffer: Buffer.from('# First\n') },
      { name: 'README.md', mimeType: 'text/markdown', buffer: Buffer.from('# Second\n') },
    ];
    await importAtInbox(page, files);
    await expect(page.locator('#toast')).toContainText('indistinguishable file paths saved as copies');
    const before = await importSnapshot(page);
    expect(before.nodes.filter((n) => n.kind === 'doc').every((n) => !n.importSource)).toBe(true);
    await importAtInbox(page, files);
    await expect(page.locator('#toast')).toContainText('Imported 2 documents');
    await expect.poll(async () => (await importSnapshot(page)).nodes.filter((n) => n.kind === 'doc').length).toBe(4);
    const after = await importSnapshot(page);
    expect(after.nodes.filter((n) => n.kind === 'doc')).toHaveLength(4);
    for (const n of before.nodes.filter((n) => n.kind === 'doc'))
      expect(after.records[n.id]).toBe(before.records[n.id]);
  });
  test('batch re-import across tabs serializes identity and refuses edits made during conflict decisions', async ({
    page,
  }) => {
    await boot(page);
    await importInbox(page);
    const files = Array.from({ length: 24 }, (_, i) => ({
      name: `Tab ${i}.md`,
      mimeType: 'text/markdown',
      buffer: Buffer.from(`# Tab ${i}\n\nOriginal.`),
    }));
    await importAtInbox(page, files);
    await expect(page.locator('#toast')).toContainText('Imported 24 documents');
    const second = await page.context().newPage();
    try {
      await boot(second);
      await Promise.all([importAtInbox(page, files), importAtInbox(second, files)]);
      for (const tab of [page, second]) await expect(tab.locator('#toast')).toContainText('unchanged 24');
      const initial = await importSnapshot(page),
        n = initial.nodes.find((n) => n.name === 'Tab 7')!;
      await page.goto(`/#doc=${n.id}`);
      await edit(page, '# Tab 7\n\nFirst local edit.');
      files[7].buffer = Buffer.from('# Tab 7\n\nDisk changed.');
      await importAtInbox(page, files);
      const dialog = page.getByRole('dialog', { name: 'Review imported files', exact: true });
      await expect(dialog).toBeVisible();
      await second.goto(`/#doc=${n.id}`);
      await expect(second.locator('#previewArticle')).toContainText('First local edit.');
      await edit(second, '# Tab 7\n\nNewer edit from another tab.');
      await dialog.getByLabel('Tab 7.md', { exact: true }).selectOption('theirs');
      await dialog.getByRole('button', { name: 'Import files', exact: true }).click();
      await expect(page.locator('#toast')).toContainText('1 not updated');
      const after = await importSnapshot(page);
      expect(after.records[n.id]).toContain('Newer edit from another tab.');
      expect(after.records[`opened:${n.id}`]).toBe(initial.records[`opened:${n.id}`]);
      expect(after.nodes.filter((n) => n.kind === 'doc')).toHaveLength(24);
    } finally {
      await second.close();
    }
  });

  test('batch re-import into Sync updates the original replica and paired device without sharing import identities', async ({
    page,
    browser,
  }) => {
    test.setTimeout(180000);
    await boot(page);
    await importInbox(page);
    const files = Array.from({ length: 24 }, (_, i) => ({
      name: `Synced ${i}.md`,
      mimeType: 'text/markdown',
      buffer: Buffer.from(`# Synced ${i}\n\nOriginal.`),
    }));
    await importAtInbox(page, files);
    await expect(page.locator('#toast')).toContainText('Imported 24 documents');
    await page.getByRole('button', { name: 'Actions for Import inbox', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
    const linkField = page.getByRole('textbox', { name: 'Sync link', exact: true });
    await expect(linkField).toBeVisible({ timeout: 60000 });
    const link = await linkField.inputValue();
    const context = await browser.newContext({ baseURL: MARS_BASE_URL });
    try {
      const peer = await context.newPage();
      await boot(peer, link);
      await peer.getByRole('button', { name: 'Connect', exact: true }).click();
      await expect(page.getByRole('button', { name: 'Codes match', exact: true })).toBeVisible({ timeout: 30000 });
      await page.getByRole('button', { name: 'Codes match', exact: true }).click();
      await expect(peer.locator('#toast')).toContainText('Sync connected', { timeout: 60000 });
      await page.getByRole('button', { name: 'Close', exact: true }).click();
      const before = await importSnapshot(page),
        n = before.nodes.find((n) => n.name === 'Synced 7')!;
      files[7].buffer = Buffer.from('# Synced 7\n\nRe-imported from disk.');
      await importAtInbox(page, files);
      await expect(page.locator('#toast')).toContainText('updated 1 · unchanged 23');
      const after = await importSnapshot(page);
      expect(after.nodes.filter((n) => n.kind === 'doc').map((n) => n.id)).toEqual(
        before.nodes.filter((n) => n.kind === 'doc').map((n) => n.id),
      );
      expect(after.nodes.find((node) => node.id === n.id)?.connection?.document).toBe(n.connection?.document);
      expect(after.records[`base:${n.id}`]).toBe(before.records[n.id]);
      expect(after.records[`opened:${n.id}`]).toBe(files[7].buffer.toString());
      await peer.getByRole('link', { name: 'Synced 7', exact: true }).click();
      await expect(peer.locator('#previewArticle')).toContainText('Re-imported from disk.', { timeout: 30000 });
      expect((await importSnapshot(peer)).nodes.filter((n) => n.importSource)).toEqual([]);
      expect(JSON.stringify(after.records[`sync:${n.id}`])).not.toContain('importSource');
      await page.reload();
      await importAtInbox(page, files);
      await expect(page.locator('#toast')).toContainText('unchanged 24');
    } finally {
      await context.close();
    }
  });
  test('batch re-import of 24 nested files updates one identity and skips unchanged documents and indexes', async ({
    page,
  }, testInfo) => {
    await boot(page);
    await importInbox(page);
    const directory = testInfo.outputPath('Workspace');
    const paths = Array.from({ length: 24 }, (_, i) =>
      join(directory, `Section ${i % 4}`, '研究 & evidence', 'Deep', `Note ${Math.floor(i / 4)}.md`),
    );
    for (const path of paths) {
      await mkdir(join(path, '..'), { recursive: true });
      await writeFile(path, `# Evidence\r\n\r\nOriginal ${paths.indexOf(path)}.\r\n`);
    }
    await importAtInbox(page, directory, true);
    await expect(page.locator('#toast')).toContainText('Imported 24 documents');
    const initial = await importSnapshot(page),
      docs = initial.nodes.filter((n) => n.kind === 'doc');
    expect(docs).toHaveLength(24);
    const changed = docs.find(
      (n) => n.importSource?.path.join('/') === 'Workspace/Section 3/研究 & evidence/Deep/Note 1.md',
    )!;
    await page.goto(`/#doc=${changed.id}`);
    await expect(page.locator('#docTitle')).toHaveText('Note 1');
    await page.locator(`.tree-row[data-id="${changed.id}"]`).getByRole('button').click();
    await page.getByRole('menuitem', { name: 'Rename', exact: true }).click();
    await page.getByRole('textbox', { name: 'Name', exact: true }).fill('My evidence label');
    await page.getByRole('textbox', { name: 'Name', exact: true }).press('Enter');
    await page.evaluate(
      async (ids) => {
        const database = await new Promise<IDBDatabase>((resolve) => {
          const r = indexedDB.open('mars');
          r.onsuccess = () => resolve(r.result);
        });
        const tx = database.transaction('docs', 'readwrite');
        for (const id of ids) tx.objectStore('docs').put({ sentinel: id }, `vec:${id}`);
        await new Promise<void>((resolve) => {
          tx.oncomplete = () => resolve();
        });
        database.close();
      },
      docs.map((n) => n.id),
    );
    const before = await importSnapshot(page);
    await writeFile(paths[7], '# Evidence\r\n\r\nEdited on disk.\r\n');
    await importAtInbox(page, directory, true);
    await expect(page.locator('#toast')).toContainText('updated 1 · unchanged 23');
    await expect(page.locator('#docTitle')).toHaveText('My evidence label');
    await expect(page.locator('#previewArticle')).toContainText('Edited on disk.');
    const after = await importSnapshot(page);
    expect(
      after.nodes
        .filter((n) => n.kind === 'doc')
        .map((n) => n.id)
        .sort(),
    ).toEqual(docs.map((n) => n.id).sort());
    expect(after.nodes.filter((n) => n.kind === 'folder')).toEqual(before.nodes.filter((n) => n.kind === 'folder'));
    for (const original of docs) {
      expect(after.records[`vec:${original.id}`]).toEqual(before.records[`vec:${original.id}`]);
      if (original.id === changed.id) {
        expect(after.records[`base:${original.id}`]).toBe(initial.records[original.id]);
        expect(after.nodes.find((n) => n.id === original.id)?.pending).toBe(true);
      } else {
        expect(after.nodes.find((n) => n.id === original.id)).toEqual(before.nodes.find((n) => n.id === original.id));
        expect(after.records[original.id]).toBe(before.records[original.id]);
      }
    }
    await page.reload();
    await expect(page.locator('#docTitle')).toHaveText('My evidence label');
    await importAtInbox(page, directory, true);
    await expect(page.locator('#toast')).toContainText('unchanged 24');
    expect((await importSnapshot(page)).nodes.filter((n) => n.kind === 'doc')).toHaveLength(24);
    await page.screenshot({ path: testInfo.outputPath('nested-reimport.png') });
  });

  test('batch re-import matches multi-file subsets and one file while retaining omitted documents', async ({
    page,
  }) => {
    await boot(page);
    await importInbox(page);
    const files = Array.from({ length: 24 }, (_, i) => ({
      name: `File ${i}.md`,
      mimeType: 'text/markdown',
      buffer: Buffer.from(`# File ${i}\n\nOriginal.`),
    }));
    await importAtInbox(page, files);
    await expect(page.locator('#toast')).toContainText('Imported 24 documents');
    const before = await importSnapshot(page);
    files[7].buffer = Buffer.from('# File 7\n\nDisk changed.');
    await importAtInbox(page, files.slice(5, 9));
    await expect(page.locator('#toast')).toContainText('updated 1 · unchanged 3');
    await importAtInbox(page, [files[7]]);
    await expect(page.locator('#toast')).toContainText('unchanged 1');
    const after = await importSnapshot(page);
    expect(after.nodes.map((n) => n.id)).toEqual(before.nodes.map((n) => n.id));
    expect(after.nodes.filter((n) => n.kind === 'doc')).toHaveLength(24);
  });

  test('batch conflicts use one cancellable dialog with per-file choices and preserve merged reviews', async ({
    page,
  }, testInfo) => {
    await boot(page);
    await importInbox(page);
    const review =
      '\n\n<!-- su-comments\n[{"id":"thread","quote":"Original","comments":[{"id":"one","body":"Existing review"}]}]\n-->\n';
    const files = Array.from({ length: 4 }, (_, i) => ({
      name: `Conflict ${i}.md`,
      mimeType: 'text/markdown',
      buffer: Buffer.from(`# File ${i}\n\nOriginal.${review}`),
    }));
    await importAtInbox(page, files);
    await expect(page.locator('#toast')).toContainText('Imported 4 documents');
    for (let i = 0; i < 3; i++) {
      const n = (await importSnapshot(page)).nodes.find((n) => n.name === `Conflict ${i}`)!;
      await page.goto(`/#doc=${n.id}`);
      await expect(page.locator('#docTitle')).toHaveText(`Conflict ${i}`);
      await edit(page, `# File ${i}\n\nMy edit.`);
      files[i].buffer = Buffer.from(
        `# File ${i}\n\nDisk edit.${review.replace('"comments":[', '"comments":[{"id":"two","body":"Incoming reply"},')}`,
      );
    }
    files.push({ name: 'New.md', mimeType: 'text/markdown', buffer: Buffer.from('# New\n\nDo not add on cancel.') });
    const before = await importSnapshot(page);
    await importAtInbox(page, files);
    const dialog = page.getByRole('dialog', { name: 'Review imported files', exact: true });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole('combobox')).toHaveCount(4);
    await expect(dialog.getByRole('button', { name: 'Import files', exact: true })).toBeFocused();
    for (let i = 0; i < 3; i++)
      await expect(dialog.getByLabel(`Conflict ${i}.md`, { exact: true })).toHaveValue('copy');
    await page.keyboard.press('Escape');
    expect(await importSnapshot(page)).toEqual(before);
    await importAtInbox(page, files);
    await expect(dialog).toBeVisible();
    await dialog.getByLabel('Conflict 0.md', { exact: true }).selectOption('mine');
    await dialog.getByLabel('Conflict 1.md', { exact: true }).selectOption('theirs');
    await page.setViewportSize({ width: 320, height: 800 });
    expect(await dialog.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath('reimport-conflicts-phone.png') });
    await dialog.getByRole('button', { name: 'Import files', exact: true }).click();
    await expect(page.locator('#toast')).toContainText('Imported 2 documents');
    const after = await importSnapshot(page);
    expect(after.nodes.filter((n) => n.kind === 'doc')).toHaveLength(6);
    const n0 = before.nodes.find((n) => n.name === 'Conflict 0')!,
      n1 = before.nodes.find((n) => n.name === 'Conflict 1')!,
      n2 = before.nodes.find((n) => n.name === 'Conflict 2')!;
    expect(after.records[n0.id]).toContain('My edit.');
    expect(after.records[n0.id]).toContain('Incoming reply');
    expect(after.records[n1.id]).toContain('Disk edit.');
    expect(after.records[n1.id]).toContain('Existing review');
    expect(after.records[`base:${n1.id}`]).toBe(before.records[n1.id]);
    expect(after.records[n2.id]).toBe(before.records[n2.id]);
    expect(after.nodes.find((n) => n.name === 'Conflict 2 (2)')?.importSource).toBeUndefined();
    await importAtInbox(page, [files[0]]);
    await expect(page.locator('#toast')).toContainText('kept your text in 1');
    await expect(dialog).toBeHidden();
  });

  test('ZIP re-import preserves extensions and destination scopes rather than merging same-named documents', async ({
    page,
  }) => {
    await boot(page);
    await importInbox(page);
    const archive = (change = false) => [
      {
        name: 'Archive.zip',
        mimeType: 'application/zip',
        buffer: Buffer.from(
          zipSync({
            'Root/A/README.md': Buffer.from(change ? '# Changed\n' : '# First\n'),
            'Root/B/README.md': Buffer.from('# Second\n'),
            'Root/A/README.txt': Buffer.from('# Text file\n'),
            'Root/A/readme.md': Buffer.from('# Lowercase\n'),
          }),
        ),
      },
    ];
    await importAtInbox(page, archive());
    await expect(page.locator('#toast')).toContainText('Imported 4 documents');
    const before = await importSnapshot(page);
    await importAtInbox(page, archive(true));
    await expect(page.locator('#toast')).toContainText('updated 1 · unchanged 3');
    expect((await importSnapshot(page)).nodes.map((n) => n.id)).toEqual(before.nodes.map((n) => n.id));
    // An explicit different destination is a separate source scope.
    await page.getByRole('button', { name: 'New document, folder, or import', exact: true }).click();
    await page.getByRole('menuitem', { name: 'New folder', exact: true }).click();
    await page.getByRole('textbox', { name: 'Name', exact: true }).fill('Separate inbox');
    await page.getByRole('textbox', { name: 'Name', exact: true }).press('Enter');
    await page.getByRole('button', { name: 'Actions for Separate inbox', exact: true }).click();
    const chooser = page.waitForEvent('filechooser');
    await page.getByRole('menuitem', { name: /^Import files…/ }).click();
    await (await chooser).setFiles(archive());
    await expect(page.locator('#toast')).toContainText('Imported 4 documents');
    expect((await importSnapshot(page)).nodes.filter((n) => n.kind === 'doc')).toHaveLength(8);
  });

  test('legacy bulk imports require confirmation once before binding full source paths', async ({ page }) => {
    await boot(page);
    await importInbox(page);
    const files = [
      { name: 'Legacy.md', mimeType: 'text/markdown', buffer: Buffer.from('# Legacy\n\nOriginal.') },
      { name: 'Other.md', mimeType: 'text/markdown', buffer: Buffer.from('# Other\n') },
    ];
    await importAtInbox(page, files);
    await expect(page.locator('#toast')).toContainText('Imported 2 documents');
    await page.evaluate(async () => {
      const database = await new Promise<IDBDatabase>((resolve) => {
        const r = indexedDB.open('mars');
        r.onsuccess = () => resolve(r.result);
      });
      const tx = database.transaction('nodes', 'readwrite'),
        store = tx.objectStore('nodes'),
        request = store.getAll();
      request.onsuccess = () => {
        for (const node of request.result) {
          delete node.importSource;
          store.put(node);
        }
      };
      await new Promise<void>((resolve) => {
        tx.oncomplete = () => resolve();
      });
      database.close();
    });
    await page.reload();
    files[0].buffer = Buffer.from('# Legacy\n\nChanged on disk.');
    await importAtInbox(page, files);
    const dialog = page.getByRole('dialog', { name: 'Review imported files', exact: true });
    await expect(dialog).toContainText('older import');
    await dialog.getByLabel('For all listed files', { exact: true }).selectOption('theirs');
    await dialog.getByRole('button', { name: 'Import files', exact: true }).click();
    await expect(page.locator('#toast')).toContainText('updated 1 · unchanged 1');
    await page.reload();
    await importAtInbox(page, files);
    await expect(page.locator('#toast')).toContainText('unchanged 2');
    await expect(dialog).toBeHidden();
    expect((await importSnapshot(page)).nodes.filter((n) => n.kind === 'doc')).toHaveLength(2);
  });
  for (const format of ['full', 'short'] as const) {
    test(`${format} document snapshots preserve saved names and nested folder context without siblings`, async ({
      page,
      context,
      browser,
      browserName,
    }, testInfo) => {
      if (browserName === 'chromium') await context.grantPermissions(['clipboard-read', 'clipboard-write']);
      await boot(page);
      const directory = testInfo.outputPath('Local');
      const path = join(directory, 'Research', '研究 & evidence');
      await mkdir(path, { recursive: true });
      await writeFile(join(path, 'LETTERS.md'), RAW);
      await writeFile(join(path, 'SIBLING.md'), '# Private sibling\n\nDo not include this document.');
      await page.locator('#importFolderInput').setInputFiles(directory);
      await expect(page.locator('#toast')).toContainText('Imported 2 documents');
      await page.getByRole('link', { name: 'LETTERS', exact: true }).click();
      await expect(page.locator('#docTitle')).toHaveText('LETTERS');
      await page.getByRole('button', { name: 'Actions for LETTERS', exact: true }).click();
      await page.getByRole('menuitem', { name: 'Rename', exact: true }).click();
      const rename = page.getByRole('textbox', { name: 'Name', exact: true });
      await rename.fill('Lease evidence.md');
      await rename.press('Enter');
      await expect(page.locator('#docTitle')).toHaveText('Lease evidence.md');
      const link = await copyLink(page, { format });
      if (format === 'full') expect(new URLSearchParams(new URL(link).hash.slice(1)).has('p')).toBe(true);
      const other = await browser.newContext({ baseURL: MARS_BASE_URL });
      try {
        const recipient = await other.newPage();
        await boot(recipient);
        await recipient.getByRole('button', { name: 'New document, folder, or import', exact: true }).click();
        await recipient.getByRole('menuitem', { name: 'New folder', exact: true }).click();
        const name = recipient.getByRole('textbox', { name: 'Name', exact: true });
        await name.fill('Inbox');
        await name.press('Enter');
        await recipient.getByRole('button', { name: 'Actions for Inbox', exact: true }).click();
        await recipient.getByRole('menuitem', { name: 'Use as default', exact: true }).click();
        await boot(recipient, link);
        await expect(recipient.locator('#docTitle')).toHaveText('Lease evidence.md');
        await expect(recipient.locator('#previewArticle')).toContainText('Original source text.');
        const data = await stored(recipient);
        expect(data.docs).toHaveLength(1);
        expect(data.docs[0].name).toBe('Lease evidence.md');
        const ancestors: string[] = [];
        for (let parent = data.docs[0].parentId; parent;) {
          const folder = data.nodes.find((node) => node.id === parent)!;
          ancestors.unshift(folder.name);
          parent = folder.parentId;
        }
        expect(ancestors).toEqual(['Inbox', 'Local', 'Research', '研究 & evidence']);
        expect(data.texts).toContain(RAW);
        expect(data.texts).not.toContain('# Private sibling\n\nDo not include this document.');
        await recipient.reload();
        await expect(recipient.locator('#docTitle')).toHaveText('Lease evidence.md');
        await expect(recipient.locator('#crumbs')).toHaveText('InboxLocalResearch研究 & evidence');
        await recipient.getByRole('button', { name: 'Actions for Lease evidence.md', exact: true }).click();
        await recipient.getByRole('menuitem', { name: 'Rename', exact: true }).click();
        const localName = recipient.getByRole('textbox', { name: 'Name', exact: true });
        await localName.fill('My local label');
        await localName.press('Enter');
        await boot(recipient, link);
        await expect(recipient.locator('#docTitle')).toHaveText('My local label');
        expect((await stored(recipient)).docs).toHaveLength(1);
        await expect(recipient.locator('#crumbs')).toHaveText('InboxLocalResearch研究 & evidence');
        const temporary = await other.newPage();
        const temporaryUrl = new URL(link);
        temporaryUrl.searchParams.set('temporary', '1');
        await boot(temporary, temporaryUrl.href);
        await expect(temporary.locator('#docTitle')).toHaveText('Lease evidence.md');
        await expect(temporary.locator('#crumbs')).toHaveText('LocalResearch研究 & evidence');
        await temporary.waitForFunction(() => {
          const params = new URLSearchParams(location.hash.slice(1));
          return params.get('t') === '1' && params.has('md') && params.has('p');
        });
        await temporary.reload();
        await expect(temporary.locator('#docTitle')).toHaveText('Lease evidence.md');
        await expect(temporary.locator('#crumbs')).toHaveText('LocalResearch研究 & evidence');
        await expect(temporary.locator('#tree .tree-row[data-kind="doc"]')).toHaveCount(1);
        expect((await stored(recipient)).docs[0].name).toBe('My local label');
      } finally {
        await other.close();
      }
    });
  }

  test('a malformed full-link folder path cannot create documents or partial folders', async ({ page }) => {
    await boot(page);
    await file(page, '# Retained\n\nKeep this document.', 'Retained.md');
    const before = await stored(page);
    const encode = (text: string) => {
      const bytes = Buffer.from(text);
      const encoded = Buffer.concat([Buffer.alloc(4), deflateRawSync(bytes)]);
      encoded.writeUInt32LE(bytes.length, 0);
      return encoded.toString('base64url');
    };
    for (const path of ['["Local",1]', 'null', '{}', 'bad json']) {
      await boot(page, `/#${new URLSearchParams({ name: 'Rejected', md: encode(RAW), p: encode(path) })}`);
      await expect(page.locator('#toast')).toContainText('damaged or incomplete folder path');
      expect((await stored(page)).nodes).toEqual(before.nodes);
    }
  });

  test('every nested folder offers targeted file and folder imports without changing the default', async ({
    page,
  }, testInfo) => {
    await boot(page);
    const folderRow = (name: string) =>
      page
        .getByRole('treeitem')
        .filter({ has: page.getByRole('button', { name: `Actions for ${name}`, exact: true }) });
    let parent: string | null = null;
    for (const name of ['Local', 'Research', 'Deep', 'Evidence']) {
      if (parent) await page.getByRole('button', { name: `Actions for ${parent}`, exact: true }).click();
      else await page.getByRole('button', { name: 'New document, folder, or import', exact: true }).click();
      await page.getByRole('menuitem', { name: 'New folder', exact: true }).click();
      const field = page.getByRole('textbox', { name: 'Name', exact: true });
      await field.fill(name);
      await field.press('Enter');
      await page.getByRole('button', { name: `Actions for ${name}`, exact: true }).click();
      await expect(page.getByRole('menuitem', { name: /^Import files…/ })).toBeEnabled();
      await expect(page.getByRole('menuitem', { name: 'Import a folder…', exact: true })).toBeEnabled();
      await page.keyboard.press('Escape');
      await expect(page.getByRole('button', { name: `Actions for ${name}`, exact: true })).toBeFocused();
      parent = name;
    }
    const target = (await folderRow('Evidence').getAttribute('data-id'))!;
    await page.getByRole('button', { name: 'Actions for Local', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Use as default', exact: true }).click();
    await expect(folderRow('Local')).toHaveAttribute('aria-description', 'Default destination');

    // The context-menu target is not the selected folder or the default destination.
    await folderRow('Research').getByText('Research', { exact: true }).click();
    await folderRow('Research').getByText('Research', { exact: true }).click();
    await folderRow('Evidence').click({ button: 'right' });
    const picking = page.waitForEvent('filechooser');
    await page.getByRole('menuitem', { name: /^Import files…/ }).click();
    const chooser = await picking;
    await folderRow('Local').getByText('Local', { exact: true }).click();
    await chooser.setFiles([
      { name: 'LETTERS.md', mimeType: 'text/markdown', buffer: Buffer.from('# Emails to resolve the meter error\n') },
      { name: 'VALIDATION.md', mimeType: 'text/markdown', buffer: Buffer.from(RAW) },
    ]);
    await expect(page.locator('#toast')).toContainText('Imported 2 documents');
    const imported = (await stored(page)).docs;
    expect(
      imported.map(({ name, parentId }) => ({ name, parentId })).sort((a, b) => a.name.localeCompare(b.name)),
    ).toEqual([
      { name: 'LETTERS', parentId: target },
      { name: 'VALIDATION', parentId: target },
    ]);

    // Reopen the ancestor, then use the action button instead of right-click.
    if ((await folderRow('Local').getAttribute('aria-expanded')) === 'false')
      await folderRow('Local').getByText('Local', { exact: true }).click();
    const directory = testInfo.outputPath('nebenkosten');
    await mkdir(join(directory, 'proofs'), { recursive: true });
    await writeFile(join(directory, 'LEGAL.md'), '# Legal assessment and practical risk\n');
    await writeFile(join(directory, 'proofs', 'EVIDENCE.md'), '# Evidence record\n');
    await page.getByRole('button', { name: 'Actions for Evidence', exact: true }).click();
    const pickingFolder = page.waitForEvent('filechooser');
    await page.getByRole('menuitem', { name: 'Import a folder…', exact: true }).click();
    await (await pickingFolder).setFiles(directory);
    await expect(page.locator('#toast')).toContainText('Imported 2 documents');
    const importedFolder = folderRow('nebenkosten');
    const folderId = (await importedFolder.getAttribute('data-id'))!;
    const proofId = (await folderRow('proofs').getAttribute('data-id'))!;
    await expect(importedFolder).toHaveAttribute('aria-level', '5');
    await expect(folderRow('proofs')).toHaveAttribute('aria-level', '6');
    const docs = (await stored(page)).docs;
    expect(docs.find((doc) => doc.name === 'LEGAL')?.parentId).toBe(folderId);
    expect(docs.find((doc) => doc.name === 'EVIDENCE')?.parentId).toBe(proofId);
    await expect(folderRow('Local')).toHaveAttribute('aria-description', 'Default destination');
  });

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
        await expect(workRow).toHaveAttribute('aria-expanded', String(expanded));
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
    test(`${entry} preserves Markdown bytes and uses the source name`, async ({ page }) => {
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
      await expect(page.locator('#docTitle')).toHaveText(
        ['form', 'query'].includes(entry) ? 'Preferred title' : 'Filename',
      );
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
      await expect(page.locator('#docTitle')).toHaveText(
        ['form', 'query'].includes(entry) ? 'Reopened reader' : 'Reader',
      );
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
      await expect(reopened.locator('#docTitle')).toHaveText('Exported');
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
    await match.getByRole('button', { name: 'Update "Reader"', exact: true }).click();
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
        const id = (await stored(page)).docs.find((doc) => doc.name === 'Stored')!.id;
        await page.evaluate((id) => {
          location.hash = `doc=${id}`;
        }, id);
      }
      const name = next === 'URL' ? 'Winning URL' : next === 'file' ? 'Winner' : 'Stored';
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
