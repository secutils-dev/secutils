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

    // The default landing screen shows the upload zone; switch to paste mode
    // so the editor is interactive without requiring a file picker.
    await page.locator('#pasteToggle').click();
    const editor = page.locator('#editor');
    await expect(editor).toBeVisible();
    await editor.fill('# Hello *world*\n\n- item 1\n- item 2');
    await page.locator('#convertBtn').click();
    await expect(page.locator('#previewArticle')).toContainText('Hello');
    await expect(page.locator('#previewArticle h1')).toContainText('Hello');
    await expect(page.locator('#previewArticle ul li')).toHaveCount(2);
  });
});
