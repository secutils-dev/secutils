import { expect, test } from '@playwright/test';

import { assertSeoBasics, assertSkillMd, getTool } from './_helpers';
import { PROMOTED_TOOLS, TOOLS, TOOLS_HOST } from './registry';

const tool = getTool('index');

test.describe(`${tool.name} (${tool.path})`, () => {
  test('SEO head block matches the AGENTS.md SEO budget (uses ItemList JSON-LD)', async ({ page }) => {
    const response = await page.goto(tool.path);
    expect(response?.ok()).toBeTruthy();
    await assertSeoBasics(page, tool);

    const ld = JSON.parse((await page.locator('script[type="application/ld+json"]').first().textContent())!);
    expect(ld['@type']).toBe('ItemList');
    expect(Array.isArray(ld.itemListElement)).toBeTruthy();
    expect(ld.itemListElement.length).toBeGreaterThanOrEqual(PROMOTED_TOOLS.length);
  });

  test('llms.txt is reachable and lists every promoted tool slug', async ({ request }) => {
    await assertSkillMd(request, tool);
    const r = await request.get('/llms.txt');
    const body = await r.text();
    for (const t of PROMOTED_TOOLS) {
      expect(body, `llms.txt should mention ${t.slug}`).toContain(t.path);
    }
  });

  test('every promoted tool has a working anchor on the index page', async ({ page }) => {
    await page.goto(tool.path);
    for (const t of PROMOTED_TOOLS) {
      const link = page.locator(`a[href$="${t.path}"], a[href*="${TOOLS_HOST}${t.path}"]`).first();
      await expect(link, `index should link to ${t.slug} (${t.path})`).toBeVisible();
    }
  });

  test('non-promoted tools are not linked from the index page', async ({ page }) => {
    await page.goto(tool.path);
    const nonPromoted = TOOLS.filter((t) => !t.promote && t.slug !== 'index');
    for (const t of nonPromoted) {
      const link = page.locator(`a[href$="${t.path}"], a[href*="${TOOLS_HOST}${t.path}"]`);
      await expect(link, `index should not link to non-promoted tool ${t.slug}`).toHaveCount(0);
    }
  });
});
