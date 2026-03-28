import { join } from 'path';

import { expect, test } from '@playwright/test';

import {
  dismissAllToasts,
  DOCS_IMG_DIR,
  EMAIL,
  ensureUserAndLogin,
  fixEntityTimestamps,
  goto,
  highlightOn,
  PASSWORD,
} from '../helpers';

const IMG_DIR = join(DOCS_IMG_DIR, 'tags');

test.describe('Tags guide screenshots', () => {
  test.beforeEach(async ({ page, request }) => {
    await ensureUserAndLogin(request, page, { email: EMAIL, password: PASSWORD });
    await fixEntityTimestamps(page, '**/api/user/tags');
  });

  test('manage tags in settings', async ({ page }) => {
    // Step 1: Open settings and navigate to the Tags tab - empty state.
    await page.getByRole('button', { name: 'Account menu' }).click();
    const settingsButton = page.getByText('Settings');
    await expect(settingsButton).toBeVisible();
    await settingsButton.click();

    const tagsTab = page.getByRole('tab', { name: 'Tags' });
    await expect(tagsTab).toBeVisible({ timeout: 15000 });
    await tagsTab.click();

    await expect(page.getByText('No tags yet')).toBeVisible({ timeout: 15000 });
    const addButton = page.getByRole('button', { name: 'Add tag' });
    await highlightOn(addButton);
    await page.screenshot({ path: join(IMG_DIR, 'tags_step1_empty.png') });

    // Step 2: Open Add Tag modal and screenshot the form.
    await addButton.click();
    const modal = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add tag' }) });
    await expect(modal).toBeVisible({ timeout: 15000 });
    await modal.getByPlaceholder('e.g. production, staging, personal').fill('production');
    await page.screenshot({ path: join(IMG_DIR, 'tags_step2_form.png') });

    // Submit and dismiss the toast.
    await modal.getByRole('button', { name: 'Create' }).click();
    await dismissAllToasts(page);
    await expect(page.getByText('production', { exact: true })).toBeVisible({ timeout: 15000 });

    // Step 3: Create additional tags via API for a richer list.
    for (const [name, color] of [
      ['staging', 'warning'],
      ['personal', 'primary'],
    ] as const) {
      const res = await page.request.post('/api/user/tags', {
        data: { name, color },
      });
      expect(res.ok()).toBeTruthy();
    }

    // Reload the settings flyout to show all tags.
    await page.getByRole('button', { name: 'Close this dialog' }).click();
    await page.getByRole('button', { name: 'Account menu' }).click();
    await page.getByText('Settings').click();
    const tagsTabReload = page.getByRole('tab', { name: 'Tags' });
    await expect(tagsTabReload).toBeVisible({ timeout: 15000 });
    await tagsTabReload.click();
    await expect(page.getByText('staging')).toBeVisible({ timeout: 15000 });
    await page.screenshot({ path: join(IMG_DIR, 'tags_step3_list.png') });
  });

  test('assign tags to a responder', async ({ page }) => {
    // Pre-create tags.
    for (const [name, color] of [
      ['production', 'default'],
      ['staging', 'warning'],
      ['personal', 'primary'],
    ] as const) {
      await page.request.post('/api/user/tags', { data: { name, color } });
    }

    await fixEntityTimestamps(page, '**/api/utils/webhooks/responders');

    // Navigate to responders.
    await goto(page, '/ws/webhooks__responders');
    const createButton = page.getByRole('button', { name: 'Create responder' });
    await expect(createButton).toBeVisible({ timeout: 15000 });
    await createButton.click();

    // Wait for flyout.
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add responder' }) });
    await expect(flyout).toBeVisible({ timeout: 15000 });

    // Fill in the name and a fixed path (default is random).
    await flyout.getByLabel('Name').fill('My API Responder');
    const pathInput = flyout.getByRole('textbox', { name: 'Path' });
    await pathInput.fill('/my-api');

    // Select tags - EuiComboBox keeps dropdown open in multi-select mode.
    await flyout.getByPlaceholder('Select tags').click();
    await page.getByRole('option', { name: 'production' }).click();
    await page.getByRole('option', { name: 'staging' }).click();
    await page.keyboard.press('Escape');

    await page.screenshot({ path: join(IMG_DIR, 'tags_step4_assign.png') });

    // Close the flyout.
    await flyout.getByRole('button', { name: 'Close' }).click();
    await page.getByRole('button', { name: 'Discard' }).click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });
  });

  test('filter by tags and global scope', async ({ page }) => {
    // Pre-create tags.
    const tagIds: Record<string, string> = {};
    for (const [name, color] of [
      ['production', 'default'],
      ['staging', 'warning'],
    ] as const) {
      const res = await page.request.post('/api/user/tags', { data: { name, color } });
      expect(res.ok()).toBeTruthy();
      const tag = await res.json();
      tagIds[name] = tag.id;
    }

    // Create responders with tags.
    for (const [name, path, tags] of [
      ['Prod API', '/prod-api', [tagIds['production']]],
      ['Prod Webhook', '/prod-hook', [tagIds['production']]],
      ['Staging Mock', '/staging-mock', [tagIds['staging']]],
    ] as const) {
      const res = await page.request.post('/api/utils/webhooks/responders', {
        data: {
          name,
          location: { pathType: '=', path },
          method: 'ANY',
          enabled: true,
          settings: { requestsToTrack: 10, statusCode: 200 },
          tagIds: tags,
        },
      });
      expect(res.ok()).toBeTruthy();
    }

    await fixEntityTimestamps(page, '**/api/utils/webhooks/responders');

    // Navigate to responders and sort by Name for a deterministic order.
    await goto(page, '/ws/webhooks__responders');
    await expect(page.getByText('Prod API')).toBeVisible({ timeout: 15000 });
    await expect(page.getByText('Staging Mock')).toBeVisible({ timeout: 15000 });
    await page.getByRole('button', { name: /Name/ }).click();
    await page.screenshot({ path: join(IMG_DIR, 'tags_step5_unfiltered.png') });

    // Step 6: Use page-level tag filter.
    const tagFilter = page.getByRole('button', { name: /Tags/ });
    await tagFilter.click();
    await page.getByRole('option', { name: 'production' }).click();
    await page.keyboard.press('Escape');
    await expect(page.getByText('Staging Mock')).not.toBeVisible();
    await expect(page.getByText('Prod API')).toBeVisible();
    await page.screenshot({ path: join(IMG_DIR, 'tags_step6_filtered.png') });
  });
});
