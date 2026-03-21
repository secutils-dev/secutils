import { join } from 'path';

import { expect, test } from '@playwright/test';

import { DOCS_IMG_DIR, EMAIL, ensureUserAndLogin, highlightOn, PASSWORD } from '../helpers';

const IMG_DIR = join(DOCS_IMG_DIR, 'export_import');

test.describe('Export/Import guide screenshots', () => {
  test.beforeEach(async ({ page, request }) => {
    await ensureUserAndLogin(request, page, { email: EMAIL, password: PASSWORD });
  });

  test('export and import data', async ({ page }) => {
    // Create a script and a secret so the export has data.
    await page.request.post('/api/user/scripts', {
      data: { name: 'my_responder_script', scriptType: 'responder', content: 'console.log("hello")' },
    });
    await page.request.post('/api/user/secrets', {
      data: { name: 'API_TOKEN', value: 'sk-live-abc123' },
    });

    // Step 1: Open settings and navigate to the Account tab.
    await page.getByRole('button', { name: 'Account menu' }).click();
    const settingsButton = page.getByText('Settings');
    await expect(settingsButton).toBeVisible();
    await settingsButton.click();

    const accountTab = page.getByRole('tab', { name: 'Account' });
    await expect(accountTab).toBeVisible({ timeout: 15000 });
    await accountTab.click();

    const exportButton = page.getByRole('button', { name: 'Export data' });
    await expect(exportButton).toBeVisible({ timeout: 15000 });
    await highlightOn(exportButton);
    await page.screenshot({ path: join(IMG_DIR, 'export_import_step1_account_tab.png') });

    // Step 2: Open the export modal.
    await exportButton.click();
    const exportModal = page.locator('.euiModal').filter({ has: page.getByText('Export data') });
    await expect(exportModal).toBeVisible({ timeout: 15000 });
    await page.screenshot({ path: join(IMG_DIR, 'export_import_step2_export_modal.png') });

    // Close the export modal.
    await exportModal.getByRole('button', { name: 'Cancel' }).click();
    await expect(exportModal).not.toBeVisible();

    // Step 3: Open the import modal.
    const importButton = page.getByRole('button', { name: 'Import data' });
    await importButton.click();
    const importModal = page.locator('.euiModal').filter({ has: page.getByText('Import data') });
    await expect(importModal).toBeVisible({ timeout: 15000 });
    await page.screenshot({ path: join(IMG_DIR, 'export_import_step3_import_modal.png') });

    // Close the import modal.
    await importModal.getByRole('button', { name: 'Cancel' }).click();
    await expect(importModal).not.toBeVisible();
  });
});
