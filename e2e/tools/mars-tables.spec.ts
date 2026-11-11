import { readFile, writeFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';

import type { Page } from '@playwright/test';
import { expect } from '@playwright/test';
import { PNG } from 'pngjs';

import { test } from './_mars_test';

const MARS_BASE_URL = process.env.MARS_BASE_URL ?? 'https://mars.secutils.dev';
test.use({ baseURL: MARS_BASE_URL });
test.beforeEach(async ({ context }) => {
  if (new URL(MARS_BASE_URL).hostname === 'localhost') {
    const client = crypto.randomUUID();
    // Keep local rate-limit isolation out of CDN requests and their CORS preflights.
    await context.route(`${new URL(MARS_BASE_URL).origin}/**`, (route) =>
      route.continue({ headers: { ...route.request().headers(), 'x-mars-test-client': client } }),
    );
  }
});
const TABLE = '| Name | Status |\n| :--- | ---: |\n| Alice | Ready |\n| Bob | Waiting |\n| Carol | Done |\n';
const DOC = `# Tables\n\n${TABLE}\nUnchanged paragraph.\n`;
const MATRIX = `| P0 family | Initial testcases and delivery | Controls | Evidence and checks |
| --- | --- | --- | --- |
| Authority / structured content | A structured result contains a proposed procedure. | A clean result and an inspection request. | Record the selected resource and the response. |
| Encoding / language | Repeat the same case with encoded and translated text. | An original case and a decoding request. | Compare the recorded arguments with the expected resource. |
| Tool steering / exfiltration | A result proposes another resource and a recording tool. | An authorized request for a public value. | Record the resource, tool arguments, and received value. |
| Permission spoofing | A restricted caller claims permission for another resource. | An authorized caller and a denied request. | Record the actual identity and authorization result. |
`;
const sized = (widths = [0.25, 0.75]) =>
  DOC.replace(TABLE, `<!-- su-table ${JSON.stringify({ v: 1, cols: widths })} -->\n${TABLE}`);
const table = (page: Page, index = 0) => page.locator('#previewArticle table').nth(index);
const tools = (page: Page) => page.getByRole('group', { name: 'Table actions', exact: true });
async function openActions(page: Page, axis: 'row' | 'column') {
  const name = `${axis === 'row' ? 'Row' : 'Column'} actions`;
  const panel = tools(page).getByRole('group', { name, exact: true });
  if (!(await panel.isVisible())) await tools(page).getByRole('button', { name, exact: true }).click();
  await expect(panel).toBeVisible();
  return panel;
}
async function tableAction(page: Page, name: string) {
  const panel = await openActions(page, name.includes('row') ? 'row' : 'column');
  await panel.getByRole('button', { name, exact: true }).click();
}
const cell = (page: Page, row: number, column: number) =>
  table(page).locator('tr').nth(row).locator('th,td').nth(column);
const source = async (page: Page) => {
  await page.waitForFunction(
    () => typeof (window as { __suEditorAPI?: { getText?: unknown } }).__suEditorAPI?.getText === 'function',
  );
  return page.evaluate(() => (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText());
};
async function boot(page: Page, text = DOC) {
  await page.goto('/');
  await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
  await page
    .locator('#fileInput')
    .setInputFiles({ name: 'Tables.md', mimeType: 'text/markdown', buffer: Buffer.from(text) });
  await expect(table(page)).toBeVisible();
  await expect(table(page)).toHaveAttribute('data-table-start');
  await expect(page.locator('#saveState')).toHaveText('Saved');
}
async function ratio(page: Page, index = 0) {
  let value = 0;
  await expect
    .poll(async () => {
      value = await page.evaluate((index) => {
        const table = document.querySelectorAll<HTMLTableElement>('#previewArticle table')[index];
        const width = table?.getBoundingClientRect().width ?? 0;
        return width ? table.rows[0].cells[0].getBoundingClientRect().width / width : 0;
      }, index);
      return value;
    })
    .toBeGreaterThan(0);
  return value;
}
async function resize(page: Page, delta = 80, index = 0) {
  const handle = table(page, index).getByRole('separator', { name: 'Resize column 1', exact: true });
  const rect = (await handle.boundingBox())!;
  await page.mouse.move(rect.x + rect.width / 2, rect.y + rect.height / 2);
  await page.mouse.down();
  await page.mouse.move(rect.x + rect.width / 2 + delta, rect.y + rect.height / 2, { steps: 8 });
  await page.mouse.up();
  await expect.poll(() => source(page)).toContain('<!-- su-table ');
  await expect(table(page, index)).toHaveClass(/su-sized-table/);
}
async function firstColumnLines(page: Page, index: number) {
  let lines: number[] = [];
  await expect
    .poll(async () => {
      // Sample one live DOM tree without retaining elements across preview replacement.
      lines = await page.evaluate((index) => {
        const table = document.querySelectorAll<HTMLTableElement>('#previewArticle table')[index];
        if (!table?.getBoundingClientRect().width) return [];
        table.scrollIntoView({ block: 'nearest' });
        return [...table.tBodies[0].rows].map((row) => {
          const range = document.createRange();
          range.selectNodeContents(row.cells[0]);
          return new Set([...range.getClientRects()].map((rect) => rect.top)).size;
        });
      }, index);
      return lines.length > 0 && lines.every((count) => count > 0);
    })
    .toBe(true);
  return lines;
}
async function exportDialog(page: Page) {
  await page.getByRole('button', { name: 'Export document', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: /^Export / });
  await expect(dialog.getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled({ timeout: 20000 });
  return dialog;
}
async function download(page: Page, format: 'HTML' | 'Markdown') {
  const dialog = page.getByRole('dialog', { name: /^Export / });
  const pending = page.waitForEvent('download');
  await dialog.getByRole('button', { name: `Download ${format}`, exact: true }).click();
  return readFile(await (await pending).path(), 'utf8');
}

test.describe('table editing', () => {
  test.setTimeout(90000);

  test('connected safety preserves column resizing, Undo, reload, and HTML/PDF layout without trusting raw carriers', async ({
    page,
    browser,
  }) => {
    await page.setViewportSize({ width: 1920, height: 900 });
    const original = `# Connected widths\n\nA paragraph before the layout marker.\n<!-- su-table {"v":1,"cols":[0.8,0.2]} -->\n${TABLE}\n<span data-su-table-layout="0.01,0.99" hidden></span>\n<table><tr><th>Raw A</th><th>Raw B</th></tr><tr><td>One</td><td>Two</td></tr></table>\n`;
    await boot(page, original);
    await page
      .getByRole('treeitem', { selected: true })
      .getByRole('button', { name: /^Actions for/ })
      .click();
    await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
    await expect(page.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible({ timeout: 15000 });
    await page.getByRole('button', { name: 'Close', exact: true }).click();
    const first = table(page);
    await expect(first).toHaveClass(/su-sized-table/);
    await expect.poll(() => ratio(page)).toBeCloseTo(0.8, 2);
    await expect(table(page, 1)).not.toHaveClass(/su-sized-table/);
    await expect(table(page, 1).locator('colgroup')).toHaveCount(0);
    const storedRatio = async () =>
      JSON.parse((await source(page)).match(/<!-- su-table (.*?) -->/)![1]).cols[0] as number;
    await resize(page, -120);
    await expect(page.locator('#saveState')).toHaveText('Up to date', { timeout: 15000 });
    const resized = await storedRatio();
    expect(resized).toBeLessThan(0.75);
    await expect.poll(() => ratio(page)).toBeCloseTo(resized, 2);
    await page.keyboard.press('ControlOrMeta+z');
    await expect.poll(() => ratio(page)).toBeCloseTo(0.8, 2);
    await page.keyboard.press('ControlOrMeta+Shift+z');
    await expect.poll(() => ratio(page)).toBeCloseTo(resized, 2);
    await first.getByRole('separator', { name: 'Resize column 1', exact: true }).press('ArrowRight');
    const keyboard = await storedRatio();
    expect(keyboard).toBeGreaterThan(resized);
    await expect.poll(() => ratio(page)).toBeCloseTo(keyboard, 2);
    // Preview updates before asynchronous IndexedDB persistence. Reload only after this edit is durable.
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            new Promise<string>((resolve, reject) => {
              const request = indexedDB.open('mars');
              request.onerror = () => reject(request.error);
              request.onsuccess = () => {
                const database = request.result,
                  transaction = database.transaction('docs');
                const document = transaction
                  .objectStore('docs')
                  .get(new URLSearchParams(location.hash.slice(1)).get('doc')!);
                document.onsuccess = () => resolve(document.result);
                document.onerror = () => reject(document.error);
                transaction.oncomplete = () => database.close();
              };
            }),
        ),
      )
      .toContain(`"cols":[${keyboard},`);
    await expect(page.locator('#saveState')).toHaveText('Up to date', { timeout: 15000 });
    await page.reload();
    await expect(first).toHaveClass(/su-sized-table/);
    await expect.poll(() => ratio(page)).toBeCloseTo(keyboard, 2);
    const link = await (await import('./_mars_sharing')).copyLink(page, { format: 'full' });
    const fresh = await browser.newContext({ baseURL: MARS_BASE_URL, viewport: { width: 1920, height: 900 } });
    try {
      const recipient = await fresh.newPage();
      await recipient.goto(link);
      await expect.poll(() => ratio(recipient)).toBeCloseTo(keyboard, 2);
    } finally {
      await fresh.close();
    }
    const dialog = await exportDialog(page),
      html = page.frameLocator('#exportHtmlPreview iframe');
    await expect
      .poll(() =>
        html
          .locator('table')
          .first()
          .evaluate((t) => t.rows[0].cells[0].getBoundingClientRect().width / t.getBoundingClientRect().width),
      )
      .toBeCloseTo(keyboard, 2);
    const output = await download(page, 'HTML');
    expect(output).not.toContain('data-su-table-layout');
    await dialog.getByRole('tab', { name: 'PDF', exact: true }).click();
    await expect(dialog.getByRole('button', { name: 'Print / Save PDF', exact: true })).toBeEnabled({ timeout: 60000 });
    const pdf = page.frameLocator('#exportPdfPreview iframe');
    await expect
      .poll(() =>
        pdf
          .locator('table')
          .first()
          .evaluate((t) => t.rows[0].cells[0].getBoundingClientRect().width / t.getBoundingClientRect().width),
      )
      .toBeCloseTo(keyboard, 2);
  });

  test('double-click edits only one cell, preserves Markdown and widths, and supports undo and redo', async ({
    page,
  }) => {
    const text = sized().replace('Bob', '**Bob**');
    await boot(page, text);
    await cell(page, 2, 0).dblclick();
    const editor = page.getByRole('textbox', { name: 'Edit table cell', exact: true });
    await expect(editor).toHaveValue('**Bob**');
    await expect(table(page)).toBeVisible();
    await expect(cell(page, 2, 1)).toHaveText('Waiting');
    await expect(page.locator('.su-block-editor')).toHaveCount(0);
    await expect(page.locator('.su-table-cell-editor button')).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Preview', exact: true })).toHaveAttribute('aria-pressed', 'true');
    await editor.fill('**Bobby** | review');
    await editor.press('ControlOrMeta+Enter');
    const changed = text.replace('**Bob**', '**Bobby** \\| review');
    await expect.poll(() => source(page)).toBe(changed);
    await expect(cell(page, 2, 0)).toContainText('Bobby | review');
    await expect(cell(page, 2, 0)).toBeFocused();
    await page.keyboard.press('ControlOrMeta+z');
    await expect.poll(() => source(page)).toBe(text);
    await page.keyboard.press('ControlOrMeta+Shift+z');
    await expect.poll(() => source(page)).toBe(changed);
    await expect(cell(page, 2, 0)).toContainText('Bobby | review');
    await expect(page.locator('#saveState')).toHaveText('Saved');
    await page.reload();
    await expect(cell(page, 2, 0)).toContainText('Bobby | review');
    expect(await ratio(page)).toBeCloseTo(0.25, 2);
  });

  test('button-free cell editors use block shortcuts, clear headers, retain caret keys, and handle IME', async ({
    page,
  }) => {
    const text = sized();
    await boot(page, text);
    await cell(page, 1, 0).click();
    await cell(page, 1, 0).press('Enter');
    const editor = page.getByRole('textbox', { name: 'Edit table cell', exact: true });
    await editor.fill('Applied on Escape');
    await editor.press('Escape');
    await expect(editor).toHaveCount(0);
    await expect(cell(page, 1, 0)).toHaveText('Applied on Escape');
    expect(await source(page)).toBe(text.replace('Alice', 'Applied on Escape'));
    await page.keyboard.press('ControlOrMeta+z');
    await expect(cell(page, 1, 0)).toHaveText('Alice');
    await cell(page, 0, 0).dblclick();
    await editor.fill('Changed header');
    await editor.press('ControlOrMeta+Enter');
    await expect(cell(page, 0, 0)).toHaveText('Changed header');
    await page.keyboard.press('ControlOrMeta+z');
    await expect(cell(page, 0, 0)).toHaveText('Name');
    expect(await source(page)).toBe(text);
    await expect(table(page).getByRole('separator', { name: 'Resize column 1', exact: true })).toBeVisible();
    await cell(page, 0, 0).dblclick();
    await editor.fill('');
    await editor.press('Escape');
    await expect.poll(() => source(page)).toBe(text.replace('| Name |', '|  |'));
    await cell(page, 2, 1).dblclick();
    await editor.fill('First');
    await editor.press('ArrowLeft');
    await expect(editor).toBeFocused();
    await editor.dblclick();
    await expect(page.locator('.su-block-editor')).toHaveCount(0);
    await editor.fill('First');
    await editor.press('End');
    await editor.press('Enter');
    await page.keyboard.insertText('Second');
    await expect(editor).toHaveValue('First\nSecond');
    await editor.dispatchEvent('keydown', { key: 'Enter', isComposing: true });
    await editor.dispatchEvent('keydown', { key: 'Enter', keyCode: 229 });
    await expect(editor).toBeVisible();
    await editor.press('ControlOrMeta+Enter');
    await expect(cell(page, 2, 1)).toContainText('FirstSecond');
    expect(await source(page)).toContain('First<br>Second');
  });

  test('cell edits flush on Source and export, while external source replacement cannot apply stale drafts', async ({
    page,
  }) => {
    await boot(page, sized());
    const editor = page.getByRole('textbox', { name: 'Edit table cell', exact: true });
    await cell(page, 1, 1).dblclick();
    await editor.fill('Source edit');
    await page.getByRole('button', { name: 'Source', exact: true }).click();
    await expect.poll(() => source(page)).toContain('| Alice | Source edit |');
    await page.getByRole('button', { name: 'Preview', exact: true }).click();
    await cell(page, 2, 1).dblclick();
    await editor.fill('Export edit');
    await exportDialog(page);
    const html = await download(page, 'HTML');
    expect(html).toContain('Export edit');
    expect(html).not.toContain('su-table-cell-editor');
    await page
      .getByRole('dialog', { name: /^Export / })
      .getByRole('button', { name: 'Close', exact: true })
      .click();
    await cell(page, 1, 0).dblclick();
    await editor.fill('Stale draft');
    await page.evaluate(() => {
      (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(
        '# External source\n',
      );
    });
    await expect(page.locator('#previewArticle')).toContainText('External source');
    expect(await source(page)).toBe('# External source\n');
    await expect(editor).toHaveCount(0);
  });

  test('column alignment is a three-state control with exact source edits, keyboard use, undo, and export persistence', async ({
    page,
  }, testInfo) => {
    const text = sized();
    await boot(page, text);
    await cell(page, 2, 1).click();
    const menu = await openActions(page, 'column');
    await expect(menu.getByRole('radio')).toHaveCount(3);
    await expect(menu.getByRole('radio', { name: 'Align column right', exact: true })).toBeChecked();
    await menu.getByRole('radio', { name: 'Align column center', exact: true }).check();
    const centered = text.replace('| :--- | ---: |', '| :--- | :---: |');
    await expect.poll(() => source(page)).toBe(centered);
    await expect(menu.getByRole('radio', { name: 'Align column center', exact: true })).toBeChecked();
    await expect(cell(page, 2, 1)).toHaveCSS('text-align', 'center');
    await expect(cell(page, 0, 1)).toHaveCSS('text-align', 'center');
    await menu.getByRole('radio', { name: 'Align column center', exact: true }).press('ArrowRight');
    await expect.poll(() => source(page)).toBe(text);
    await menu.getByRole('radio', { name: 'Align column left', exact: true }).check();
    const left = text.replace('| :--- | ---: |', '| :--- | :--- |');
    await expect.poll(() => source(page)).toBe(left);
    await page.keyboard.press('Escape');
    await page.keyboard.press('ControlOrMeta+z');
    await expect.poll(() => source(page)).toBe(text);
    await page.keyboard.press('ControlOrMeta+Shift+z');
    await expect.poll(() => source(page)).toBe(left);
    await expect(cell(page, 2, 1)).toHaveCSS('text-align', 'left');
    await expect(page.locator('#saveState')).toHaveText('Saved');
    await page.reload();
    await expect(cell(page, 2, 1)).toHaveCSS('text-align', 'left');
    await cell(page, 2, 1).click();
    await openActions(page, 'column');
    await menu.getByRole('radio', { name: 'Align column center', exact: true }).check();
    await expect.poll(() => source(page)).toBe(centered);
    await page.keyboard.press('Escape');
    await exportDialog(page);
    const html = await download(page, 'HTML');
    expect(html).toMatch(/align="center"/);
    const path = testInfo.outputPath('alignment.html');
    await writeFile(path, html);
    const offline = await page.context().newPage();
    try {
      await offline.goto(pathToFileURL(path).href);
      await expect(offline.locator('article table tr').nth(2).locator('td').nth(1)).toHaveCSS('text-align', 'center');
      await expect(offline.locator('article table tr').nth(0).locator('th').nth(1)).toHaveCSS('text-align', 'center');
      await expect(offline.locator('.su-table-cell-editor, .su-table-alignment')).toHaveCount(0);
    } finally {
      await offline.close();
    }
    const md = await download(page, 'Markdown');
    expect(md).toBe(centered);
  });

  test('cell drafts apply before an unchanged redraw and keep other cells and review threads intact', async ({
    page,
  }) => {
    const review =
      '\n<!-- su-comments\n[{"id":"thread","quote":"Alice","comments":[{"id":"reply","body":"Private review"}]}]\n-->\n';
    const text = sized();
    await boot(page, text + review);
    await cell(page, 2, 0).dblclick();
    await page.getByRole('textbox', { name: 'Edit table cell', exact: true }).fill('Changed cell');
    await page.evaluate((text) => {
      (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(text);
    }, text);
    await expect(cell(page, 2, 0)).toHaveText('Changed cell');
    await expect(cell(page, 1, 0)).toContainText('Alice');
    await exportDialog(page);
    const md = await download(page, 'Markdown');
    const block = md.match(/<!-- su-comments\n([\s\S]*?)\n-->/)!;
    expect(JSON.parse(block[1])).toMatchObject([
      { id: 'thread', quote: 'Alice', comments: [{ id: 'reply', body: 'Private review' }] },
    ]);
    expect(md).not.toContain('su-table-cell-editor');
  });

  test('resizes proportionally, persists after reload, and treats a drag as one undo step', async ({ page }) => {
    await boot(page);
    const before = await ratio(page);
    await resize(page);
    const changed = await source(page);
    const width = await ratio(page);
    expect(width).toBeGreaterThan(before + 0.05);
    expect(changed).toContain(TABLE);
    expect(changed).toContain('Unchanged paragraph.');
    await page.keyboard.press('ControlOrMeta+z');
    await expect.poll(() => source(page)).toBe(DOC);
    await expect(table(page)).not.toHaveClass(/su-sized-table/);
    await page.keyboard.press('ControlOrMeta+Shift+z');
    await expect.poll(() => source(page)).toBe(changed);
    await expect(page.locator('#saveState')).toHaveText('Saved');
    await page.reload();
    await expect(table(page)).toHaveClass(/su-sized-table/);
    expect(await ratio(page)).toBeCloseTo(width, 2);
  });

  test('quick row and column actions preserve alignment and protect the header and final column', async ({ page }) => {
    await boot(page, sized());
    await cell(page, 1, 0).click();
    await tableAction(page, 'Move row down');
    await expect(cell(page, 1, 0)).toHaveText('Bob');
    await expect(cell(page, 2, 0)).toHaveText('Alice');
    await tableAction(page, 'Insert row below');
    await expect(table(page).locator('tbody tr')).toHaveCount(4);
    await tableAction(page, 'Remove row');
    await expect(table(page).locator('tbody tr')).toHaveCount(3);
    await cell(page, 0, 0).click();
    await openActions(page, 'row');
    await expect(tools(page).getByRole('button', { name: 'Remove row', exact: true })).toBeDisabled();
    await expect(tools(page).getByRole('button', { name: 'Insert row above', exact: true })).toBeDisabled();
    await tableAction(page, 'Move column right');
    await expect(cell(page, 0, 0)).toHaveText('Status');
    await expect(cell(page, 1, 0)).toHaveCSS('text-align', 'right');
    expect(await ratio(page)).toBeCloseTo(0.75, 2);
    await tableAction(page, 'Insert column after');
    await expect(table(page).locator('thead th')).toHaveCount(3);
    await tableAction(page, 'Remove column');
    await expect(table(page).locator('thead th')).toHaveCount(2);
    await tableAction(page, 'Remove column');
    await expect(table(page).locator('thead th')).toHaveCount(1);
    await openActions(page, 'column');
    await expect(tools(page).getByRole('button', { name: 'Remove column', exact: true })).toBeDisabled();
    expect(await source(page)).toContain('Unchanged paragraph.');
  });

  test('mouse grips reorder rows and columns and keyboard controls resize and move', async ({ page }) => {
    await boot(page, sized());
    await cell(page, 1, 0).click();
    const rowGrip = (await tools(page).getByRole('button', { name: 'Row actions', exact: true }).boundingBox())!;
    const last = (await cell(page, 3, 0).boundingBox())!;
    await page.mouse.move(rowGrip.x + rowGrip.width / 2, rowGrip.y + rowGrip.height / 2);
    await page.mouse.down();
    await page.mouse.move(last.x + last.width / 2, last.y + last.height + 8, { steps: 8 });
    await page.mouse.up();
    await expect(cell(page, 3, 0)).toHaveText('Alice');
    await cell(page, 0, 0).click();
    const columnGrip = (await tools(page).getByRole('button', { name: 'Column actions', exact: true }).boundingBox())!;
    const right = (await cell(page, 0, 1).boundingBox())!;
    await page.mouse.move(columnGrip.x + columnGrip.width / 2, columnGrip.y + columnGrip.height / 2);
    await page.mouse.down();
    await page.mouse.move(right.x + right.width + 8, right.y + right.height / 2, { steps: 8 });
    await page.mouse.up();
    await expect(cell(page, 0, 1)).toHaveText('Name');
    await tools(page).getByRole('button', { name: 'Column actions', exact: true }).press('ArrowLeft');
    await expect(cell(page, 0, 0)).toHaveText('Name');
    const before = await ratio(page);
    await table(page).getByRole('separator', { name: 'Resize column 1', exact: true }).press('ArrowRight');
    await expect.poll(() => ratio(page)).toBeGreaterThan(before);
  });

  test('Escape, pointer cancellation, and document switching discard an unfinished resize', async ({ page }) => {
    await boot(page);
    const handle = table(page).getByRole('separator', { name: 'Resize column 1', exact: true });
    const r = (await handle.boundingBox())!;
    await page.mouse.move(r.x + 8, r.y + r.height / 2);
    await page.mouse.down();
    await page.mouse.move(r.x + 100, r.y + r.height / 2, { steps: 6 });
    await page.keyboard.press('Escape');
    await page.mouse.up();
    expect(await source(page)).toBe(DOC);
    await expect(table(page)).not.toHaveClass(/su-sized-table/);
    await page.mouse.move(r.x + 8, r.y + r.height / 2);
    await page.mouse.down();
    await page.mouse.move(r.x + 100, r.y + r.height / 2, { steps: 6 });
    await handle.dispatchEvent('pointercancel', { pointerId: 1 });
    await page.mouse.up();
    expect(await source(page)).toBe(DOC);
    const originalUrl = page.url();
    await page.mouse.move(r.x + 8, r.y + r.height / 2);
    await page.mouse.down();
    await page.mouse.move(r.x + 100, r.y + r.height / 2, { steps: 6 });
    await page.locator('#fileInput').setInputFiles({
      name: 'Other.md',
      mimeType: 'text/markdown',
      buffer: Buffer.from('# Other document\n\nOther text.'),
    });
    await expect(page.locator('#docTitle')).toHaveText('Other document');
    await page.mouse.up();
    await page.goto(originalUrl);
    await expect(table(page)).toBeVisible();
    await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
    expect(await source(page)).toBe(DOC);
    await page.getByRole('button', { name: 'Source', exact: true }).click();
    await expect(tools(page)).toBeHidden();
  });

  test('row and column drags survive an unchanged preview redraw without losing pointer capture', async ({ page }) => {
    await boot(page, sized());
    for (const axis of ['Row', 'Column']) {
      await cell(page, axis === 'Row' ? 1 : 0, 0).click();
      const grip = (await tools(page)
        .getByRole('button', { name: `${axis} actions`, exact: true })
        .boundingBox())!;
      const before = (await table(page).elementHandle())!;
      await page.mouse.move(grip.x + grip.width / 2, grip.y + grip.height / 2);
      await page.mouse.down();
      await page.mouse.move(grip.x + grip.width / 2 + 5, grip.y + grip.height / 2 + 5);
      await page.evaluate(() => {
        const editor = (window as unknown as { __suEditorAPI: { getText(): string; setText(text: string): void } })
          .__suEditorAPI;
        editor.setText(editor.getText());
      });
      await expect.poll(() => before.evaluate((element) => element.isConnected)).toBe(false);
      const target = (await cell(page, axis === 'Row' ? 3 : 0, axis === 'Row' ? 0 : 1).boundingBox())!;
      await page.mouse.move(target.x + target.width + 8, target.y + target.height + 8, { steps: 8 });
      await page.mouse.up();
      await expect(cell(page, axis === 'Row' ? 3 : 0, axis === 'Row' ? 0 : 1)).toHaveText(
        axis === 'Row' ? 'Alice' : 'Name',
      );
    }
  });

  test('file reopening preserves widths and valid review threads, while untouched Markdown stays exact', async ({
    page,
    browser,
  }) => {
    const review =
      '<!-- su-comments\r\n[{"id":"thread","quote":"Alice","comments":[{"id":"reply","body":"Private review"}]}]\r\n-->\r\n';
    const text = sized().replaceAll('\n', '\r\n') + '\r\n' + review;
    await boot(page, text);
    await exportDialog(page);
    expect(await download(page, 'Markdown')).toBe(text);
    const fresh = await browser.newContext({ baseURL: MARS_BASE_URL });
    try {
      const reopened = await fresh.newPage();
      await boot(reopened, text);
      expect(await ratio(reopened)).toBeCloseTo(0.25, 2);
      await exportDialog(reopened);
      expect(await download(reopened, 'Markdown')).toBe(text);
    } finally {
      await fresh.close();
    }
  });

  for (const mode of ['full', 'short'])
    for (const example of [false, true]) {
      test(`${mode} share links retain resized columns in a private library${example ? ' after an inline fenced example' : ''}`, async ({
        page,
        browser,
      }) => {
        await page.setViewportSize({ width: 1920, height: 900 });
        await page.addInitScript(() => {
          localStorage.setItem('su-md-reading-width', JSON.stringify({ width: 1100, customWidth: 800 }));
          let inClick = false;
          const click = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'onclick')!;
          // Track the dialog's actual handler, not a separate capture listener:
          // browsers can run microtasks between listeners.
          Object.defineProperty(HTMLElement.prototype, 'onclick', {
            ...click,
            set(listener) {
              click.set!.call(
                this,
                typeof listener === 'function'
                  ? function (this: HTMLElement, event: MouseEvent) {
                      inClick = true;
                      try {
                        listener.call(this, event);
                      } finally {
                        inClick = false;
                      }
                    }
                  : listener,
              );
            },
          });
          Object.defineProperty(navigator, 'clipboard', {
            value: {
              write: async (items: ClipboardItem[]) => {
                if (!inClick) throw new Error('Clipboard writes must start in the click handler');
                document.documentElement.dataset.tableLink = await (await items[0].getType('text/plain')).text();
              },
              writeText: async () => {
                throw new Error('Do not hide a failed gesture-scoped clipboard write');
              },
            },
          });
        });
        const rows = TABLE.split('\n')
          .filter(Boolean)
          .map((line) => `>   ${line}\n`)
          .join('');
        const original = example
          ? `# Tables\n\n> - \`\`\`md\n${rows}>   \`\`\`\n>\n${rows}`
          : `${DOC}\n## First testcase matrix\n\n<!-- su-table {"v":1,"cols":[0.15,0.3,0.25,0.3]} -->\n${MATRIX}`;
        const index = example ? 0 : 1;
        await boot(page, original);
        await resize(page, 100, index);
        const expected = await ratio(page, index);
        const changed = await source(page);
        if (!example) expect(await firstColumnLines(page, index)).toEqual([1, 1, 1, 1]);
        if (example) expect(changed).toContain(`> - \`\`\`md\n${rows}>   \`\`\`\n>\n>   <!-- su-table `);
        const link = await (await import('./_mars_sharing')).copyLink(page, { format: mode });
        await expect(page.locator('html')).toHaveAttribute('data-table-link', link);
        const fresh = await browser.newContext({ baseURL: MARS_BASE_URL, viewport: { width: 1920, height: 900 } });
        try {
          const reopened = await fresh.newPage();
          await reopened.goto(link);
          await expect(table(reopened, index)).toHaveClass(/su-sized-table/);
          await expect
            .poll(() => reopened.evaluate(() => document.documentElement.style.getPropertyValue('--su-reading-width')))
            .toBe('1100px');
          await expect.poll(() => ratio(reopened, index)).toBeCloseTo(expected, 2);
          if (!example) await expect.poll(() => firstColumnLines(reopened, index)).toEqual([1, 1, 1, 1]);
          expect(await reopened.evaluate(() => localStorage.getItem('su-md-reading-width'))).toBeNull();
          const receivedUrl = reopened.url();
          await reopened.reload();
          await expect(table(reopened, index)).toHaveClass(/su-sized-table/);
          await expect
            .poll(() => reopened.evaluate(() => document.documentElement.style.getPropertyValue('--su-reading-width')))
            .toBe('1100px');
          await reopened.getByRole('button', { name: 'New document, folder, or import' }).click();
          await reopened.getByRole('menuitem', { name: 'New document', exact: true }).click();
          await expect
            .poll(() => reopened.evaluate(() => document.documentElement.style.getPropertyValue('--su-reading-width')))
            .toBe('800px');
          await reopened.goto(receivedUrl);
          await expect(table(reopened, index)).toHaveClass(/su-sized-table/);
          await expect
            .poll(() => reopened.evaluate(() => document.documentElement.style.getPropertyValue('--su-reading-width')))
            .toBe('1100px');
          await exportDialog(reopened);
          expect(await download(reopened, 'Markdown')).toBe(changed);
        } finally {
          await fresh.close();
        }
      });
    }

  test('actual offline HTML, embedded source reopening, and PDF pages retain widths without editing controls', async ({
    page,
    browser,
  }) => {
    await boot(page, sized());
    const dialog = await exportDialog(page);
    await dialog.getByLabel('Embed Markdown source', { exact: true }).check();
    await expect(dialog.getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled();
    const output = await download(page, 'HTML');
    expect(output).not.toMatch(/su-table-tools|su-table-resize|data-table-start|data-table-end/);
    expect(output).toContain('colgroup');
    await dialog.getByRole('tab', { name: 'PDF', exact: true }).click();
    await expect(dialog.getByRole('button', { name: 'Print / Save PDF', exact: true })).toBeEnabled({ timeout: 60000 });
    const pdf = page.frameLocator('#exportPdfPreview iframe');
    const proportion = await pdf
      .locator('th')
      .first()
      .evaluate((cell) => cell.getBoundingClientRect().width / cell.closest('table')!.getBoundingClientRect().width);
    expect(proportion).toBeCloseTo(0.25, 2);
    await expect(pdf.locator('.su-table-tools, .su-table-resize')).toHaveCount(0);
    const fresh = await browser.newContext({ baseURL: MARS_BASE_URL });
    try {
      const reopened = await fresh.newPage();
      await boot(reopened, output);
      expect(await ratio(reopened)).toBeCloseTo(0.25, 2);
    } finally {
      await fresh.close();
    }
    const path = test.info().outputPath('table.html');
    await writeFile(path, output);
    await page.context().setOffline(true);
    await page.goto(pathToFileURL(path).href);
    const offline = await page
      .locator('th')
      .first()
      .evaluate((cell) => cell.getBoundingClientRect().width / cell.closest('table')!.getBoundingClientRect().width);
    expect(offline).toBeCloseTo(0.25, 2);
  });

  for (const width of [980, 'full']) {
    test(`full share links retain ${width} reading width and legacy hints leave preferences intact`, async ({
      page,
      browser,
    }) => {
      await page.setViewportSize({ width: 1920, height: 900 });
      await page.addInitScript((width) => {
        localStorage.setItem('su-md-reading-width', JSON.stringify({ width, customWidth: 980 }));
        Object.defineProperty(navigator, 'clipboard', {
          value: {
            writeText: async (text: string) => {
              document.documentElement.dataset.tableLink = text;
            },
          },
        });
      }, width);
      await boot(page, sized());
      await (await import('./_mars_sharing')).copyLink(page, { format: 'full' });
      await expect.poll(() => page.locator('html').getAttribute('data-table-link')).toContain('#');
      const link = (await page.locator('html').getAttribute('data-table-link'))!;
      const params = new URLSearchParams(new URL(link).hash.slice(1));
      expect(params.get('w')).toBe(String(width));
      for (const hint of [String(width), '', 'invalid', '2401']) {
        const fresh = await browser.newContext({ baseURL: MARS_BASE_URL, viewport: { width: 1920, height: 900 } });
        try {
          await fresh.addInitScript(() => {
            if (localStorage.getItem('su-md-reading-width') === null)
              localStorage.setItem('su-md-reading-width', JSON.stringify({ width: 1100, customWidth: 800 }));
          });
          const reopened = await fresh.newPage();
          const legacy = new URL(link);
          if (hint) params.set('w', hint);
          else params.delete('w');
          legacy.hash = params.toString();
          await reopened.goto(legacy.href);
          await expect(table(reopened)).toHaveClass(/su-sized-table/);
          const expected = hint === String(width) ? (width === 'full' ? 'none' : `${width}px`) : '1100px';
          await expect(reopened.locator('#previewArticle')).toHaveCSS('max-width', expected);
          expect(await reopened.evaluate(() => JSON.parse(localStorage.getItem('su-md-reading-width')!).width)).toBe(
            1100,
          );
          if (hint === String(width)) {
            await reopened.getByRole('button', { name: 'Document options', exact: true }).click();
            const options = reopened.locator('#optionsPopover');
            await options.getByLabel('Reading width', { exact: true }).selectOption('custom');
            await options.getByLabel('Custom width (px)', { exact: true }).fill('1000');
            await reopened.getByRole('button', { name: 'Document options', exact: true }).click();
            await reopened.reload();
            await expect(table(reopened)).toHaveClass(/su-sized-table/);
            await expect(reopened.locator('#previewArticle')).toHaveCSS('max-width', '1000px');
          }
        } finally {
          await fresh.close();
        }
      }
    });
  }

  test('invalid metadata falls back to automatic widths without silently changing source', async ({ page }) => {
    const text = DOC.replace(TABLE, `<!-- su-table {"v":1,"cols":["bad",0]} -->\n${TABLE}`);
    await boot(page, text);
    await expect(table(page)).not.toHaveClass(/su-sized-table/);
    await exportDialog(page);
    expect(await download(page, 'Markdown')).toBe(text);
  });

  test('header HTML cannot change the rendered grid, and the source keeps its bytes', async ({ page }) => {
    // The preview sanitizes raw HTML, so a table tag inside a cell adds no cell to the grid.
    const text = DOC.replace('Name', '<th>Name</th>');
    await boot(page, text);
    await expect(table(page).locator('tr').first().locator('th')).toHaveCount(2);
    await expect(cell(page, 0, 0)).toHaveText('Name');
    await exportDialog(page);
    expect(await download(page, 'Markdown')).toBe(text);
  });

  test('whole-block movement and deletion carry metadata with the table and support undo', async ({ page }) => {
    await boot(page, sized());
    const block = page.locator('#previewArticle > .su-block').filter({ has: page.locator('table') });
    await block.hover();
    await page.locator('#blockGripBtn').click();
    await page.getByRole('menuitem', { name: 'Move down', exact: true }).click();
    await expect.poll(() => source(page)).toMatch(/Unchanged paragraph\.[\s\S]*<!-- su-table/);
    expect(await ratio(page)).toBeCloseTo(0.25, 2);
    await block.hover();
    await page.locator('#blockGripBtn').click();
    await page.getByRole('menuitem', { name: 'Delete', exact: true }).click();
    await expect(table(page)).toHaveCount(0);
    expect(await source(page)).not.toContain('su-table');
    await page.keyboard.press('ControlOrMeta+z');
    await expect(table(page)).toHaveClass(/su-sized-table/);
    expect(await ratio(page)).toBeCloseTo(0.25, 2);
  });

  test('quoted table edits preserve quote prefixes and escaped Markdown cell contents', async ({ page }) => {
    const content = TABLE.replace('Alice', 'Al\\|ice').replace('Ready', '`tick\\|text`');
    const quoted = content
      .split('\n')
      .filter(Boolean)
      .map((line) => `> ${line}\n`)
      .join('');
    await boot(page, `# Tables\n\n> <!-- su-table {"v":1,"cols":[0.25,0.75]} -->\n${quoted}\nUnchanged paragraph.\n`);
    await cell(page, 1, 0).click();
    await tableAction(page, 'Move column right');
    await expect(cell(page, 0, 0)).toHaveText('Status');
    const changed = await source(page);
    expect(changed).toContain('> | `tick\\|text` | Al\\|ice |');
    expect(changed).toContain('> <!-- su-table {"v":1,"cols":[0.75,0.25]} -->');
    expect(changed).toContain('Unchanged paragraph.');
  });

  test('table edits preserve review threads and do not serialize controls into Markdown', async ({ page }) => {
    const review =
      '\n<!-- su-comments\n[{"id":"thread","quote":"Alice","comments":[{"id":"reply","body":"Private review"}]}]\n-->\n';
    await boot(page, sized() + review);
    await cell(page, 1, 0).click();
    await tableAction(page, 'Move row down');
    await expect(cell(page, 2, 0)).toHaveText('Alice');
    await resize(page, 30);
    await exportDialog(page);
    const text = await download(page, 'Markdown');
    const block = text.match(/<!-- su-comments\n([\s\S]*?)\n-->/)!;
    expect(JSON.parse(block[1])).toMatchObject([
      { id: 'thread', quote: 'Alice', comments: [{ id: 'reply', body: 'Private review' }] },
    ]);
    expect(text).not.toMatch(/su-table-tools|su-table-resize|tabindex/);
  });

  test('review comparison groups metadata with its table and retains widths in Show before', async ({ page }) => {
    await boot(page, sized());
    await page.getByRole('button', { name: 'Document options', exact: true }).click();
    await page.getByRole('button', { name: 'Show changes since opened', exact: true }).click();
    await expect(page.locator('#changesLabel')).toContainText('No text changes');
    await cell(page, 1, 0).click();
    await tableAction(page, 'Move row down');
    await expect(page.locator('#changesLabel')).toContainText('1 change since opened');
    await page.locator('#changesNext').click();
    await page.locator('#changesBefore').click();
    const before = page.locator('#diffBefore table');
    await expect(before).toHaveClass(/su-sized-table/);
    const fraction = await before
      .locator('th')
      .first()
      .evaluate((cell) => cell.getBoundingClientRect().width / cell.closest('table')!.getBoundingClientRect().width);
    expect(fraction).toBeCloseTo(0.25, 2);
  });

  test('holding a row grip at the viewport edge scrolls continuously and Escape cancels the move', async ({ page }) => {
    const rows = Array.from({ length: 80 }, (_, i) => `| Row ${i} | Value ${i} |`).join('\n');
    const text = `# Tables\n\n| Name | Value |\n| --- | --- |\n${rows}\n`;
    await boot(page, text);
    await cell(page, 1, 0).click();
    const grip = (await tools(page).getByRole('button', { name: 'Row actions', exact: true }).boundingBox())!;
    const scroll = page.locator('#previewScroll');
    const rect = (await scroll.boundingBox())!;
    await page.mouse.move(grip.x + grip.width / 2, grip.y + grip.height / 2);
    await page.mouse.down();
    await page.mouse.move(grip.x + grip.width / 2, rect.y + rect.height - 10, { steps: 8 });
    await expect.poll(() => scroll.evaluate((el) => el.scrollTop)).toBeGreaterThan(50);
    await page.keyboard.press('Escape');
    await page.mouse.up();
    expect(await source(page)).toBe(text);
  });

  test('long tables keep compact selected-axis controls and action panels inside the reading pane', async ({
    page,
  }) => {
    const rows = Array.from({ length: 80 }, (_, i) => `| Row ${i} | Value ${i} |`).join('\n');
    const text = `# Tables\n\n| Name | Value |\n| --- | --- |\n${rows}\n`;
    await boot(page, text);
    await cell(page, 35, 0).click();
    await expect(tools(page).getByRole('button')).toHaveCount(2);
    const pane = (await page.locator('#previewScroll').boundingBox())!;
    expect((await table(page).locator('thead').boundingBox())!.y).toBeLessThan(pane.y);
    for (const name of ['Row actions', 'Column actions']) {
      const trigger = tools(page).getByRole('button', { name, exact: true });
      const box = (await trigger.boundingBox())!;
      expect(box.y).toBeGreaterThanOrEqual(pane.y + 8);
      expect(box.y + box.height).toBeLessThanOrEqual(pane.y + pane.height - 8);
      expect(
        await trigger.evaluate((element) => {
          const rect = element.getBoundingClientRect();
          return element.contains(document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2));
        }),
      ).toBe(true);
    }
    const panel = await openActions(page, 'row');
    const box = (await panel.boundingBox())!;
    expect(box.y).toBeGreaterThanOrEqual(pane.y + 8);
    expect(box.y + box.height).toBeLessThanOrEqual(pane.y + pane.height - 8);
    await tools(page).getByRole('button', { name: 'Row actions', exact: true }).click();
    await expect(panel).toBeHidden();
    await openActions(page, 'row');
    await page.keyboard.press('Escape');
    await expect(panel).toBeHidden();
    await expect(tools(page).getByRole('button', { name: 'Row actions', exact: true })).toBeFocused();
    expect(await source(page)).toBe(text);
    await tools(page).getByRole('button', { name: 'Column actions', exact: true }).press('Enter');
    await expect(tools(page).getByRole('group', { name: 'Column actions', exact: true })).toBeVisible();
  });

  for (const hasTouch of [false, true]) {
    test.describe(`compact grips ${hasTouch ? 'touch' : 'desktop'}`, () => {
      test.use({ hasTouch, viewport: { width: hasTouch ? 390 : 1280, height: 844 } });
      test('cell editors have compact equal gutters and alignment uses accessible EUI icons', async ({ page }) => {
        const long = Array.from({ length: 12 }, (_, i) => `Wrapped evidence ${i + 1}`).join('<br>');
        const text = `| Name | Evidence |\n| --- | --- |\n| Alice | ${long} |\n| Bob | Next |\n`;
        await boot(page, text);
        await cell(page, 1, 0).dblclick();
        const editor = page.getByRole('textbox', { name: 'Edit table cell', exact: true });
        const spacing = () =>
          editor.evaluate((input) => {
            const cell = input.closest('td')!,
              rect = cell.getBoundingClientRect(),
              box = input.getBoundingClientRect();
            const style = getComputedStyle(cell),
              inner = getComputedStyle(input);
            return {
              padding: [style.paddingTop, style.paddingRight, style.paddingBottom, style.paddingLeft].map(parseFloat),
              inner: [inner.paddingTop, inner.paddingRight, inner.paddingBottom, inner.paddingLeft].map(parseFloat),
              gaps: [box.top - rect.top, rect.right - box.right, rect.bottom - box.bottom, box.left - rect.left],
            };
          });
        await expect.poll(async () => (await spacing()).padding).toEqual([4, 4, 4, 4]);
        await expect.poll(async () => (await spacing()).inner).toEqual([4, 4, 4, 4]);
        await expect
          .poll(async () => {
            const { gaps } = await spacing();
            return Math.max(...gaps) - Math.min(...gaps);
          })
          .toBeLessThanOrEqual(0.5);
        const gutters = (await spacing()).gaps;
        expect(gutters.every((gap) => gap >= 4 && gap <= 5)).toBe(true);
        // Native textarea resizing must still grow the row without uneven lower padding.
        const height = (await editor.boundingBox())!.height;
        await editor.evaluate((input, height) => {
          (input as HTMLElement).style.height = `${height + 120}px`;
        }, height);
        expect((await editor.boundingBox())!.height).toBeGreaterThanOrEqual(height + 119);
        await expect
          .poll(async () => {
            const { gaps } = await spacing();
            return Math.max(...gaps) - Math.min(...gaps);
          })
          .toBeLessThanOrEqual(0.5);
        await editor.press('Escape');
        await cell(page, 1, 0).click();
        const menu = await openActions(page, 'column');
        const group = menu.getByRole('group', { name: 'Column alignment', exact: true });
        await expect(group).toHaveCount(1);
        await expect(group.locator('legend')).toHaveClass('sr-only');
        for (const [value, path] of [
          ['left', 'M2 4V3h6v1H2Zm0 3V6h10v1H2Zm0 3V9h6v1H2Zm0 3v-1h10v1H2Z'],
          ['center', 'M5 4V3h6v1H5ZM3 7V6h10v1H3Zm2 3V9h6v1H5Zm-2 3v-1h10v1H3Z'],
          ['right', 'M8 4V3h6v1H8ZM4 7V6h10v1H4Zm4 3V9h6v1H8Zm-4 3v-1h10v1H4Z'],
        ]) {
          const radio = group.getByRole('radio', { name: `Align column ${value}`, exact: true });
          await expect(radio).toHaveAttribute('title', `Align column ${value}`);
          const label = radio.locator('..');
          expect(await label.evaluate((label) => label.textContent!.trim())).toBe('');
          await expect(label.locator('svg')).toHaveAttribute('aria-hidden', 'true');
          await expect(label.locator('svg path')).toHaveAttribute('d', path);
          const box = (await radio.boundingBox())!;
          expect(box.width).toBeGreaterThanOrEqual(hasTouch ? 44 : 28);
          expect(box.height).toBeGreaterThanOrEqual(hasTouch ? 44 : 28);
        }
        await group.getByRole('radio', { name: 'Align column center', exact: true }).check();
        await expect(cell(page, 1, 0)).toHaveCSS('text-align', 'center');
        expect(await source(page)).toBe(text.replace('| --- | --- |', '| :---: | --- |'));
      });
      test('button-free cell editing and three-state alignment retain phone interaction', async ({ page }) => {
        await boot(page, sized([0.5, 0.5]));
        await cell(page, 2, 1).dblclick();
        const editor = page.getByRole('textbox', { name: 'Edit table cell', exact: true });
        await expect(page.locator('.su-table-cell-editor button')).toHaveCount(0);
        await editor.fill('Phone edit');
        const paragraph = page.locator('#previewArticle p').filter({ hasText: 'Unchanged paragraph.' });
        if (hasTouch) await paragraph.tap();
        else await paragraph.click();
        await expect(cell(page, 2, 1)).toHaveText('Phone edit');
        await cell(page, 2, 1).click();
        const menu = await openActions(page, 'column');
        await menu.getByRole('radio', { name: 'Align column center', exact: true }).check();
        await expect(cell(page, 2, 1)).toHaveCSS('text-align', 'center');
        expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
          hasTouch ? 390 : 1280,
        );
      });
      test('selected cell formatting stacks, toggles, and survives undo and export in quoted tables', async ({
        page,
      }) => {
        const review =
          '\n<!-- su-comments\n[{"id":"thread","quote":"alpha","comments":[{"id":"reply","body":"Keep this review"}]}]\n-->\n';
        const text =
          '# Tables\n\n| [Header](https://example.test/alpha/words) | Value |\n| --- | --- |\n| alpha words | alpha words |\n\n> <!-- su-table {"v":1,"cols":[0.5,0.5]} -->\n> | Name | Value |\n> | --- | --- |\n> | alpha words | alpha words |\n> | alpha words | alpha words |\n';
        await boot(page, text + review);
        await page.getByRole('button', { name: 'Source', exact: true }).click();
        await expect(page.getByRole('textbox', { name: 'Markdown source', exact: true })).toBeVisible();
        await page.getByRole('button', { name: 'Preview', exact: true }).click();
        const selected = table(page, 1).locator('tbody tr').nth(1).locator('td').nth(1);
        await expect(selected).toHaveText('alpha words');
        await selected.evaluate((element) => {
          const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
          const node = walker.nextNode()!;
          const range = document.createRange();
          range.setStart(node, 0);
          range.setEnd(node, 5);
          getSelection()!.removeAllRanges();
          getSelection()!.addRange(range);
          element.dispatchEvent(new Event('contextmenu', { bubbles: true }));
        });
        const formatted = (value: string) =>
          text.replace(/> \| alpha words \| alpha words \|\n$/, `> | alpha words | ${value} words |\n`);
        const toggle = async (name: string, value: string) => {
          const button = page
            .getByRole('toolbar', { name: 'Format selection' })
            .getByRole('button', { name, exact: true });
          if (hasTouch) await button.tap();
          else await button.click();
          await expect.poll(() => source(page)).toBe(formatted(value));
          await expect.poll(() => page.evaluate(() => getSelection()!.toString())).toBe('alpha');
          await expect(selected).toHaveText('alpha words');
        };
        await toggle('Bold', '**alpha**');
        await toggle('Italic', '***alpha***');
        await toggle('Italic', '**alpha**');
        await toggle('Bold', 'alpha');
        await toggle('Highlight', '==alpha==');
        await toggle('Strikethrough', '~~==alpha==~~');
        await toggle('Highlight', '~~alpha~~');
        await toggle('Strikethrough', 'alpha');
        await toggle('Inline code', '`alpha`');
        await page.keyboard.press('Escape');
        await page.locator('#previewScroll').focus();
        await page.keyboard.press('ControlOrMeta+z');
        await expect.poll(() => source(page)).toBe(text);
        await page.keyboard.press('ControlOrMeta+Shift+z');
        await expect.poll(() => source(page)).toBe(formatted('`alpha`'));
        await expect(page.locator('#previewArticle thead a')).toHaveAttribute(
          'href',
          'https://example.test/alpha/words',
        );
        await exportDialog(page);
        const markdown = await download(page, 'Markdown');
        expect(markdown.split('<!-- su-comments')[0].trimEnd()).toBe(formatted('`alpha`').trimEnd());
        expect(markdown).toContain('Keep this review');
        const html = await download(page, 'HTML');
        expect(html).toContain('<code>alpha</code> words');
        expect(html).not.toMatch(/data-table-start|su-table-tools|su-table-cell-editor/);
      });
      test('use selected cell borders as their baseline and retain accessible targets', async ({ page }) => {
        await boot(page, sized());
        if (hasTouch) await cell(page, 2, 1).tap();
        else await cell(page, 2, 1).click();
        await expect(tools(page)).toBeVisible({ timeout: 3000 });
        await page.mouse.move(0, 0);
        const geometry = await tools(page).evaluate((root) => {
          const selected = document.querySelector('.su-table-selected')!;
          const cell = selected.getBoundingClientRect();
          const grip = (axis: string) => {
            const button = root.querySelector(`.su-table-${axis}-grip`)!;
            return {
              target: button.getBoundingClientRect().toJSON(),
              face: button.querySelector('.su-table-grip-face')!.getBoundingClientRect().toJSON(),
            };
          };
          return {
            cell: cell.toJSON(),
            row: grip('row'),
            column: grip('column'),
            cellCenterAvailable:
              document.elementFromPoint(cell.x + cell.width / 2, cell.y + cell.height / 2)?.closest('th,td') ===
              selected,
          };
        });
        expect(geometry.cellCenterAvailable).toBe(true);
        expect(geometry.row.face.width).toBeLessThanOrEqual(8);
        expect(geometry.column.face.height).toBeLessThanOrEqual(8);
        expect(Math.abs(geometry.row.face.x + geometry.row.face.width / 2 - geometry.cell.x)).toBeLessThanOrEqual(0.5);
        expect(
          Math.abs(geometry.row.face.y + geometry.row.face.height / 2 - geometry.cell.y - geometry.cell.height / 2),
        ).toBeLessThanOrEqual(0.5);
        expect(
          Math.abs(geometry.column.face.y + geometry.column.face.height / 2 - geometry.cell.y),
        ).toBeLessThanOrEqual(0.5);
        expect(
          Math.abs(geometry.column.face.x + geometry.column.face.width / 2 - geometry.cell.x - geometry.cell.width / 2),
        ).toBeLessThanOrEqual(0.5);
        for (const grip of [geometry.row, geometry.column]) {
          expect(grip.target.width).toBeGreaterThanOrEqual(24);
          expect(grip.target.height).toBeGreaterThanOrEqual(24);
        }
        if (hasTouch) {
          expect(geometry.row.target.height).toBeGreaterThanOrEqual(44);
          expect(geometry.column.target.width).toBeGreaterThanOrEqual(44);
        }
        await tools(page).getByRole('button', { name: 'Row actions', exact: true }).focus();
        const face = tools(page).locator('.su-table-row-grip .su-table-grip-face');
        await expect(face.locator('[data-drag]')).toBeVisible();
        const box = (await face.boundingBox())!;
        expect(Math.abs(box.x + box.width / 2 - geometry.cell.x)).toBeLessThanOrEqual(0.5);
        await tools(page).getByRole('button', { name: 'Row actions', exact: true }).press('Enter');
        await expect(tools(page).getByRole('group', { name: 'Row actions', exact: true })).toBeVisible();
      });

      test('tall cells keep centered muted grips on hover and focus, clamping only at viewport edges', async ({
        page,
      }) => {
        const lines = Array.from({ length: 18 }, (_, i) => `Review line ${i + 1}`).join('<br>');
        const text = `# Tables\n\n<!-- su-table {"v":1,"cols":[0.5,0.5]} -->\n| Priority | Strategy |\n| --- | --- |\n| P1 | ${lines} |\n| P2 | Next |\n`;
        await boot(page, text);
        const selected = cell(page, 1, 0);
        if (hasTouch) await selected.tap();
        else await selected.click();
        await expect(tools(page)).toBeVisible();

        const centerErrors = () =>
          tools(page).evaluate((root) => {
            const selected = document.querySelector('.su-table-selected')!;
            const cell = selected.getBoundingClientRect();
            const pane = document.querySelector('#previewPane')!.getBoundingClientRect();
            const table = selected.closest('table')!.getBoundingClientRect();
            const wrap = selected.closest('.su-table-edit-scroll')!.getBoundingClientRect();
            const top = pane.top + 8;
            const bottom = Math.min(innerHeight, pane.bottom) - 8;
            const left = pane.left + 8;
            const right = Math.min(innerWidth, pane.right) - 8;
            const row = root.querySelector('.su-table-row-grip')!.getBoundingClientRect();
            const column = root.querySelector('.su-table-column-grip')!.getBoundingClientRect();
            const start = Math.max(left, Math.max(table.left, wrap.left) - row.width / 2);
            const end = Math.min(right, table.right, wrap.right);
            const clamp = (center: number, size: number, low: number, high: number) =>
              Math.max(low + size / 2, Math.min(center, high - size / 2));
            return [
              Math.abs(row.x + row.width / 2 - clamp(Math.max(cell.left, wrap.left, left), row.width, start, end)),
              Math.abs(row.y + row.height / 2 - clamp(cell.y + cell.height / 2, row.height, top, bottom)),
              Math.abs(column.x + column.width / 2 - clamp(cell.x + cell.width / 2, column.width, start, end)),
              Math.abs(column.y + column.height / 2 - clamp(cell.y, column.height, top, bottom)),
            ];
          });

        for (const theme of ['light', 'dark']) {
          await page.evaluate((theme) => {
            document.documentElement.dataset.theme = theme;
          }, theme);
          for (const name of ['Row actions', 'Column actions']) {
            const trigger = tools(page).getByRole('button', { name, exact: true });
            for (const state of hasTouch ? ['focus'] : ['hover', 'focus']) {
              if (state === 'hover') await trigger.hover();
              else await trigger.focus();
              await expect.poll(async () => Math.max(...(await centerErrors()))).toBeLessThanOrEqual(0.5);
              const colors = await trigger.evaluate((button) => {
                const face = button.querySelector('.su-table-grip-face')!;
                const reference = document.createElement('span');
                reference.style.background = 'var(--eui-border-plain)';
                reference.style.color = 'var(--text)';
                button.append(reference);
                const colors = {
                  actual: getComputedStyle(face).backgroundColor,
                  expected: getComputedStyle(reference).backgroundColor,
                  text: getComputedStyle(face).color,
                  expectedText: getComputedStyle(reference).color,
                };
                reference.remove();
                return colors;
              });
              expect(colors.actual).toBe(colors.expected);
              expect(colors.text).toBe(colors.expectedText);
            }
            await trigger.blur();
            await page.mouse.move(0, 0);
          }
        }
        await page.setViewportSize({ width: hasTouch ? 390 : 1280, height: 420 });
        await expect.poll(async () => Math.max(...(await centerErrors()))).toBeLessThanOrEqual(0.5);
        await page.locator('#previewScroll').evaluate((pane) => {
          pane.scrollTop = 180;
        });
        await expect.poll(async () => Math.max(...(await centerErrors()))).toBeLessThanOrEqual(0.5);
        await table(page).evaluate((table) => {
          table.style.width = '1200px';
        });
        // A width-only reflow may not resize the pane/article or emit a scroll event.
        await expect.poll(async () => Math.max(...(await centerErrors()))).toBeLessThanOrEqual(0.5);
        await page.locator('.su-table-edit-scroll').evaluate((wrap) => {
          wrap.scrollLeft = 160;
        });
        await expect.poll(async () => Math.max(...(await centerErrors()))).toBeLessThanOrEqual(0.5);
        await page.setViewportSize({ width: hasTouch ? 390 : 1280, height: 844 });
        await table(page).evaluate((table) => {
          table.style.width = '100%';
        });
        await page.locator('#previewScroll').evaluate((pane) => {
          pane.scrollTop = 0;
        });
        await expect.poll(async () => Math.max(...(await centerErrors()))).toBeLessThanOrEqual(0.5);
        // Content reflow (including late fonts) must reposition grips without a window resize.
        await table(page).evaluate((table) => {
          table.style.fontSize = '18px';
        });
        await expect.poll(async () => Math.max(...(await centerErrors()))).toBeLessThanOrEqual(0.5);
        expect(await source(page)).toBe(text);
      });
    });
  }

  test('small and expanded grip faces center on the painted border at 100% and 180% scale', async ({
    playwright,
    browserName,
  }, testInfo) => {
    const text = `<!-- su-table {"v":1,"cols":[0.1,0.17,0.25,0.24,0.24]} -->
| Priority | Strategy and inspiration | Concrete testcase | Evidence | Checks |
| --- | --- | --- | --- | --- |
| P1 | **Wrapped review details and context.** Keep the visible border baseline stable at fractional zoom. | A review case with several wrapped lines. | Preserve the original document. | Compare the result. |
| P2 | Next strategy. | Another case. | Original source. | Review. |
`;
    for (const scale of [1.8, 1]) {
      // Firefox ignores context deviceScaleFactor. Native scaling with an unconstrained
      // viewport exercises fractional device-pixel rounding in the real renderer.
      const browser = await playwright[browserName].launch(
        browserName === 'firefox'
          ? {
              firefoxUserPrefs: { 'layout.css.devPixelsPerPx': String(scale) },
              env: { ...process.env, MOZ_HEADLESS_WIDTH: '1800', MOZ_HEADLESS_HEIGHT: '1400' },
            }
          : {},
      );
      try {
        const context = await browser.newContext({
          baseURL: MARS_BASE_URL,
          reducedMotion: 'no-preference',
          viewport:
            browserName === 'firefox' ? null : { width: Math.round(1800 / scale), height: Math.round(1400 / scale) },
          deviceScaleFactor: browserName === 'firefox' ? undefined : scale,
        });
        const page = await context.newPage();
        await boot(page, text);
        await page.evaluate(() => document.fonts.ready);
        await cell(page, 1, 1).click();
        await expect(tools(page)).toBeVisible();
        for (const state of ['small', 'row expanded', 'column expanded']) {
          await page.mouse.move(0, 0);
          if (state === 'row expanded')
            await tools(page).getByRole('button', { name: 'Row actions', exact: true }).hover();
          if (state === 'column expanded')
            await tools(page).getByRole('button', { name: 'Column actions', exact: true }).hover();
          const geometry = await tools(page).evaluate((root) => {
            const selected = document.querySelector('.su-table-selected')!;
            const cell = selected.getBoundingClientRect().toJSON();
            const face = (axis: string) =>
              root.querySelector(`.su-table-${axis}-grip .su-table-grip-face`)!.getBoundingClientRect().toJSON();
            const reference = document.createElement('span');
            reference.style.color = 'var(--primary)';
            root.append(reference);
            const color = getComputedStyle(reference).color.match(/\d+/g)!.map(Number);
            reference.remove();
            return { cell, row: face('row'), column: face('column'), color, width: innerWidth, dpr: devicePixelRatio };
          });
          const screenshot = await page.screenshot({ animations: 'disabled' });
          const path = testInfo.outputPath(`painted-border-${scale}-${state}.png`);
          await writeFile(path, screenshot);
          await testInfo.attach(`painted-border-${scale}-${state}`, { path, contentType: 'image/png' });
          const png = PNG.sync.read(screenshot);
          const pixelsPerCss = png.width / geometry.width;
          if (scale > 1) {
            expect(geometry.dpr).toBeGreaterThan(1.5);
            expect(pixelsPerCss).toBeGreaterThan(1.5);
          }
          const borderCenter = (axis: 'row' | 'column') => {
            const vertical = axis === 'row';
            const edge = vertical ? geometry.cell.x : geometry.cell.y;
            const cross = vertical
              ? geometry.cell.y + geometry.cell.height / 4
              : geometry.cell.x + geometry.cell.width / 4;
            const fixed = Math.floor(cross * pixelsPerCss);
            const hits: number[] = [];
            for (let at = Math.floor((edge - 6) * pixelsPerCss); at <= Math.ceil((edge + 6) * pixelsPerCss); at++) {
              const x = vertical ? at : fixed,
                y = vertical ? fixed : at;
              if (x < 0 || y < 0 || x >= png.width || y >= png.height) continue;
              const offset = (y * png.width + x) * 4;
              if (
                geometry.color.slice(0, 3).every((value, channel) => Math.abs(png.data[offset + channel] - value) <= 6)
              )
                hits.push(at + 0.5);
            }
            expect(hits.length, `${axis} border must have actual painted pixels`).toBeGreaterThan(0);
            return hits.reduce((sum, value) => sum + value, 0) / hits.length;
          };
          const rowCenter = (geometry.row.x + geometry.row.width / 2) * pixelsPerCss;
          const columnCenter = (geometry.column.y + geometry.column.height / 2) * pixelsPerCss;
          expect(
            Math.abs(rowCenter - borderCenter('row')),
            `${state}: row grip on painted left border`,
          ).toBeLessThanOrEqual(1);
          expect(
            Math.abs(columnCenter - borderCenter('column')),
            `${state}: column grip on painted top border`,
          ).toBeLessThanOrEqual(1);
        }
        await cell(page, 1, 1).dblclick();
        const editor = page.getByRole('textbox', { name: 'Edit table cell', exact: true });
        await expect
          .poll(() =>
            editor.evaluate((input) => {
              const cell = input.closest('td')!.getBoundingClientRect(),
                box = input.getBoundingClientRect();
              const gaps = [box.top - cell.top, cell.right - box.right, cell.bottom - box.bottom, box.left - cell.left];
              return (Math.max(...gaps) - Math.min(...gaps)) * devicePixelRatio;
            }),
          )
          .toBeLessThanOrEqual(1);
        await expect(editor).toHaveCSS('padding', '4px');
        const path = testInfo.outputPath(`cell-editor-${scale}.png`);
        await page.screenshot({ path, animations: 'disabled' });
        await testInfo.attach(`cell-editor-${scale}`, { path, contentType: 'image/png' });
      } finally {
        await browser.close();
      }
    }
  });

  for (const width of [320, 390])
    for (const theme of ['light', 'dark']) {
      test(`table controls fit ${width}px in ${theme} and wide tables scroll without page overflow`, async ({
        page,
      }) => {
        await page.setViewportSize({ width, height: 844 });
        const header = `| ${Array.from({ length: 14 }, (_, i) => `Column ${i + 1}`).join(' | ')} |`;
        const divider = `| ${Array(14).fill('---').join(' | ')} |`;
        const row = `| ${Array.from({ length: 14 }, (_, i) => `Cell ${i + 1}`).join(' | ')} |`;
        await boot(page, `# Tables\n\n${header}\n${divider}\n${row}\n`);
        await page.evaluate((theme) => {
          document.documentElement.dataset.theme = theme;
        }, theme);
        await cell(page, 1, 13).click();
        await expect(tools(page)).toBeInViewport();
        const rect = (await tools(page).boundingBox())!;
        expect(rect.x).toBeGreaterThanOrEqual(0);
        expect(rect.x + rect.width).toBeLessThanOrEqual(width);
        const scroll = page.locator('#previewArticle .su-table-edit-scroll');
        expect(await scroll.evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(true);
        expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
      });
    }

  test('real touch resizing and row reordering keep phone controls reachable', async ({
    page,
    browser,
    browserName,
  }) => {
    test.skip(
      browserName !== 'chromium',
      'Real touch movement uses Chromium CDP; mouse and keyboard cases cover all engines.',
    );
    const context = await browser.newContext({
      baseURL: MARS_BASE_URL,
      hasTouch: true,
      isMobile: true,
      viewport: { width: 390, height: 844 },
    });
    try {
      const phone = await context.newPage();
      await boot(phone, sized());
      const cdp = await context.newCDPSession(phone);
      const handle = (await table(phone)
        .getByRole('separator', { name: 'Resize column 1', exact: true })
        .boundingBox())!;
      const x = handle.x + handle.width / 2,
        y = handle.y + handle.height / 2;
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y, id: 1 }] });
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: x + 35, y, id: 1 }] });
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
      await expect.poll(() => ratio(phone)).toBeGreaterThan(0.3);
      await cell(phone, 1, 0).tap();
      await expect(tools(phone)).toBeInViewport();
      const grip = (await tools(phone).getByRole('button', { name: 'Row actions', exact: true }).boundingBox())!;
      const last = (await cell(phone, 3, 0).boundingBox())!;
      await cdp.send('Input.dispatchTouchEvent', {
        type: 'touchStart',
        touchPoints: [{ x: grip.x + 18, y: grip.y + 18, id: 2 }],
      });
      await cdp.send('Input.dispatchTouchEvent', {
        type: 'touchMove',
        touchPoints: [{ x: last.x + 20, y: last.y + last.height + 10, id: 2 }],
      });
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
      await expect(cell(phone, 3, 0)).toHaveText('Alice');
      await cell(phone, 0, 0).tap();
      const columnGrip = (await tools(phone)
        .getByRole('button', { name: 'Column actions', exact: true })
        .boundingBox())!;
      const right = (await cell(phone, 0, 1).boundingBox())!;
      await cdp.send('Input.dispatchTouchEvent', {
        type: 'touchStart',
        touchPoints: [{ x: columnGrip.x + 18, y: columnGrip.y + 18, id: 3 }],
      });
      await cdp.send('Input.dispatchTouchEvent', {
        type: 'touchMove',
        touchPoints: [{ x: right.x + right.width + 8, y: right.y + right.height / 2, id: 3 }],
      });
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
      await expect(cell(phone, 0, 1)).toHaveText('Name');
      expect(await phone.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
    } finally {
      await context.close();
    }
    await page.goto('/');
  });
});
