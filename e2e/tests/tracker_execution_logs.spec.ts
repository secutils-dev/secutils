import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

const FIXED_TS = 1740000000;

function mockExecutionLogs(trackerId: string) {
  return [
    {
      id: '00000000-0000-0000-0000-000000000001',
      trackerId,
      startedAt: FIXED_TS,
      finishedAt: FIXED_TS + 3,
      status: 'success',
      isManual: false,
      hasChanges: true,
      durationMs: 850,
      revisionSize: 2048,
      phases: [
        { phase: 'fetch', durationMs: 800, status: 'success' },
        { phase: 'persist', durationMs: 50, status: 'success' },
      ],
    },
    {
      id: '00000000-0000-0000-0000-000000000002',
      trackerId,
      startedAt: FIXED_TS - 3600,
      finishedAt: FIXED_TS - 3598,
      status: 'failure',
      error: 'Timeout',
      isManual: true,
      durationMs: 5000,
      phases: [{ phase: 'fetch', durationMs: 5000, status: 'failure' }],
    },
  ];
}

test.describe('Tracker execution logs', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('clearing logs refreshes the logs grid to show empty state', async ({ page }) => {
    const createRes = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'Clear Logs Test',
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "ok"; }' },
      },
    });
    expect(createRes.ok()).toBeTruthy();
    const tracker = (await createRes.json()) as { id: string };

    const mockLogs = mockExecutionLogs(tracker.id);

    // Return mock logs on first request, empty on subsequent (after clear).
    let cleared = false;
    await page.route('**/api/utils/web_scraping/*/*/logs', async (route) => {
      if (route.request().method() !== 'GET') {
        await route.fallback();
        return;
      }
      await route.fulfill({ json: cleared ? [] : mockLogs });
    });

    // Mock the clear_logs endpoint to succeed.
    await page.route('**/api/utils/web_scraping/*/*/clear_logs', async (route) => {
      cleared = true;
      await route.fulfill({ status: 200, body: '' });
    });

    // Mock a revision so the control panel shows.
    await page.route('**/api/utils/web_scraping/*/*/history', async (route) => {
      await route.fulfill({
        json: [{ id: '00000000-0000-0000-0000-000000000099', data: { original: 'ok' }, createdAt: FIXED_TS }],
      });
    });

    await page.route('**/api/utils/web_scraping/*/logs_summary', async (route) => {
      await route.fulfill({ json: {} });
    });

    await page.goto('/ws/web_scraping__page');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Clear Logs Test' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    await row.getByRole('button', { name: 'Show history' }).click();

    const logsButton = page.getByRole('button', { name: 'Logs' });
    await expect(logsButton).toBeVisible({ timeout: 10000 });
    await logsButton.click();

    // Verify logs are displayed.
    await expect(page.getByText('OK', { exact: true })).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('Fail', { exact: true })).toBeVisible();

    // Click "Clear logs" button.
    const clearButton = page.getByRole('button', { name: 'Clear logs' });
    await clearButton.click();

    const confirmModal = page.getByRole('alertdialog').filter({
      has: page.getByRole('heading', { name: 'Clear page tracker execution logs?' }),
    });
    await expect(confirmModal).toBeVisible();
    await confirmModal.getByRole('button', { name: 'Clear' }).click();

    // Verify the logs grid now shows the empty state.
    await expect(page.getByText('No execution logs yet')).toBeVisible({ timeout: 10000 });
  });

  test('logs view is accessible when first revision fetch fails', async ({ page }) => {
    const createRes = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'Failed Revision Test',
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "ok"; }' },
      },
    });
    expect(createRes.ok()).toBeTruthy();
    const tracker = (await createRes.json()) as { id: string };

    const mockLogs = mockExecutionLogs(tracker.id);

    // Make the history endpoint fail.
    await page.route('**/api/utils/web_scraping/*/*/history', async (route) => {
      await route.fulfill({ status: 500, json: { message: 'Internal server error' } });
    });

    await page.route('**/api/utils/web_scraping/*/*/logs', async (route) => {
      if (route.request().method() !== 'GET') {
        await route.fallback();
        return;
      }
      await route.fulfill({ json: mockLogs });
    });

    await page.route('**/api/utils/web_scraping/*/logs_summary', async (route) => {
      await route.fulfill({ json: {} });
    });

    await page.goto('/ws/web_scraping__page');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Failed Revision Test' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    await row.getByRole('button', { name: 'Show history' }).click();

    // The Logs button should be visible and clickable even though revisions failed.
    const logsButton = page.getByRole('button', { name: 'Logs' });
    await expect(logsButton).toBeVisible({ timeout: 10000 });
    await expect(logsButton).toBeEnabled();
    await logsButton.click();

    // Verify execution logs are displayed.
    await expect(page.getByText('OK', { exact: true })).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('Fail', { exact: true })).toBeVisible();
  });
});
