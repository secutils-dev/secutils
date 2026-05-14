import { expect, test } from '@playwright/test';

import { assertSeoBasics, assertSkillMd, getTool } from './_helpers';

const tool = getTool('pem');

test.describe(`${tool.name} (${tool.path})`, () => {
  test('SEO head block matches the AGENTS.md SEO budget', async ({ page }) => {
    const response = await page.goto(tool.path);
    expect(response?.ok()).toBeTruthy();
    await assertSeoBasics(page, tool);
  });

  test('skill .md is reachable with required frontmatter', async ({ request }) => {
    await assertSkillMd(request, tool);
  });

  test('the bundled "Try example" button decodes a sample certificate', async ({ page }) => {
    await page.goto(tool.path);
    await expect(page.locator('#emptyState')).toBeVisible();
    await page.locator('#exampleBtn').click();
    await expect(page.locator('#emptyState')).toBeHidden();
    await expect(page.locator('#outputContainer')).toContainText(/Subject|Issuer/i);
  });
});
