import { readFile } from 'node:fs/promises';

import type { Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

import { assertSeoBasics, assertSkillMd, getTool } from './_helpers';

const tool = getTool('md');

// Drives the imperative editor API the page exposes once the editor mounts
// (CodeMirror from esm.sh, or a <textarea> fallback). This keeps tests
// independent of the editor's internal DOM.
async function setMarkdown(page: Page, md: string): Promise<void> {
  await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
  await page.evaluate((text) => {
    (window as { __suEditorAPI: { setText(t: string): void } }).__suEditorAPI.setText(text);
  }, md);
}

test.describe(`${tool.name} (${tool.path})`, () => {
  test('SEO head block matches the AGENTS.md SEO budget', async ({ page }) => {
    const response = await page.goto(tool.path);
    expect(response?.ok()).toBeTruthy();
    await assertSeoBasics(page, tool);
  });

  test('skill .md is reachable with required frontmatter', async ({ request }) => {
    await assertSkillMd(request, tool);
  });

  test('reading-first: preview is the default view and editor is hidden', async ({ page }) => {
    await page.goto(tool.path);
    await expect(page.locator('#previewWrap')).toBeVisible();
    await expect(page.locator('#editorMount')).toBeHidden();
    // The empty start state offers actionable entry points.
    await expect(page.locator('#emptyState')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Open file' })).toBeVisible();
  });

  test('renders Markdown into the preview', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, '# Hello *world*\n\n- item 1\n- item 2');

    await expect(page.locator('#previewArticle h1')).toContainText('Hello');
    await expect(page.locator('#previewArticle ul li')).toHaveCount(2);
    await expect(page.locator('#emptyState')).toBeHidden();
  });

  test('unmatched inline HTML stays literal without swallowing the rest of the document', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(
      page,
      [
        'An ESLint/Oxlint plugin to enforce rules like "no raw colors" and',
        '"no restyling <Button> from outside" with errors to steer agents',
        'back to your variants and tokens.',
        '',
        'Unmatched <span class="example"> and </em> stay visible.',
        '',
        'Paired <em>HTML</em>, <button>Click</button>, and a break<br>still work.',
        '',
        '## Next section',
        '',
        '- A later item',
      ].join('\n'),
    );

    const preview = page.locator('#previewArticle');
    await expect(preview).toContainText('"no restyling <Button> from outside"');
    await expect(preview).toContainText('Unmatched <span class="example"> and </em> stay visible.');
    await expect(preview.locator('button')).toHaveText(['Click']);
    await expect(preview.locator('em')).toHaveText(['HTML']);
    await expect(preview.locator('br')).toHaveCount(1);
    await expect(preview.locator(':scope > h2')).toContainText('Next section');
    await expect(preview.locator(':scope > ul > li')).toHaveText(['A later item']);

    await page.getByRole('button', { name: 'HTML', exact: true }).click();
    const exported = page.frameLocator('#htmlPreview').locator('article');
    await expect(exported).toContainText('"no restyling <Button> from outside"');
    await expect(exported.locator('button')).toHaveText(['Click']);
    await expect(exported.locator(':scope > h2')).toContainText('Next section');
    await expect(exported.locator(':scope > ul > li')).toHaveText(['A later item']);
  });

  test('Source view swaps preview for the editor and back', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, '# Editable');

    await page.getByRole('button', { name: 'Source', exact: true }).click();
    await expect(page.locator('#editorMount')).toBeVisible();
    await expect(page.locator('#previewWrap')).toBeHidden();

    await page.keyboard.press('Escape');
    await expect(page.locator('#previewWrap')).toBeVisible();
    await expect(page.locator('#editorMount')).toBeHidden();
  });

  test('HTML view renders the exported document in an iframe', async ({ page }) => {
    await page.goto(tool.path);
    // The HTML segment is disabled until there is content.
    await expect(page.getByRole('button', { name: 'HTML', exact: true })).toBeDisabled();

    await setMarkdown(page, '# Exported\n\nBody text.');
    await page.getByRole('button', { name: 'HTML', exact: true }).click();

    const frame = page.frameLocator('#htmlPreview');
    await expect(frame.locator('article h1')).toContainText('Exported');
    await expect(page.locator('#previewWrap')).toBeHidden();

    // The "Find in page" HTML option (default on) adds an in-export find widget.
    await frame.locator('#find-btn').click();
    await expect(frame.locator('#su-find.open')).toBeVisible();
    await frame.locator('#su-find input').fill('Body');
    await expect(frame.locator('article mark.su-find-hit')).toContainText('Body');
  });

  test('reading width presets and custom value match Preview, HTML view, and export', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, '# Width test\n\nA readable paragraph.');
    const article = page.locator('#previewArticle');
    const htmlArticle = page.frameLocator('#htmlPreview').locator('article');
    await expect(article).toHaveCSS('max-width', '800px');

    await page.getByRole('button', { name: 'Options', exact: true }).click();
    const width = page.getByLabel('Reading width');
    await width.selectOption('1100');
    await expect(article).toHaveCSS('max-width', '1100px');
    await page.getByRole('button', { name: 'HTML', exact: true }).click();
    await expect(htmlArticle).toHaveCSS('max-width', '1100px');

    await page.getByRole('button', { name: 'Options', exact: true }).click();
    await width.selectOption('full');
    await expect(article).toHaveCSS('max-width', 'none');
    await expect(htmlArticle).toHaveCSS('max-width', 'none');

    await width.selectOption('custom');
    const custom = page.getByLabel('Custom width (px)');
    await expect(custom).toBeVisible();
    await custom.fill('960');
    await expect(article).toHaveCSS('max-width', '960px');
    await expect(htmlArticle).toHaveCSS('max-width', '960px');
    await custom.fill('9999');
    await custom.press('Tab');
    await expect(custom).toHaveValue('960');
    await expect(htmlArticle).toHaveCSS('max-width', '960px');

    const downloadPromise = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Export', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Download HTML' }).click();
    const output = await readFile(await (await downloadPromise).path(), 'utf8');
    expect(output).toContain('style="--su-reading-width: 960px"');
    expect(output).not.toContain('--su-reading-width: 9999px');
  });

  test('exported HTML lets readers change width without changing its default', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, '# Reader\n\n## First\n\nA\n\n## Second\n\nB\n\n## Third\n\nC');
    await page.getByRole('button', { name: 'Options', exact: true }).click();
    await page.getByLabel('Reading width').selectOption('1100');
    const downloadPromise = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Export', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Download HTML' }).click();
    const output = await readFile(await (await downloadPromise).path(), 'utf8');

    await page.route('https://example.test/exported.html', (route) =>
      route.fulfill({ contentType: 'text/html', body: output }),
    );
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto('https://example.test/exported.html');
    const article = page.locator('main article');
    await expect(article).toHaveCSS('max-width', '1100px');
    await expect(page.locator('#toc')).toBeVisible();
    await page.getByRole('button', { name: 'Reading settings' }).click();
    await page.getByLabel('Reading width').selectOption('full');
    await expect(article).toHaveCSS('max-width', 'none');
    const tocBox = await page.locator('#toc').boundingBox();
    const articleBox = await article.boundingBox();
    expect(tocBox && articleBox && articleBox.x >= tocBox.x + tocBox.width + 16).toBeTruthy();
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(1440);

    await page.setViewportSize({ width: 390, height: 844 });
    await expect(page.locator('#toc')).toBeHidden();
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
    const settingsBox = await page.locator('#reading-settings').boundingBox();
    expect(settingsBox && settingsBox.x >= 0 && settingsBox.x + settingsBox.width <= 390).toBeTruthy();

    await page.getByRole('button', { name: 'Use document default' }).click();
    await expect(article).toHaveCSS('max-width', '1100px');
    await page.getByLabel('Reading width').selectOption('custom');
    await page.getByLabel('Custom width (px)').fill('900');
    await expect(article).toHaveCSS('max-width', '900px');
    await page.reload();
    await expect(article).toHaveCSS('max-width', '1100px');

    await page.emulateMedia({ media: 'print' });
    await expect(article).toHaveCSS('max-width', 'none');
    await expect(page.locator('#reading-settings')).toBeHidden();
  });

  test('GitHub alerts and ==highlights== are enhanced', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, '> [!NOTE]\n> Heads up.\n\nSome ==marked== text.');

    await expect(page.locator('#previewArticle .markdown-alert-note')).toBeVisible();
    await expect(page.locator('#previewArticle .markdown-alert-title')).toContainText('Note');
    await expect(page.locator('#previewArticle mark.su-hl')).toContainText('marked');
  });

  test('highlights inline code in tables in preview and HTML export', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(
      page,
      [
        '| Behaviour | **Workflows** (this PR) | AB **conversation** | AB **agent** | **Dashboards** write-restricted |',
        '| --- | --- | --- | --- | --- |',
        '| Storage | own ES index | own ES index | own ES index | root metadata on the saved object |',
        '| Field name | `access_control` | `access_control` | `access_control` | `accessControl` |',
        '| Modes | `private`, `public` | `private`, `public` | `private`, `shared`, `public` | `default`, `write_restricted` |',
        '| What the restrictive mode restricts | visibility | visibility | visibility | ==writes only - everyone still reads== |',
        '| Default for new objects | ==`public`== | `private` | `private` | `default` |',
      ].join('\n'),
    );

    const preview = page.locator('#previewArticle');
    await expect(preview.locator('mark.su-hl')).toHaveText(['writes only - everyone still reads', 'public']);
    await expect(preview.getByRole('cell', { name: 'public', exact: true }).locator('mark.su-hl code')).toBeVisible();
    await expect(preview).not.toContainText('==');

    await page.getByRole('button', { name: 'HTML', exact: true }).click();
    const exported = page.frameLocator('#htmlPreview').locator('article');
    await expect(exported.locator('mark.su-hl')).toHaveText(['writes only - everyone still reads', 'public']);
    await expect(exported.getByRole('cell', { name: 'public', exact: true }).locator('mark.su-hl code')).toBeVisible();
    await expect(exported).not.toContainText('==');
  });

  test('highlights preserve nested inline formatting and literal code delimiters', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(
      page,
      'Before ==use `a == b` with **bold**, *emphasis*, and [a link](https://example.com)== after.\n\n' +
        '==``a ` b == c``== and ==another==.',
    );

    const marks = page.locator('#previewArticle mark.su-hl');
    await expect(marks).toHaveText(['use a == b with bold, emphasis, and a link', 'a ` b == c', 'another']);
    await expect(marks.first().locator('code')).toHaveText('a == b');
    await expect(marks.first().locator('strong')).toHaveText('bold');
    await expect(marks.first().locator('em')).toHaveText('emphasis');
    await expect(marks.first().getByRole('link', { name: 'a link' })).toHaveAttribute('href', 'https://example.com');
  });

  test('keeps code, escaped, empty, and unmatched highlight markers literal', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(
      page,
      [
        '`==inline code==`',
        '```text\n==fenced code==\n```',
        '    ==indented code==',
        '<code>==HTML code==</code>',
        '\\==escaped\\==',
        'Empty == == and ==== stay intact before ==highlighted==.',
        'Unmatched ==marker',
      ].join('\n\n'),
    );

    const preview = page.locator('#previewArticle');
    await expect(preview.locator('mark.su-hl')).toHaveText(['highlighted']);
    await expect(preview.locator('code')).toHaveText([
      '==inline code==',
      '==fenced code==\n',
      '==indented code==\n',
      '==HTML code==',
    ]);
    await expect(preview).toContainText('==escaped==');
    await expect(preview).toContainText('Empty == == and ==== stay intact before highlighted.');
    await expect(preview).toContainText('Unmatched ==marker');
  });

  test('mermaid code blocks render to an inline SVG diagram', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, ['```mermaid', 'flowchart LR', '  A[Start] --> B[End]', '```'].join('\n'));

    // Mermaid is lazy-loaded from a CDN, so allow extra time for the diagram.
    await expect(page.locator('#previewArticle figure.su-mermaid svg')).toBeVisible({ timeout: 15000 });
  });

  test('find-in-page highlights matches', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, 'alpha beta alpha gamma alpha');

    await page.getByRole('button', { name: 'Find in document' }).click();
    await page.getByPlaceholder('Find in document').fill('alpha');

    await expect(page.locator('#previewArticle mark.find-hit')).toHaveCount(3);
    await expect(page.locator('#findCount')).toContainText('/3');
  });

  test('loads Markdown from the URL fragment', async ({ page }) => {
    // base64url of "| len(LE u32) | deflate-raw('# Frag load') |" for "# Frag load".
    const md = '# Frag load\n\nFrom the fragment.';
    const encoded = await page.evaluate(async (text: string) => {
      const bytes = new TextEncoder().encode(text);
      const stream = new Blob([bytes]).stream().pipeThrough(new CompressionStream('deflate-raw'));
      const deflated = new Uint8Array(await new Response(stream).arrayBuffer());
      const out = new Uint8Array(4 + deflated.length);
      new DataView(out.buffer).setUint32(0, bytes.length, true);
      out.set(deflated, 4);
      let s = '';
      for (const b of out) s += String.fromCharCode(b);
      return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
    }, md);

    await page.goto(`${tool.path}#${encoded}`);
    await expect(page.locator('#previewArticle h1')).toContainText('Frag load');
    // The empty "start" panel and the loading state must not linger once hydrated.
    await expect(page.locator('#emptyState')).toBeHidden();
    await expect(page.locator('#loadingState')).toBeHidden();
  });

  test('Load URL extracts embedded source from an exported HTML document', async ({ page }) => {
    // An exported HTML doc carrying the "Embed source" <script type="text/markdown">.
    // The </script> inside the source is escaped to <\/script>, exactly as the
    // tool's exporter writes it; the loader must unescape it on the way back.
    const html = [
      '<!DOCTYPE html><html><head><title>x</title></head><body>',
      '<main><article><h1>Rendered</h1></article></main>',
      '<script type="text/markdown" id="su-md-source" data-source="markdown">',
      '# Round trip\n\nConsole: `console.log("<\\/script>")`',
      '</script>',
      '</body></html>',
    ].join('\n');

    await page.route('https://example.com/exported.html', (route) =>
      route.fulfill({ contentType: 'text/html', body: html }),
    );

    await page.goto(tool.path);
    await page.getByLabel('Markdown URL to load').fill('https://example.com/exported.html');
    await page.getByRole('button', { name: 'Load URL' }).click();

    // The embedded Markdown is rendered, not the document's own <h1>Rendered</h1>.
    await expect(page.locator('#previewArticle h1')).toContainText('Round trip');
    await expect(page.locator('#previewArticle code')).toContainText('console.log("</script>")');
  });
});
