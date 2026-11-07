import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

import type { Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

const MARS_BASE_URL = process.env.MARS_BASE_URL ?? 'https://mars.secutils.dev';
test.use({ baseURL: MARS_BASE_URL });
const BODY =
  '# Report\r\n\r\n## One\r\n\r\nHello **reader**.\r\n\r\n## Two\r\n\r\n```js\r\nconst answer = 42;\r\n```\r\n\r\n## Three\r\n\r\nLiteral `</script>` text.\r\n';
const REVIEW = JSON.stringify(
  [{ id: 'thread', quote: 'reader', comments: [{ body: 'PRIVATE REVIEW' }] }],
  null,
  2,
).replaceAll('\n', '\r\n');
const TEXT = `---\r\ntitle: Internal title\r\n---\r\n${BODY}\r\n<!-- su-comments\r\n${REVIEW}\r\n-->\r\n`;
const dlg = (page: Page) => page.getByRole('dialog', { name: /^Export / });
const htmlFrame = (page: Page) => page.frameLocator('#exportHtmlPreview iframe');
const pdfFrame = (page: Page) => page.frameLocator('#exportPdfPreview iframe');

async function openDoc(page: Page, text = TEXT, name = 'Report.md') {
  await page.goto('/');
  await expect(page.locator('#emptyState')).toBeVisible();
  await page.locator('#fileInput').setInputFiles({ name, mimeType: 'text/markdown', buffer: Buffer.from(text) });
  await expect(page.locator('#exportBtn')).toBeEnabled({ timeout: 15000 });
  await expect(page.locator('#saveState')).toHaveText('Saved');
}
async function openExport(page: Page) {
  await page.getByRole('button', { name: 'Export document', exact: true }).click();
  await expect(dlg(page).getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled({ timeout: 20000 });
}
async function htmlDownload(page: Page) {
  const pending = page.waitForEvent('download');
  await dlg(page).getByRole('button', { name: 'Download HTML', exact: true }).click();
  const download = await pending;
  return { name: download.suggestedFilename(), text: await readFile(await download.path(), 'utf8') };
}
async function readyPdf(page: Page) {
  await dlg(page).getByRole('tab', { name: 'PDF', exact: true }).click();
  await expect(dlg(page).getByRole('button', { name: 'Print / Save PDF', exact: true })).toBeEnabled({
    timeout: 60000,
  });
}

test.describe('export workspace', () => {
  test.setTimeout(90000);
  test('standalone reader lazily prepares PDF with the committed paper settings', async ({ page }) => {
    await page.addInitScript(() => {
      try {
        localStorage.setItem(
          'mars.exportSettings',
          JSON.stringify({
            v: 1,
            html: { readingWidth: 800, toc: true, find: true, pdf: true, source: false },
            pdf: { paper: 'Letter', orientation: 'landscape' },
          }),
        );
      } catch {
        // Opaque preview frames cannot access browser storage.
      }
      window.print = () => {
        document.documentElement.dataset.printRequested = 'yes';
      };
    });
    const requests: string[] = [];
    page.on('request', (request) => {
      if (request.url().includes('pagedjs')) requests.push(request.url());
    });
    await openDoc(page);
    await openExport(page);
    expect(requests).toHaveLength(0);
    await htmlFrame(page).getByRole('button', { name: 'Prepare PDF', exact: true }).click();
    const readerDialog = htmlFrame(page).getByRole('dialog', { name: 'Print / Save PDF', exact: true });
    await expect(readerDialog.getByRole('button', { name: 'Print / Save PDF', exact: true })).toBeEnabled({
      timeout: 60000,
    });
    expect(requests).toHaveLength(1);
    const pages = readerDialog.frameLocator('iframe');
    const dimensions = await pages
      .locator('.pagedjs_page')
      .first()
      .evaluate((el) => ({ width: (el as HTMLElement).offsetWidth, height: (el as HTMLElement).offsetHeight }));
    expect(dimensions.width).toBeCloseTo((279.4 * 96) / 25.4, 0);
    expect(dimensions.height).toBeCloseTo((215.9 * 96) / 25.4, 0);
    await readerDialog.getByRole('button', { name: 'Print / Save PDF', exact: true }).click();
    await expect(pages.locator('html')).toHaveAttribute('data-print-requested', 'yes');
    await readerDialog.getByRole('button', { name: 'Close', exact: true }).click();
    await expect(readerDialog).toHaveCount(0);
  });

  test('copies prepared HTML, commits block edits, and stays visible after document fullscreen', async ({ page }) => {
    await page.addInitScript(() => {
      Object.defineProperty(navigator, 'clipboard', {
        value: {
          write: async (items: ClipboardItem[]) => {
            document.documentElement.dataset.copiedHtml = await (await items[0].getType('text/plain')).text();
          },
          writeText: async (text: string) => {
            document.documentElement.dataset.copiedHtml = text;
          },
        },
      });
    });
    await openDoc(page, '# Edit\n\nOriginal words.', 'Edit.md');
    await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
    await page.locator('#previewArticle p').dblclick();
    const blockEditor = page.locator('.su-block-editor').getByRole('textbox');
    await blockEditor.press('ControlOrMeta+a');
    await blockEditor.pressSequentially('Edited words.');
    await openExport(page);
    await expect(htmlFrame(page).locator('article')).toContainText('Edited words.');
    await dlg(page).getByRole('button', { name: 'Copy HTML', exact: true }).click();
    await expect(page.locator('#toast')).toContainText('Copied to clipboard');
    expect(await page.locator('html').getAttribute('data-copied-html')).toBe(
      await page.locator('#exportHtmlPreview iframe').getAttribute('srcdoc'),
    );
    expect(await page.evaluate(() => localStorage.getItem('mars.exportSettings'))).not.toBeNull();
    await page.keyboard.press('Escape');
    await expect(page.locator('#previewArticle')).toContainText('Edited words.');
    await page.getByRole('button', { name: 'Toggle fullscreen', exact: true }).click();
    await expect.poll(() => page.evaluate(() => !!document.fullscreenElement)).toBe(true);
    await page.keyboard.press('ControlOrMeta+p');
    await expect(dlg(page)).toBeVisible();
    expect(await page.evaluate(() => document.fullscreenElement)).toBeNull();
  });

  test('keeps an immutable capture after a document reload and refreshes it explicitly', async ({ page }) => {
    await openDoc(page, '# Before\n\nBefore reload.', 'Before.md');
    await openExport(page);
    await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
    await page.evaluate(() =>
      (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(
        '# After\n\nAfter reload.',
      ),
    );
    await expect(dlg(page).getByRole('button', { name: 'Refresh preview', exact: true })).toBeVisible();
    await expect(htmlFrame(page).locator('article')).toContainText('Before reload.');
    await dlg(page).getByRole('button', { name: 'Refresh preview', exact: true }).click();
    await expect(dlg(page).getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled();
    await expect(htmlFrame(page).locator('article')).toContainText('After reload.');
    await expect(dlg(page).getByRole('button', { name: 'Refresh preview', exact: true })).toBeHidden();
  });

  test('retains complete Mermaid SVGs, interactive maps, light pages, and review isolation', async ({ page }) => {
    const text =
      '# Rich output\n\n```mermaid\nflowchart LR\nA[Start] --> B[Complete diagram]\n```\n\n```geojson\n{"type":"LineString","coordinates":[[0,0],[2,2]]}\n```\n\n```stl\nsolid triangle\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid triangle\n```\n\n$$E = mc^2$$\n';
    await openDoc(page, text, 'Rich.md');
    const diagram = page.locator('#previewArticle .su-mermaid');
    await expect(diagram.locator('.su-mermaid-stage > svg')).toBeVisible({ timeout: 20000 });
    await diagram.getByRole('button', { name: 'Zoom in', exact: true }).click();
    await openExport(page);
    await expect(htmlFrame(page).locator('.su-mermaid svg')).toBeVisible();
    await expect(htmlFrame(page).locator('.su-mermaid-viewport')).toHaveCount(0);
    await expect(htmlFrame(page).locator('.katex')).toBeVisible();
    await expect(htmlFrame(page).locator('svg.su-diagram-view path')).toHaveCount(1);
    await htmlFrame(page)
      .locator('figure[data-kind="geojson"]')
      .getByRole('button', { name: 'Zoom in', exact: true })
      .click();
    await readyPdf(page);
    await expect(pdfFrame(page).locator('.su-mermaid svg')).toBeVisible();
    await expect(pdfFrame(page).locator('svg.su-diagram-view path')).toHaveCount(1);
    await expect(pdfFrame(page).locator('.su-diagram-controls')).toHaveCount(0);
    await expect(pdfFrame(page).locator('html')).toHaveAttribute('data-theme', 'light');
    await expect(pdfFrame(page).locator('.katex')).toBeVisible();
    await expect(pdfFrame(page).locator('figure[data-kind="stl"] polygon')).toHaveCount(1);
    await dlg(page).getByRole('tab', { name: 'HTML', exact: true }).click();
    await expect(dlg(page).getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled();
    const output = await htmlDownload(page);
    const path = test.info().outputPath('rich-reader.html');
    await writeFile(path, output.text);
    await page.context().setOffline(true);
    await page.goto(pathToFileURL(path).href);
    await expect(page.locator('.su-mermaid svg')).toBeVisible();
    await expect(page.locator('figure[data-kind="geojson"] svg.su-diagram-view path')).toHaveCount(1);
    await page.locator('figure[data-kind="stl"]').getByRole('button', { name: 'Rotate right', exact: true }).click();
    await expect(page.locator('figure[data-kind="stl"] polygon')).toHaveCount(1);
  });

  test('uses one toolbar control, isolated exact HTML, and lossless Markdown download', async ({ page }) => {
    const pagedRequests: string[] = [];
    page.on('request', (request) => {
      if (request.url().includes('pagedjs')) pagedRequests.push(request.url());
    });
    await openDoc(page);
    await openExport(page);
    await expect(page.locator('.toolbar [role=tab]')).toHaveCount(0);
    await expect(dlg(page).getByRole('heading')).toHaveText('Export “Internal title”');
    await expect(dlg(page).locator('#exportDocumentName, details')).toHaveCount(0);
    await expect(dlg(page).locator('#exportTocHint')).toHaveCount(0);
    await expect(dlg(page).getByRole('button', { name: 'Reset settings', exact: true })).toHaveCount(0);
    await expect(dlg(page).getByText('Shown when the document has useful headings.', { exact: true })).toHaveCount(0);
    await expect(
      dlg(page).getByText('Uses the paper size and orientation from the PDF tab.', { exact: true }),
    ).toHaveCount(0);
    for (const label of ['Table of contents', 'In-page find', 'PDF action in exported HTML', 'Embed Markdown source']) {
      await expect(dlg(page).getByLabel(label, { exact: true })).toBeVisible();
    }
    await expect(htmlFrame(page).locator('article')).toContainText('Hello reader.');
    expect(await page.locator('#exportHtmlPreview iframe').getAttribute('sandbox')).not.toContain('allow-same-origin');
    const isolation = await htmlFrame(page)
      .locator('body')
      .evaluate(async () => {
        let parentBlocked = false,
          storageBlocked = false;
        try {
          void parent.document.body;
        } catch {
          parentBlocked = true;
        }
        try {
          await new Promise((resolve, reject) => {
            const request = indexedDB.open('mars');
            request.onsuccess = resolve;
            request.onerror = reject;
          });
        } catch {
          storageBlocked = true;
        }
        return { parentBlocked, storageBlocked };
      });
    expect(isolation).toEqual({ parentBlocked: true, storageBlocked: true });
    const output = await htmlDownload(page);
    expect(await page.locator('#exportHtmlPreview iframe').getAttribute('srcdoc')).toBe(output.text);
    expect(output.text).not.toMatch(/PRIVATE REVIEW|su-comments|su-block|data-start|plausible|data-su-bundle/);
    expect(output.text).not.toContain('id="su-md-source"');
    expect(pagedRequests).toHaveLength(0);
    const pending = page.waitForEvent('download');
    await dlg(page).getByRole('button', { name: 'Download Markdown' }).click();
    expect(await readFile(await (await pending).path(), 'utf8')).toBe(TEXT);
    await dlg(page).getByRole('button', { name: 'Close export' }).click();
    await expect(page.locator('#exportBtn')).toBeFocused();
  });

  test('omits table-of-contents hints even when the document has no section headings', async ({ page }) => {
    await openDoc(page, '# Short document\n\nJust a paragraph.', 'Short.md');
    await openExport(page);
    await expect(dlg(page).locator('#exportTocHint')).toHaveCount(0);
    await expect(
      dlg(page).getByText('This document has too few section headings for a table of contents.', { exact: true }),
    ).toHaveCount(0);
    await expect(dlg(page).getByLabel('Table of contents', { exact: true })).toBeEnabled();
    await expect(htmlFrame(page).getByRole('navigation', { name: 'Table of contents', exact: true })).toHaveCount(0);
    expect((await htmlDownload(page)).text).not.toContain('id="toc"');
  });

  test('keeps drafts separate, validates custom width, and commits only output actions', async ({ page }) => {
    await page.addInitScript(() => localStorage.setItem('su-md-reading-width', '{"width":1234,"customWidth":1234}'));
    await openDoc(page);
    await openExport(page);
    await expect(dlg(page).getByLabel('Custom width (px)')).toHaveValue('1234');
    await dlg(page).getByLabel('Custom width (px)').fill('319');
    await expect(dlg(page).getByText('Enter a whole number from 320 to 2400.')).toBeVisible();
    await expect(dlg(page).getByRole('button', { name: 'Download HTML', exact: true })).toBeDisabled();
    await dlg(page).getByLabel('Custom width (px)').fill('1400');
    await expect(dlg(page).getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled();
    await dlg(page).getByRole('button', { name: 'Close export' }).click();
    expect(await page.evaluate(() => localStorage.getItem('mars.exportSettings'))).toBeNull();
    await openExport(page);
    await expect(dlg(page).getByLabel('Custom width (px)')).toHaveValue('1234');
    await dlg(page).getByLabel('Reading width', { exact: true }).selectOption('1100');
    await expect(dlg(page).getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled();
    await htmlDownload(page);
    const committed = await page.evaluate(() => JSON.parse(localStorage.getItem('mars.exportSettings')!));
    expect(committed.html.readingWidth).toBe(1100);
    expect(await page.evaluate(() => JSON.parse(localStorage.getItem('su-md-reading-width')!).width)).toBe(1234);
    await dlg(page).getByRole('button', { name: 'Close export' }).click();
    await openExport(page);
    await expect(dlg(page).getByLabel('Reading width', { exact: true })).toHaveValue('1100');
  });

  test('retains body-only source, toggles reader features, and escapes document titles', async ({ page }) => {
    await openDoc(page, TEXT, 'Escaped <title>.md');
    await openExport(page);
    await dlg(page).getByLabel('Embed Markdown source', { exact: true }).check();
    await dlg(page).getByLabel('PDF action in exported HTML').uncheck();
    await dlg(page).getByLabel('In-page find', { exact: true }).uncheck();
    await expect(dlg(page).getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled();
    const output = await htmlDownload(page);
    expect(output.name).toBe('Internal title.html');
    expect(output.text).toContain('<\\/script>');
    expect(output.text).not.toMatch(/PRIVATE REVIEW|id="pdf-btn"|id="find-btn"/);
    const source = await htmlFrame(page).locator('#su-md-source').textContent();
    expect(source).toContain('## One');
    expect(source).not.toMatch(/title:|su-comments/);
  });

  test('exports a non-current document without navigation, with native focus return', async ({ page }) => {
    await openDoc(page, '# First\n\nFirst body.', 'First.md');
    await page.getByRole('button', { name: 'New document, folder, or import' }).click();
    await page.getByRole('menuitem', { name: 'New document', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Source', exact: true })).toHaveAttribute('aria-pressed', 'true');
    await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
    await page.evaluate(() =>
      (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(
        '# Second\n\nSecond body.',
      ),
    );
    await page.getByRole('button', { name: 'Preview', exact: true }).click();
    await expect(page.locator('#previewArticle')).toContainText('Second body.');
    const route = page.url();
    await page.getByRole('button', { name: 'Actions for First', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Export…', exact: true }).click();
    await expect(dlg(page).getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled();
    await expect(htmlFrame(page).locator('article')).toContainText('First body.');
    expect(page.url()).toBe(route);
    await expect(page.locator('#previewArticle')).toContainText('Second body.');
    expect((await htmlDownload(page)).name).toBe('First.html');
    await page.keyboard.press('Escape');
    await expect(page.locator('#exportBtn')).toBeFocused();
  });

  test('PDF shortcut lazily prepares real pages, dimensions, isolated print requests, and cached tabs', async ({
    page,
  }) => {
    await page.addInitScript(() => {
      window.print = () => {
        document.documentElement.dataset.printRequests = String(
          Number(document.documentElement.dataset.printRequests ?? 0) + 1,
        );
      };
    });
    await openDoc(page);
    await page.keyboard.press('ControlOrMeta+p');
    await expect(dlg(page).getByRole('tab', { name: 'PDF' })).toHaveAttribute('aria-selected', 'true');
    await expect(dlg(page).getByRole('button', { name: 'Print / Save PDF', exact: true })).toBeEnabled({
      timeout: 60000,
    });
    for (const [paper, orientation, expected] of [
      ['A4', 'portrait', [210, 297]],
      ['A4', 'landscape', [297, 210]],
      ['Letter', 'portrait', [215.9, 279.4]],
      ['Letter', 'landscape', [279.4, 215.9]],
    ] as const) {
      await dlg(page).getByLabel('Paper size', { exact: true }).selectOption(paper);
      await dlg(page).getByLabel('Orientation', { exact: true }).selectOption(orientation);
      await expect(dlg(page).getByRole('button', { name: 'Print / Save PDF', exact: true })).toBeEnabled({
        timeout: 60000,
      });
      const dimensions = await pdfFrame(page)
        .locator('.pagedjs_page')
        .first()
        .evaluate((el) => ({ width: (el as HTMLElement).offsetWidth, height: (el as HTMLElement).offsetHeight }));
      expect(dimensions.width).toBeCloseTo((expected[0] * 96) / 25.4, 0);
      expect(dimensions.height).toBeCloseTo((expected[1] * 96) / 25.4, 0);
    }
    const prepared = await page.locator('#exportPdfPreview iframe').getAttribute('srcdoc');
    await dlg(page).getByRole('button', { name: 'Print / Save PDF', exact: true }).click();
    await expect(pdfFrame(page).locator('html')).toHaveAttribute('data-print-requests', '1');
    await expect(page.locator('#toast')).toContainText('Print requested');
    await dlg(page).getByRole('tab', { name: 'HTML', exact: true }).click();
    await expect(dlg(page).getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled();
    await readyPdf(page);
    expect(await page.locator('#exportPdfPreview iframe').getAttribute('srcdoc')).toBe(prepared);
    expect(await page.locator('#exportPdfPreview iframe').getAttribute('sandbox')).not.toContain('allow-same-origin');
    await expect(pdfFrame(page).getByRole('button', { name: 'Print / Save PDF', exact: true })).toHaveCount(0);
    await dlg(page).getByRole('button', { name: 'Print / Save PDF', exact: true }).click();
    await expect(pdfFrame(page).locator('html')).toHaveAttribute('data-print-requests', '2');
    await page.emulateMedia({ media: 'print' });
    const printed = await pdfFrame(page)
      .locator('.pagedjs_page')
      .first()
      .evaluate((el) => ({
        width: el.getBoundingClientRect().width,
        transform: getComputedStyle(el.parentElement!).transform,
      }));
    expect(printed.width).toBeCloseTo((279.4 * 96) / 25.4, 0);
    expect(printed.transform).toBe('none');
    await expect(pdfFrame(page).locator('#print-frame-action')).toHaveCount(0);
  });

  test('shows preparation status and disables printing only until PDF pages are ready', async ({ page }) => {
    let release!: () => void, requested!: () => void;
    const pending = new Promise<void>((resolve) => {
      release = resolve;
    });
    const started = new Promise<void>((resolve) => {
      requested = resolve;
    });
    await page.route('**/pagedjs@*/**', async (route) => {
      requested();
      await pending;
      await route.continue();
    });
    await openDoc(page);
    await openExport(page);
    await dlg(page).getByRole('tab', { name: 'PDF', exact: true }).click();
    try {
      await started;
      await expect(dlg(page).getByRole('status')).toHaveText('Preparing preview…');
      await expect(dlg(page).locator('#exportPdfPreview')).toHaveAttribute('aria-busy', 'true');
      await expect(dlg(page).getByRole('button', { name: 'Print / Save PDF', exact: true })).toBeDisabled();
    } finally {
      release();
    }
    await expect(dlg(page).getByRole('button', { name: 'Print / Save PDF', exact: true })).toBeEnabled({
      timeout: 60000,
    });
    await expect(dlg(page).getByRole('status')).toBeHidden();
    await expect(dlg(page).locator('.export-status')).toBeHidden();
    await expect(dlg(page).locator('#exportPdfPreview')).not.toHaveAttribute('aria-busy');
    await expect(pdfFrame(page).getByRole('button')).toHaveCount(0);
    await dlg(page).getByRole('tab', { name: 'HTML', exact: true }).click();
    await readyPdf(page);
    await expect(dlg(page).locator('.export-status')).toBeHidden();
  });

  test('keeps PDF pages centered when scrolling appears or the viewport narrows', async ({ page }) => {
    await page.setViewportSize({ width: 2000, height: 1600 });
    await openDoc(page, '# Centered\n\nPage content.', 'Centered.md');
    await openExport(page);
    await readyPdf(page);
    for (const viewport of [
      { width: 2000, height: 1600 },
      { width: 2000, height: 844 },
      { width: 390, height: 844 },
    ]) {
      await page.setViewportSize(viewport);
      await expect
        .poll(() =>
          pdfFrame(page)
            .locator('.pagedjs_page')
            .first()
            .evaluate((page) => {
              const rect = page.getBoundingClientRect();
              return Math.abs(rect.x + rect.width / 2 - document.documentElement.clientWidth / 2);
            }),
        )
        .toBeLessThan(2);
      const colors = await pdfFrame(page)
        .locator('.pagedjs_page')
        .first()
        .evaluate((page) => ({
          page: getComputedStyle(page).backgroundColor,
          surround: getComputedStyle(document.body).backgroundColor,
        }));
      expect(colors).toEqual({ page: 'rgb(255, 255, 255)', surround: 'rgb(229, 231, 235)' });
    }
  });

  test('PDF dependency failures offer Retry while HTML and Markdown remain available', async ({ page }) => {
    await page.route('**/pagedjs@*/**', (route) => route.abort());
    await openDoc(page);
    await openExport(page);
    await dlg(page).getByRole('tab', { name: 'PDF', exact: true }).click();
    await expect(dlg(page).getByRole('button', { name: 'Retry', exact: true })).toBeVisible({ timeout: 20000 });
    await expect(dlg(page).getByRole('status')).toContainText('Could not prepare PDF preview');
    await expect(dlg(page).getByRole('button', { name: 'Print / Save PDF', exact: true })).toBeDisabled();
    await expect(dlg(page).getByRole('button', { name: 'Download Markdown' })).toBeEnabled();
    await dlg(page).getByRole('tab', { name: 'HTML', exact: true }).click();
    await expect(dlg(page).getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled();
    await page.unroute('**/pagedjs@*/**');
    await dlg(page).getByRole('tab', { name: 'PDF', exact: true }).click();
    await dlg(page).getByRole('button', { name: 'Retry', exact: true }).click();
    await expect(dlg(page).getByRole('button', { name: 'Print / Save PDF', exact: true })).toBeEnabled({
      timeout: 60000,
    });
  });

  test('ignores superseded frames and messages, cancels without output, and supports keyboard tabs', async ({
    page,
  }) => {
    await openDoc(page);
    await openExport(page);
    await dlg(page).getByLabel('Reading width', { exact: true }).selectOption('1100');
    await dlg(page).getByLabel('Reading width', { exact: true }).selectOption('full');
    await expect(dlg(page).getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled();
    await expect(page.locator('#exportHtmlPreview iframe')).toHaveCount(1);
    const output = await htmlDownload(page);
    expect(output.text).toContain('--su-reading-width:none');
    await dlg(page).getByRole('tab', { name: 'HTML', exact: true }).focus();
    await page.keyboard.press('ArrowRight');
    await expect(dlg(page).getByRole('tab', { name: 'PDF', exact: true })).toBeFocused();
    await expect(dlg(page).getByRole('tab', { name: 'PDF', exact: true })).toHaveAttribute('aria-selected', 'true');
    await page.evaluate(() => window.postMessage({ kind: 'mars-export', type: 'ready', operation: 'fake' }, '*'));
    await page.keyboard.press('Escape');
    await expect(dlg(page)).toBeHidden();
    await expect(page.locator('#exportDialog iframe')).toHaveCount(0);
    await expect(page.locator('#exportBtn')).toBeFocused();
  });

  test('retains interactive geometry and colored print snapshots in actual offline HTML files', async ({
    page,
    browserName,
  }) => {
    test.skip(
      browserName !== 'chromium',
      'Pixel assertions require the Chromium software GPU in this test environment.',
    );
    await page.goto('/');
    await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
    await page.locator('#fileInput').setInputFiles(resolve(__dirname, 'fixtures/3mf/face-colors.3mf'));
    await expect(page.locator('#previewArticle canvas')).toBeVisible({ timeout: 15000 });
    const model = page.locator('#previewArticle figure[data-kind="su-mesh"]');
    await model.getByRole('button', { name: 'Rotate right', exact: true }).click();
    const savedView = await model.getAttribute('data-current-view');
    await model.getByRole('button', { name: 'Save current view to source', exact: true }).click();
    await expect(model).toHaveAttribute('data-view', savedView!);
    await model.getByRole('button', { name: 'Rotate up', exact: true }).click();
    await openExport(page);
    await expect(htmlFrame(page).locator('figure[data-kind="su-mesh"]')).toHaveAttribute('data-view', savedView!);
    const image = await htmlFrame(page).locator('.su-stl-snapshot').getAttribute('src');
    await readyPdf(page);
    await expect(pdfFrame(page).locator('.su-stl-snapshot')).toHaveAttribute('src', image!);
    await expect(pdfFrame(page).locator('.su-stl-snapshot')).toBeVisible();
    await expect(pdfFrame(page).locator('figure[data-kind="su-mesh"]')).toHaveAttribute('data-view', savedView!);
    await dlg(page).getByRole('tab', { name: 'HTML', exact: true }).click();
    await expect(dlg(page).getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled();
    const output = await htmlDownload(page);
    const path = test.info().outputPath('reader.html');
    await writeFile(path, output.text);
    await page.context().setOffline(true);
    await page.goto(pathToFileURL(path).href);
    await expect(page.locator('figure canvas')).toBeVisible();
    await page.getByRole('button', { name: 'Rotate right', exact: true }).click();
    await page.getByRole('button', { name: 'Reset view', exact: true }).click();
    await page.emulateMedia({ media: 'print' });
    await expect(page.locator('figure canvas')).toBeHidden();
    await expect(page.locator('.su-stl-snapshot')).toBeVisible();
    const colors = await page.locator('.su-stl-snapshot').evaluate(async (element) => {
      const image = element as HTMLImageElement;
      await image.decode();
      const canvas = document.createElement('canvas');
      canvas.width = image.naturalWidth;
      canvas.height = image.naturalHeight;
      const context = canvas.getContext('2d')!;
      context.drawImage(image, 0, 0);
      const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
      const counts = { red: 0, green: 0, blue: 0 };
      for (let i = 0; i < pixels.length; i += 4) {
        const [r, g, b] = pixels.subarray(i, i + 3);
        if (r > 80 && r > g * 1.8 && r > b * 1.8) counts.red++;
        if (g > 80 && g > r * 1.8 && g > b * 1.8) counts.green++;
        if (b > 80 && b > r * 1.8 && b > g * 1.8) counts.blue++;
      }
      return counts;
    });
    expect(colors.red).toBeGreaterThan(100);
    expect(colors.green).toBeGreaterThan(100);
    expect(colors.blue).toBeGreaterThan(100);
  });

  for (const width of [320, 390, 768, 1280, 2000])
    for (const theme of ['dark', 'light']) {
      test(`fits ${width}px in ${theme} with bounded previews and visible primary actions`, async ({ page }) => {
        await page.setViewportSize({ width, height: 844 });
        await openDoc(page);
        await page.evaluate((theme) => {
          document.documentElement.dataset.theme = theme;
        }, theme);
        await openExport(page);
        const rect = await dlg(page).boundingBox();
        expect(rect!.x).toBeGreaterThanOrEqual(0);
        expect(rect!.x + rect!.width).toBeLessThanOrEqual(width);
        expect(rect!.width).toBeGreaterThan(width * 0.9);
        expect(rect!.height).toBeGreaterThan(844 * 0.9);
        expect(await dlg(page).evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
        const tabs = await dlg(page).getByRole('tablist', { name: 'Export format' }).boundingBox();
        const settings = await dlg(page).locator('.export-tabs').boundingBox();
        expect(Math.abs(tabs!.x + tabs!.width / 2 - settings!.x - settings!.width / 2)).toBeLessThan(1);
        await expect(dlg(page).getByRole('button', { name: 'Download HTML', exact: true })).toBeInViewport();
        await readyPdf(page);
        await expect(dlg(page).getByRole('button', { name: 'Print / Save PDF', exact: true })).toBeInViewport();
        const pageWidth = await pdfFrame(page)
          .locator('.pagedjs_page')
          .first()
          .evaluate((el) => el.getBoundingClientRect().width);
        const frameWidth = (await page.locator('#exportPdfPreview iframe').boundingBox())!.width;
        expect(pageWidth).toBeLessThanOrEqual(frameWidth + 1);
        const centers = await pdfFrame(page)
          .locator('.pagedjs_page')
          .evaluateAll((pages) =>
            pages.map((page) => {
              const rect = page.getBoundingClientRect();
              return Math.abs(rect.x + rect.width / 2 - document.documentElement.clientWidth / 2);
            }),
          );
        for (const distance of centers) expect(distance).toBeLessThan(2);
        await expect(dlg(page).locator('.export-status')).toBeHidden();
        await expect(pdfFrame(page).getByRole('button')).toHaveCount(0);
        await expect(dlg(page).getByText('White pages in every theme', { exact: false })).toBeVisible();
        await expect(pdfFrame(page).locator('html')).toHaveAttribute('data-theme', 'light');
        if (width < 700) {
          await dlg(page)
            .locator('.export-body')
            .evaluate((el) => {
              el.scrollTop = el.scrollHeight;
            });
          await expect(dlg(page).getByRole('tablist', { name: 'Export format' })).toBeInViewport();
        }
        await dlg(page).getByRole('button', { name: 'Close export' }).click();
      });
    }
});
