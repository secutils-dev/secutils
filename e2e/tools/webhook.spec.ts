import { expect, test } from '@playwright/test';

import { assertSeoBasics, assertSkillMd, getTool } from './_helpers';

const tool = getTool('webhook');

test.describe(`${tool.name} (${tool.path})`, () => {
  test('SEO head block matches the AGENTS.md SEO budget', async ({ page }) => {
    const response = await page.goto(tool.path);
    expect(response?.ok()).toBeTruthy();
    await assertSeoBasics(page, tool);
  });

  test('skill .md is reachable with required frontmatter', async ({ request }) => {
    await assertSkillMd(request, tool);
  });

  test('renders the session controls and mints a fresh webhook on first visit', async ({ page }) => {
    await page.goto(tool.path);

    // Session catalog controls are always present, even before registration.
    await expect(page.getByRole('combobox')).toBeVisible();
    await expect(page.getByRole('button', { name: '+ New' })).toBeVisible({ timeout: 15000 });

    // A fresh visit either surfaces the registration overlay (while keys are
    // generated and the public key is registered) or the resulting webhook URL.
    const regOverlay = page.locator('#regOverlay');
    const urlBlock = page.locator('#urlBlock');
    await expect
      .poll(async () => (await regOverlay.isVisible()) || (await urlBlock.isVisible()), {
        timeout: 15000,
      })
      .toBeTruthy();
  });

  test('privacy modal explains the end-to-end encryption model', async ({ page }) => {
    await page.goto(tool.path);

    await page.getByRole('button', { name: 'Privacy', exact: true }).click();
    const dialog = page.locator('#privacyDialog');
    await expect(dialog).toBeVisible();
    await expect(dialog).toContainText(/end-to-end/i);

    await page.getByRole('button', { name: 'Close' }).click();
    await expect(dialog).not.toBeVisible();
  });
});
