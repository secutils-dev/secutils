import { expect, test } from '@playwright/test';

import { dismissAllToasts, ensureUserAndLogin } from '../helpers';

test.describe.serial('Page tracker schedule presets', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('hourly schedule stores correct anchored cron', async ({ page }) => {
    const createRes = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'Hourly Schedule Test',
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
      },
    });
    expect(createRes.ok()).toBeTruthy();

    await page.goto('/ws/web_scraping__page');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Hourly Schedule Test' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout).toBeVisible();

    const frequencyRow = flyout.locator('.euiFormRow').filter({ has: page.locator('label', { hasText: 'Frequency' }) });
    const selects = frequencyRow.locator('select');

    await selects.nth(0).selectOption('@hourly');
    await expect(flyout.getByText('UTC')).toBeVisible();

    // Hourly: nth(1) = minute
    await selects.nth(1).selectOption('15');

    await flyout.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('Successfully updated')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    const listRes = await page.request.get('/api/utils/web_scraping/page');
    expect(listRes.ok()).toBeTruthy();
    const trackers = await listRes.json();
    const tracker = trackers.find((t: { name: string }) => t.name === 'Hourly Schedule Test');
    expect(tracker).toBeDefined();
    expect(tracker.retrack.config.job.schedule).toBe('0 15 * * * *');

    // Roundtrip: reopen and verify controls reflect saved values.
    await row.getByRole('button', { name: 'Edit' }).click();
    const editFlyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(editFlyout).toBeVisible();

    const editRow = editFlyout.locator('.euiFormRow').filter({ has: page.locator('label', { hasText: 'Frequency' }) });
    const editSelects = editRow.locator('select');

    await expect(editSelects.nth(0)).toHaveValue('@hourly');
    await expect(editSelects.nth(1)).toHaveValue('15');

    await editFlyout.getByRole('button', { name: 'Close' }).click();
    await expect(editFlyout).not.toBeVisible({ timeout: 10000 });
  });

  test('daily schedule stores correct anchored cron', async ({ page }) => {
    const createRes = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'Daily Schedule Test',
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
      },
    });
    expect(createRes.ok()).toBeTruthy();

    await page.goto('/ws/web_scraping__page');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Daily Schedule Test' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout).toBeVisible();

    const frequencyRow = flyout.locator('.euiFormRow').filter({ has: page.locator('label', { hasText: 'Frequency' }) });
    const selects = frequencyRow.locator('select');

    await selects.nth(0).selectOption('@daily');
    await expect(flyout.getByText('UTC')).toBeVisible();

    // Daily: nth(1) = hour, nth(2) = minute
    await selects.nth(1).selectOption('9');
    await selects.nth(2).selectOption('30');

    await flyout.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('Successfully updated')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    const listRes = await page.request.get('/api/utils/web_scraping/page');
    expect(listRes.ok()).toBeTruthy();
    const trackers = await listRes.json();
    const tracker = trackers.find((t: { name: string }) => t.name === 'Daily Schedule Test');
    expect(tracker).toBeDefined();
    expect(tracker.retrack.config.job.schedule).toBe('0 30 9 * * *');

    await row.getByRole('button', { name: 'Edit' }).click();
    const editFlyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(editFlyout).toBeVisible();

    const editRow = editFlyout.locator('.euiFormRow').filter({ has: page.locator('label', { hasText: 'Frequency' }) });
    const editSelects = editRow.locator('select');

    await expect(editSelects.nth(0)).toHaveValue('@daily');
    await expect(editSelects.nth(1)).toHaveValue('9');
    await expect(editSelects.nth(2)).toHaveValue('30');

    await editFlyout.getByRole('button', { name: 'Close' }).click();
    await expect(editFlyout).not.toBeVisible({ timeout: 10000 });
  });

  test('weekly schedule stores correct anchored cron', async ({ page }) => {
    const createRes = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'Weekly Schedule Test',
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
      },
    });
    expect(createRes.ok()).toBeTruthy();

    await page.goto('/ws/web_scraping__page');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Weekly Schedule Test' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout).toBeVisible();

    const frequencyRow = flyout.locator('.euiFormRow').filter({ has: page.locator('label', { hasText: 'Frequency' }) });
    const selects = frequencyRow.locator('select');

    await selects.nth(0).selectOption('@weekly');
    await expect(flyout.getByText('UTC')).toBeVisible();

    // Weekly: nth(1) = weekday, nth(2) = hour, nth(3) = minute
    await selects.nth(1).selectOption('3');
    await selects.nth(2).selectOption('14');
    await selects.nth(3).selectOption('45');

    await flyout.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('Successfully updated')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    const listRes = await page.request.get('/api/utils/web_scraping/page');
    expect(listRes.ok()).toBeTruthy();
    const trackers = await listRes.json();
    const tracker = trackers.find((t: { name: string }) => t.name === 'Weekly Schedule Test');
    expect(tracker).toBeDefined();
    expect(tracker.retrack.config.job.schedule).toBe('0 45 14 * * 3');

    await row.getByRole('button', { name: 'Edit' }).click();
    const editFlyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(editFlyout).toBeVisible();

    const editRow = editFlyout.locator('.euiFormRow').filter({ has: page.locator('label', { hasText: 'Frequency' }) });
    const editSelects = editRow.locator('select');

    await expect(editSelects.nth(0)).toHaveValue('@weekly');
    await expect(editSelects.nth(1)).toHaveValue('3');
    await expect(editSelects.nth(2)).toHaveValue('14');
    await expect(editSelects.nth(3)).toHaveValue('45');

    await editFlyout.getByRole('button', { name: 'Close' }).click();
    await expect(editFlyout).not.toBeVisible({ timeout: 10000 });
  });

  test('monthly schedule stores correct anchored cron', async ({ page }) => {
    const createRes = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'Monthly Schedule Test',
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
      },
    });
    expect(createRes.ok()).toBeTruthy();

    await page.goto('/ws/web_scraping__page');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Monthly Schedule Test' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout).toBeVisible();

    const frequencyRow = flyout.locator('.euiFormRow').filter({ has: page.locator('label', { hasText: 'Frequency' }) });
    const selects = frequencyRow.locator('select');

    await selects.nth(0).selectOption('@monthly');
    await expect(flyout.getByText('UTC')).toBeVisible();

    // Monthly: nth(1) = day-of-month, nth(2) = hour, nth(3) = minute
    await selects.nth(1).selectOption('15');
    await selects.nth(2).selectOption('8');
    await selects.nth(3).selectOption('0');

    await flyout.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('Successfully updated')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    const listRes = await page.request.get('/api/utils/web_scraping/page');
    expect(listRes.ok()).toBeTruthy();
    const trackers = await listRes.json();
    const tracker = trackers.find((t: { name: string }) => t.name === 'Monthly Schedule Test');
    expect(tracker).toBeDefined();
    expect(tracker.retrack.config.job.schedule).toBe('0 0 8 15 * *');

    await row.getByRole('button', { name: 'Edit' }).click();
    const editFlyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(editFlyout).toBeVisible();

    const editRow = editFlyout.locator('.euiFormRow').filter({ has: page.locator('label', { hasText: 'Frequency' }) });
    const editSelects = editRow.locator('select');

    await expect(editSelects.nth(0)).toHaveValue('@monthly');
    await expect(editSelects.nth(1)).toHaveValue('15');
    await expect(editSelects.nth(2)).toHaveValue('8');
    await expect(editSelects.nth(3)).toHaveValue('0');

    await editFlyout.getByRole('button', { name: 'Close' }).click();
    await expect(editFlyout).not.toBeVisible({ timeout: 10000 });
  });
});

