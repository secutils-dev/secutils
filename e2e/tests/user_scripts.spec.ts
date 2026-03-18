import { expect, test } from '@playwright/test';

import { dismissAllToasts, EMAIL, ensureUserAndLogin, fixEntityTimestamps, PASSWORD } from '../helpers';

test.describe('User Scripts guide screenshots', () => {
  test.beforeEach(async ({ page, request }) => {
    await ensureUserAndLogin(request, page, { email: EMAIL, password: PASSWORD });
    await fixEntityTimestamps(page, '**/api/user/scripts');
  });

  test('duplicate name rejection', async ({ page }) => {
    // Create a script via API
    const createResponse = await page.request.post('/api/user/scripts', {
      data: {
        name: 'UNIQUE_SCRIPT',
        scriptType: 'responder',
        content: '(() => { return { statusCode: 200 }; })();',
      },
    });
    expect(createResponse.ok()).toBeTruthy();

    // Navigate to scripts settings
    await page.getByRole('button', { name: 'Account menu' }).click();
    await page.getByText('Settings').click();
    const scriptsTab = page.getByRole('tab', { name: 'Scripts' });
    await expect(scriptsTab).toBeVisible({ timeout: 15000 });
    await scriptsTab.click();

    // Wait for the script to be visible
    await expect(page.getByText('UNIQUE_SCRIPT', { exact: true })).toBeVisible({ timeout: 15000 });

    // Try to create a new script with the same name
    await page.getByRole('button', { name: 'Add script' }).click();
    const modal = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add script' }) });
    await expect(modal).toBeVisible({ timeout: 15000 });

    // Fill in the same name
    await modal.getByPlaceholder('MY_SCRIPT').fill('UNIQUE_SCRIPT');
    await modal.getByRole('combobox').selectOption('responder');
    // Wait for Monaco editor and type some content to enable the Create button
    await expect(modal.locator('.monaco-editor')).toBeVisible({ timeout: 15000 });
    await modal.locator('.monaco-editor textarea').click({ force: true });
    await page.keyboard.type('(() => { return { statusCode: 200 }; })();');

    // Click Create button
    await modal.getByRole('button', { name: 'Create' }).click();

    // Expect toast message about duplicate name
    await expect(page.getByText("A script with name 'UNIQUE_SCRIPT' already exists.")).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);

    // Close modal
    await modal.getByRole('button', { name: 'Cancel' }).click();
    await expect(modal).not.toBeVisible();
  });
});
