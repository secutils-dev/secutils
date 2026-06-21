import { expect, test } from '@playwright/test';

import { assertSeoBasics, assertSkillMd, getTool } from './_helpers';

const tool = getTool('md');

// Drives the imperative editor API the page exposes once the editor mounts
// (CodeMirror from esm.sh, or a <textarea> fallback). This keeps tests
// independent of the editor's internal DOM. See markdown-to-html.spec.ts.
async function setMarkdown(page: import('@playwright/test').Page, md: string): Promise<void> {
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

  test('GitHub alerts and ==highlights== are enhanced', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, '> [!NOTE]\n> Heads up.\n\nSome ==marked== text.');

    await expect(page.locator('#previewArticle .markdown-alert-note')).toBeVisible();
    await expect(page.locator('#previewArticle .markdown-alert-title')).toContainText('Note');
    await expect(page.locator('#previewArticle mark.su-hl')).toContainText('marked');
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
      '<\/script>',
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