test.describe.serial('API tracker schedule presets', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('weekly schedule stores correct anchored cron for API tracker', async ({ page }) => {
    const createRes = await page.request.post('/api/utils/web_scraping/api', {
      data: {
        name: 'Weekly API Schedule Test',
        config: { revisions: 3 },
        target: { url: 'https://secutils.dev/' },
      },
    });
    expect(createRes.ok()).toBeTruthy();

    await page.goto('/ws/web_scraping__api');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Weekly API Schedule Test' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit API tracker' }) });
    await expect(flyout).toBeVisible();

    const frequencyRow = flyout.locator('.euiFormRow').filter({ has: page.locator('label', { hasText: 'Frequency' }) });
    const selects = frequencyRow.locator('select');

    await selects.nth(0).selectOption('@weekly');
    await expect(flyout.getByText('UTC')).toBeVisible();

    // Weekly: nth(1) = weekday, nth(2) = hour, nth(3) = minute
    await selects.nth(1).selectOption('5');
    await selects.nth(2).selectOption('18');
    await selects.nth(3).selectOption('30');

    await flyout.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('Successfully updated')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    const listRes = await page.request.get('/api/utils/web_scraping/api');
    expect(listRes.ok()).toBeTruthy();
    const trackers = await listRes.json();
    const tracker = trackers.find((t: { name: string }) => t.name === 'Weekly API Schedule Test');
    expect(tracker).toBeDefined();
    expect(tracker.retrack.config.job.schedule).toBe('0 30 18 * * 5');

    await row.getByRole('button', { name: 'Edit' }).click();
    const editFlyout = page
      .getByRole('dialog')
      .filter({ has: page.getByRole('heading', { name: 'Edit API tracker' }) });
    await expect(editFlyout).toBeVisible();

    const editRow = editFlyout.locator('.euiFormRow').filter({ has: page.locator('label', { hasText: 'Frequency' }) });
    const editSelects = editRow.locator('select');

    await expect(editSelects.nth(0)).toHaveValue('@weekly');
    await expect(editSelects.nth(1)).toHaveValue('5');
    await expect(editSelects.nth(2)).toHaveValue('18');
    await expect(editSelects.nth(3)).toHaveValue('30');

    await editFlyout.getByRole('button', { name: 'Close' }).click();
    await expect(editFlyout).not.toBeVisible({ timeout: 10000 });
  });
});
