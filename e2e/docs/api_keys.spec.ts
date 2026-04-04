import { join } from 'path';

import { expect, test } from '@playwright/test';

import { DOCS_IMG_DIR, EMAIL, ensureUserAndLogin, goto, highlightOn, PASSWORD } from '../helpers';

const IMG_DIR = join(DOCS_IMG_DIR, 'api_keys');

test.describe('API Keys guide screenshots', () => {
  test.beforeEach(async ({ page, request }) => {
    await ensureUserAndLogin(request, page, { email: EMAIL, password: PASSWORD });

    // Clean up any existing API keys.
    const keysRes = await page.request.get('/api/user/api_keys');
    if (keysRes.ok()) {
      const keys = await keysRes.json();
      for (const key of keys) {
        await page.request.delete(`/api/user/api_keys/${key.id}`);
      }
    }
  });

  test('manage API keys', async ({ page }) => {
    // Step 1: Navigate to Settings → Security tab and show the "Manage API keys" button.
    await goto(page, '/ws/workspace__overview');
    await page.getByRole('button', { name: 'Account menu' }).click();
    const settingsButton = page.getByText('Settings');
    await expect(settingsButton).toBeVisible();
    await settingsButton.click();

    const securityTab = page.getByRole('tab', { name: 'Security' });
    await expect(securityTab).toBeVisible({ timeout: 15000 });
    await securityTab.click();

    const manageButton = page.getByRole('button', { name: 'Manage API keys' });
    await expect(manageButton).toBeVisible({ timeout: 15000 });
    await highlightOn(manageButton);
    await page.screenshot({ path: join(IMG_DIR, 'api_keys_step1_security_tab.png') });

    // Step 2: Open the API Keys modal — empty state.
    await manageButton.click();
    const modal = page.locator('.euiModal').filter({ has: page.getByText('API keys') });
    await expect(modal).toBeVisible({ timeout: 15000 });
    await expect(modal.getByText('No API keys yet.')).toBeVisible({ timeout: 15000 });

    const createButton = modal.getByRole('button', { name: 'Create API key' });
    await highlightOn(createButton);
    await page.screenshot({ path: join(IMG_DIR, 'api_keys_step2_empty.png') });

    // Step 3: Click "Create API key" and fill in the form.
    await createButton.click();
    const createDialog = page.getByRole('alertdialog');
    await expect(createDialog).toBeVisible({ timeout: 15000 });
    await createDialog.getByPlaceholder('e.g. CI deployment key').fill('CI deployment key');
    await page.screenshot({ path: join(IMG_DIR, 'api_keys_step3_create_form.png') });

    // Step 4: Save and show the token reveal.
    await createDialog.getByRole('button', { name: 'Create' }).click();
    await expect(modal.getByText('API key created')).toBeVisible({ timeout: 15000 });
    await page.screenshot({ path: join(IMG_DIR, 'api_keys_step4_token_reveal.png') });

    // Dismiss the token.
    await modal.getByRole('button', { name: 'Dismiss' }).click();

    // Step 5: Create a second key to show a populated list.
    await createButton.click();
    const createDialog2 = page.getByRole('alertdialog');
    await expect(createDialog2).toBeVisible({ timeout: 15000 });
    await createDialog2.getByPlaceholder('e.g. CI deployment key').fill('Staging environment');
    await createDialog2.getByRole('button', { name: 'Create' }).click();
    await expect(modal.getByText('API key created')).toBeVisible({ timeout: 15000 });
    await modal.getByRole('button', { name: 'Dismiss' }).click();

    // Intercept the API keys list response to pin timestamps for stable screenshots.
    await page.route('**/api/user/api_keys', async (route) => {
      if (route.request().method() !== 'GET') {
        await route.continue();
        return;
      }
      const response = await route.fetch();
      if (!response.ok()) {
        await route.fulfill({ response });
        return;
      }
      const json = await response.json();
      const FIXED_TS = 1740000000;
      for (const key of json) {
        key.createdAt = FIXED_TS;
        key.updatedAt = FIXED_TS;
      }
      await route.fulfill({ response, json });
    });

    // Reload the modal to pick up intercepted timestamps.
    await modal.getByRole('button', { name: 'Close', exact: true }).click();
    await expect(modal).not.toBeVisible();
    await page.getByRole('button', { name: 'Manage API keys' }).click();
    await expect(modal).toBeVisible({ timeout: 15000 });
    await expect(modal.getByText('CI deployment key')).toBeVisible({ timeout: 15000 });
    await expect(modal.getByText('Staging environment')).toBeVisible({ timeout: 15000 });
    await page.screenshot({ path: join(IMG_DIR, 'api_keys_step5_list.png') });

    await modal.getByRole('button', { name: 'Close', exact: true }).click();
  });
});
