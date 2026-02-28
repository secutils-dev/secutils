import { expect, test } from '@playwright/test';

import { dismissAllToasts, ensureUserAndLogin } from '../helpers';

test.describe.serial('Page tracker enable/disable', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('disabling a page tracker persists and shows disabled state', async ({ page }) => {
    const createRes = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'Toggle Page Tracker',
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
      },
    });
    expect(createRes.ok()).toBeTruthy();

    await page.goto('/ws/web_scraping__page');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Toggle Page Tracker' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    // Verify no offline icon initially.
    await expect(row.getByRole('img', { name: 'Tracker is disabled' })).not.toBeVisible();

    // Open Edit flyout and disable the tracker.
    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout).toBeVisible();

    const enableSwitch = flyout.getByLabel('Enable tracker');
    await expect(enableSwitch).toBeChecked();
    await enableSwitch.uncheck();
    await expect(enableSwitch).not.toBeChecked();

    await flyout.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('Successfully updated')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    // Verify disabled icon is shown in the grid.
    await expect(row.getByRole('img', { name: 'Tracker is disabled' })).toBeVisible();

    // Verify the state persisted: reopen the flyout and check the switch.
    await row.getByRole('button', { name: 'Edit' }).click();
    const editFlyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(editFlyout).toBeVisible();
    await expect(editFlyout.getByLabel('Enable tracker')).not.toBeChecked();
    await editFlyout.getByRole('button', { name: 'Close' }).click();
    await expect(editFlyout).not.toBeVisible({ timeout: 10000 });
  });

  test('re-enabling a disabled page tracker persists and removes disabled state', async ({ page }) => {
    const createRes = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'Re-enable Page Tracker',
        enabled: false,
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
      },
    });
    expect(createRes.ok()).toBeTruthy();

    await page.goto('/ws/web_scraping__page');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Re-enable Page Tracker' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    // Verify offline icon is shown initially.
    await expect(row.getByRole('img', { name: 'Tracker is disabled' })).toBeVisible();

    // Open Edit flyout and re-enable the tracker.
    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout).toBeVisible();

    const enableSwitch = flyout.getByLabel('Enable tracker');
    await expect(enableSwitch).not.toBeChecked();
    await enableSwitch.check();
    await expect(enableSwitch).toBeChecked();

    await flyout.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('Successfully updated')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    // Verify offline icon is gone.
    await expect(row.getByRole('img', { name: 'Tracker is disabled' })).not.toBeVisible();
  });
});

test.describe.serial('API tracker enable/disable', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('disabling an API tracker persists and shows disabled state', async ({ page }) => {
    const createRes = await page.request.post('/api/utils/web_scraping/api', {
      data: {
        name: 'Toggle API Tracker',
        config: { revisions: 3 },
        target: { url: 'https://secutils.dev/' },
      },
    });
    expect(createRes.ok()).toBeTruthy();

    await page.goto('/ws/web_scraping__api');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Toggle API Tracker' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    // Verify no offline icon initially.
    await expect(row.getByRole('img', { name: 'Tracker is disabled' })).not.toBeVisible();

    // Open Edit flyout and disable the tracker.
    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit API tracker' }) });
    await expect(flyout).toBeVisible();

    const enableSwitch = flyout.getByLabel('Enable tracker');
    await expect(enableSwitch).toBeChecked();
    await enableSwitch.uncheck();
    await expect(enableSwitch).not.toBeChecked();

    await flyout.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('Successfully updated')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    // Verify disabled icon is shown in the grid.
    await expect(row.getByRole('img', { name: 'Tracker is disabled' })).toBeVisible();

    // Verify the state persisted: reopen the flyout and check the switch.
    await row.getByRole('button', { name: 'Edit' }).click();
    const editFlyout = page
      .getByRole('dialog')
      .filter({ has: page.getByRole('heading', { name: 'Edit API tracker' }) });
    await expect(editFlyout).toBeVisible();
    await expect(editFlyout.getByLabel('Enable tracker')).not.toBeChecked();
    await editFlyout.getByRole('button', { name: 'Close' }).click();
    await expect(editFlyout).not.toBeVisible({ timeout: 10000 });
  });

  test('re-enabling a disabled API tracker persists and removes disabled state', async ({ page }) => {
    const createRes = await page.request.post('/api/utils/web_scraping/api', {
      data: {
        name: 'Re-enable API Tracker',
        enabled: false,
        config: { revisions: 3 },
        target: { url: 'https://secutils.dev/' },
      },
    });
    expect(createRes.ok()).toBeTruthy();

    await page.goto('/ws/web_scraping__api');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Re-enable API Tracker' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    // Verify offline icon is shown initially.
    await expect(row.getByRole('img', { name: 'Tracker is disabled' })).toBeVisible();

    // Open Edit flyout and re-enable the tracker.
    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit API tracker' }) });
    await expect(flyout).toBeVisible();

    const enableSwitch = flyout.getByLabel('Enable tracker');
    await expect(enableSwitch).not.toBeChecked();
    await enableSwitch.check();
    await expect(enableSwitch).toBeChecked();

    await flyout.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('Successfully updated')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    // Verify offline icon is gone.
    await expect(row.getByRole('img', { name: 'Tracker is disabled' })).not.toBeVisible();
  });
});
