import { join } from 'path';

import { expect, test } from '@playwright/test';

import {
  dismissAllToasts,
  DOCS_IMG_DIR,
  EMAIL,
  ensureUserAndLogin,
  fixEntityTimestamps,
  highlightOn,
  PASSWORD,
} from '../helpers';

const IMG_DIR = join(DOCS_IMG_DIR, 'secrets');

test.describe('Secrets guide screenshots', () => {
  test.beforeEach(async ({ page, request }) => {
    await ensureUserAndLogin(request, page, { email: EMAIL, password: PASSWORD });
    await fixEntityTimestamps(page, '**/api/user/secrets');
  });

  test('manage user secrets', async ({ page }) => {

    // Step 1: Open settings and navigate to Secrets tab — empty state.
    await page.getByRole('button', { name: 'Account menu' }).click();
    const settingsButton = page.getByText('Settings');
    await expect(settingsButton).toBeVisible();
    await settingsButton.click();

    const secretsTab = page.getByRole('tab', { name: 'Secrets' });
    await expect(secretsTab).toBeVisible({ timeout: 15000 });
    await secretsTab.click();

    await expect(page.getByText('No secrets yet')).toBeVisible({ timeout: 15000 });
    const addButton = page.getByRole('button', { name: 'Add secret' });
    await highlightOn(addButton);
    await page.screenshot({ path: join(IMG_DIR, 'secrets_step1_empty.png') });

    // Step 2: Open Add Secret modal and screenshot the form.
    await addButton.click();
    const modal = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add secret' }) });
    await expect(modal).toBeVisible({ timeout: 15000 });
    await modal.getByPlaceholder('MY_API_KEY').fill('API_TOKEN');
    await modal.getByPlaceholder('Enter secret value…').fill('sk-live-abc123def456');
    await page.screenshot({ path: join(IMG_DIR, 'secrets_step2_form.png') });

    // Submit and dismiss the toast.
    await modal.getByRole('button', { name: 'Create' }).click();
    await dismissAllToasts(page);
    await expect(page.getByText('API_TOKEN', { exact: true })).toBeVisible({ timeout: 15000 });
    await page.screenshot({ path: join(IMG_DIR, 'secrets_step3_created.png') });

    // Step 4: Create a second secret.
    await addButton.click();
    const modal2 = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add secret' }) });
    await expect(modal2).toBeVisible({ timeout: 15000 });
    await modal2.getByPlaceholder('MY_API_KEY').fill('DB_PASSWORD');
    await modal2.getByPlaceholder('Enter secret value…').fill('p@ssw0rd!');
    await modal2.getByRole('button', { name: 'Create' }).click();
    await dismissAllToasts(page);
    await expect(page.getByText('DB_PASSWORD', { exact: true })).toBeVisible({ timeout: 15000 });
    await page.screenshot({ path: join(IMG_DIR, 'secrets_step4_list.png') });
  });
});
