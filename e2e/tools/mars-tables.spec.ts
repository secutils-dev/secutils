import { readFile, writeFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';

import type { Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

const MARS_BASE_URL = process.env.MARS_BASE_URL ?? 'https://mars.secutils.dev';
test.use({ baseURL: MARS_BASE_URL });
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
const cell = (page: Page, row: number, column: number) =>
  table(page).locator('tr').nth(row).locator('th,td').nth(column);
const source = (page: Page) =>
  page.evaluate(() => (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText());
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
    await tools(page).getByRole('button', { name: 'Move row down', exact: true }).click();
    await expect(cell(page, 1, 0)).toHaveText('Bob');
    await expect(cell(page, 2, 0)).toHaveText('Alice');
    await tools(page).getByRole('button', { name: 'Insert row below', exact: true }).click();
    await expect(table(page).locator('tbody tr')).toHaveCount(4);
    await tools(page).getByRole('button', { name: 'Remove row', exact: true }).click();
    await expect(table(page).locator('tbody tr')).toHaveCount(3);
    await cell(page, 0, 0).click();
    await expect(tools(page).getByRole('button', { name: 'Remove row', exact: true })).toBeDisabled();
    await expect(tools(page).getByRole('button', { name: 'Insert row above', exact: true })).toBeDisabled();
    await tools(page).getByRole('button', { name: 'Move column right', exact: true }).click();
    await expect(cell(page, 0, 0)).toHaveText('Status');
    await expect(cell(page, 1, 0)).toHaveCSS('text-align', 'right');
    expect(await ratio(page)).toBeCloseTo(0.75, 2);
    await tools(page).getByRole('button', { name: 'Insert column after', exact: true }).click();
    await expect(table(page).locator('thead th')).toHaveCount(3);
    await tools(page).getByRole('button', { name: 'Remove column', exact: true }).click();
    await expect(table(page).locator('thead th')).toHaveCount(2);
    await tools(page).getByRole('button', { name: 'Remove column', exact: true }).click();
    await expect(table(page).locator('thead th')).toHaveCount(1);
    await expect(tools(page).getByRole('button', { name: 'Remove column', exact: true })).toBeDisabled();
    expect(await source(page)).toContain('Unchanged paragraph.');
  });

  test('mouse grips reorder rows and columns and keyboard controls resize and move', async ({ page }) => {
    await boot(page, sized());
    await cell(page, 1, 0).click();
    const rowGrip = (await tools(page)
      .getByRole('button', { name: 'Drag row to reorder', exact: true })
      .boundingBox())!;
    const last = (await cell(page, 3, 0).boundingBox())!;
    await page.mouse.move(rowGrip.x + rowGrip.width / 2, rowGrip.y + rowGrip.height / 2);
    await page.mouse.down();
    await page.mouse.move(last.x + last.width / 2, last.y + last.height + 8, { steps: 8 });
    await page.mouse.up();
    await expect(cell(page, 3, 0)).toHaveText('Alice');
    await cell(page, 0, 0).click();
    const columnGrip = (await tools(page)
      .getByRole('button', { name: 'Drag column to reorder', exact: true })
      .boundingBox())!;
    const right = (await cell(page, 0, 1).boundingBox())!;
    await page.mouse.move(columnGrip.x + columnGrip.width / 2, columnGrip.y + columnGrip.height / 2);
    await page.mouse.down();
    await page.mouse.move(right.x + right.width + 8, right.y + right.height / 2, { steps: 8 });
    await page.mouse.up();
    await expect(cell(page, 0, 1)).toHaveText('Name');
    await tools(page).getByRole('button', { name: 'Drag column to reorder', exact: true }).press('ArrowLeft');
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
          const add = EventTarget.prototype.addEventListener;
          // Browsers can run microtasks between listeners. Track the menu handler itself,
          // not a capture listener that finishes before the handler runs.
          EventTarget.prototype.addEventListener = function (type, listener, options) {
            if (
              type !== 'click' ||
              !(this instanceof HTMLButtonElement) ||
              this.getAttribute('role') !== 'menuitem' ||
              typeof listener !== 'function'
            ) {
              return add.call(this, type, listener, options);
            }
            return add.call(
              this,
              type,
              function (event) {
                inClick = true;
                try {
                  listener.call(this, event);
                } finally {
                  inClick = false;
                }
              },
              options,
            );
          };
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
        await page.getByRole('button', { name: 'Share', exact: true }).click();
        await page.getByRole('menuitem', { name: mode === 'full' ? /^Copy full link/ : /^Copy short link/ }).click();
        await expect.poll(() => page.locator('html').getAttribute('data-table-link')).toContain('#');
        const link = (await page.locator('html').getAttribute('data-table-link'))!;
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
      await page.getByRole('button', { name: 'Share', exact: true }).click();
      await page.getByRole('menuitem', { name: /^Copy full link/ }).click();
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

  test('header HTML that changes the rendered grid cannot expose an unsavable resize', async ({ page }) => {
    const text = DOC.replace('Name', '<th>Name</th>');
    await boot(page, text);
    await expect(table(page).getByRole('separator')).toHaveCount(0);
    await cell(page, 0, 0).click();
    await expect(tools(page)).toBeHidden();
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
    await tools(page).getByRole('button', { name: 'Move column right', exact: true }).click();
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
    await tools(page).getByRole('button', { name: 'Move row down', exact: true }).click();
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
    await tools(page).getByRole('button', { name: 'Move row down', exact: true }).click();
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
    const grip = (await tools(page).getByRole('button', { name: 'Drag row to reorder', exact: true }).boundingBox())!;
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
      const grip = (await tools(phone)
        .getByRole('button', { name: 'Drag row to reorder', exact: true })
        .boundingBox())!;
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
        .getByRole('button', { name: 'Drag column to reorder', exact: true })
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
