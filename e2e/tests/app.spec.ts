import { expect, test } from '@playwright/test';

test.describe('Secutils.dev Application', () => {
  test('API status endpoint is reachable', async ({ request }) => {
    const response = await request.get('/api/status');
    expect(response.ok()).toBeTruthy();

    const body = await response.json();
    expect(body).toHaveProperty('version');
    expect(body).toHaveProperty('level', 'available');
  });

  test('home page loads successfully', async ({ page }) => {
    await page.goto('/');
    await expect(page).toHaveTitle(/Secutils\.dev/);
  });

  test('login page is accessible', async ({ page }) => {
    await page.goto('/');

    // The app should show a login/signup form or redirect to Kratos.
    await expect(page.locator('input[name="password"], input[name="identifier"], form')).toBeVisible({
      timeout: 15000,
    });
  });
});
