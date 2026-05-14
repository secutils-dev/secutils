import { expect, test } from '@playwright/test';

import { assertSeoBasics, assertSkillMd, getTool } from './_helpers';

const tool = getTool('echo');

test.describe(`${tool.name} (${tool.path})`, () => {
  test('SEO head block matches the AGENTS.md SEO budget', async ({ page }) => {
    const response = await page.goto(tool.path);
    expect(response?.ok()).toBeTruthy();
    await assertSeoBasics(page, tool);
  });

  test('skill .md is reachable with required frontmatter', async ({ request }) => {
    await assertSkillMd(request, tool);
  });

  test('configurator updates the previewed mock URL when status / body change', async ({ page }) => {
    await page.goto(tool.path);
    const status = page.locator('#status');
    const body = page.locator('#body');
    const preview = page.locator('#preview');
    await expect(preview).toBeVisible();

    await status.fill('418');
    await body.fill('I am a teapot');

    await expect.poll(async () => preview.inputValue(), { timeout: 5000 }).toMatch(/\?c=\S+$/);
    const url = await preview.inputValue();
    expect(url.startsWith('http')).toBeTruthy();
    expect(url).toContain('?c=');
  });

  test('the served mock returns the configured status and body when fetched', async ({ page, request }) => {
    await page.goto(tool.path);
    await page.locator('#status').fill('418');
    await page.locator('#body').fill('I am a teapot');
    await expect.poll(async () => page.locator('#preview').inputValue(), { timeout: 5000 }).toMatch(/\?c=\S+$/);

    const url = await page.locator('#preview').inputValue();
    const response = await request.get(url);
    expect(response.status()).toBe(418);
    expect(await response.text()).toBe('I am a teapot');
  });
});
