import { expect, test } from '@playwright/test';

import { assertSeoBasics, assertSkillMd, getTool } from './_helpers';

const tool = getTool('md-to-html');

test.describe(`${tool.name} (${tool.path})`, () => {
  test('SEO head block matches the AGENTS.md SEO budget', async ({ page }) => {
    const response = await page.goto(tool.path);
    expect(response?.ok()).toBeTruthy();
    await assertSeoBasics(page, tool);
  });

  test('skill .md is reachable with required frontmatter', async ({ request }) => {
    await assertSkillMd(request, tool);
  });

  test('typing Markdown updates the rendered preview', async ({ page }) => {
    await page.goto(tool.path);

    // The editor (CodeMirror, lazy-loaded from esm.sh, or a <textarea> fallback)
    // exposes a small imperative API on window once mounted. Drive it directly so
    // the test does not depend on the editor's internal DOM.
    await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
    await page.evaluate(() => {
      (window as { __suEditorAPI: { setText(t: string): void } }).__suEditorAPI.setText(
        '# Hello *world*\n\n- item 1\n- item 2',
      );
    });

    await expect(page.locator('#previewArticle')).toContainText('Hello');
    await expect(page.locator('#previewArticle h1')).toContainText('Hello');
    await expect(page.locator('#previewArticle ul li')).toHaveCount(2);
  });

  test('mermaid code blocks render to an inline SVG diagram', async ({ page }) => {
    await page.goto(tool.path);

    await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
    await page.evaluate(() => {
      (window as { __suEditorAPI: { setText(t: string): void } }).__suEditorAPI.setText(
        ['```mermaid', 'flowchart LR', '  A[Start] --> B[End]', '```'].join('\n'),
      );
    });

    // Mermaid is lazy-loaded from a CDN, so allow extra time for the diagram.
    await expect(page.locator('#previewArticle figure.su-mermaid svg')).toBeVisible({ timeout: 15000 });
  });
});
