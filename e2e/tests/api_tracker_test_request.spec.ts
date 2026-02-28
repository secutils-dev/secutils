import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

test.describe.serial('API Tracker Test Request', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('shows test request panel in the edit flyout', async ({ page }) => {
    await page.goto('/ws/web_scraping__api');

    const createButton = page.getByRole('button', { name: 'Track API' });
    await expect(createButton).toBeVisible({ timeout: 15000 });

    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add API tracker' }) });
    await expect(flyout).toBeVisible();

    const testButton = flyout.getByRole('button', { name: 'Test request' });
    await expect(testButton).toBeVisible();
    await expect(testButton).toBeDisabled();
  });

  test('enables test button after filling URL', async ({ page }) => {
    await page.goto('/ws/web_scraping__api');

    const createButton = page.getByRole('button', { name: 'Track API' });
    await expect(createButton).toBeVisible({ timeout: 15000 });

    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add API tracker' }) });
    await expect(flyout).toBeVisible();

    const urlInput = flyout.getByLabel('URL');
    await urlInput.fill('https://api.example.com/data');

    const testButton = flyout.getByRole('button', { name: 'Test request' });
    await expect(testButton).toBeEnabled();
  });

  test('executes test request and shows response', async ({ page }) => {
    await page.route('**/api/utils/web_scraping/api/test', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          status: 200,
          headers: { 'content-type': 'application/json' },
          body: '{"result":"ok"}',
          latencyMs: 42,
        }),
      });
    });

    await page.goto('/ws/web_scraping__api');

    const createButton = page.getByRole('button', { name: 'Track API' });
    await expect(createButton).toBeVisible({ timeout: 15000 });

    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add API tracker' }) });
    await expect(flyout).toBeVisible();

    const urlInput = flyout.getByLabel('URL');
    await urlInput.fill('https://api.example.com/data');

    const testButton = flyout.getByRole('button', { name: 'Test request' });
    await testButton.click();

    await expect(flyout.getByText('200')).toBeVisible({ timeout: 15000 });
    await expect(flyout.getByText('42ms')).toBeVisible();
    await expect(flyout.getByText('"result"')).toBeVisible();

    await flyout.getByRole('tab', { name: /Headers/ }).click();
    await expect(flyout.locator('.euiCodeBlock').getByText('content-type: application/json')).toBeVisible();
  });

  test('shows error message on test failure', async ({ page }) => {
    await page.route('**/api/utils/web_scraping/api/test', async (route) => {
      await route.fulfill({
        status: 400,
        contentType: 'application/json',
        body: JSON.stringify({ message: 'Request failed: connection refused' }),
      });
    });

    await page.goto('/ws/web_scraping__api');

    const createButton = page.getByRole('button', { name: 'Track API' });
    await expect(createButton).toBeVisible({ timeout: 15000 });

    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add API tracker' }) });
    await expect(flyout).toBeVisible();

    const urlInput = flyout.getByLabel('URL');
    await urlInput.fill('https://api.example.com/data');

    const testButton = flyout.getByRole('button', { name: 'Test request' });
    await testButton.click();

    await expect(flyout.getByText('Request failed', { exact: false })).toBeVisible({ timeout: 15000 });
  });
});
