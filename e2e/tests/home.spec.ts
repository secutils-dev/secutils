import { expect, test } from '@playwright/test';

import { ensureUserAndLogin, goto } from '../helpers';

test.describe('Home page', () => {
  test.describe('unauthenticated', () => {
    test('redirects to sign-in page', async ({ page }) => {
      await goto(page, '/');

      await expect(page.getByPlaceholder('Email')).toBeVisible({ timeout: 15000 });
      await expect(page.getByRole('button', { name: 'Sign in', exact: true })).toBeVisible();
    });
  });

  test.describe('authenticated', () => {
    test.beforeEach(async ({ page, request }) => {
      await ensureUserAndLogin(request, page);
    });

    test('shows welcome header inside a panel', async ({ page }) => {
      await goto(page, '/ws');

      await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });
      await expect(page.getByText('Your open-source security toolbox')).toBeVisible();
    });

    test('shows progress indicator for a new user', async ({ page }) => {
      await goto(page, '/ws');

      await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });
      await expect(page.getByText('using 0 of 4 tools')).toBeVisible({ timeout: 15000 });
    });

    test('displays all four utility cards', async ({ page }) => {
      await goto(page, '/ws');

      await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });

      for (const title of ['Webhooks', 'Digital Certificates', 'Content Security Policy', 'Web Scraping']) {
        await expect(page.getByText(title, { exact: true }).first()).toBeVisible();
      }
    });

    test('clicking a utility card navigates to the tool', async ({ page }) => {
      await goto(page, '/ws');

      await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });

      await page.getByRole('button', { name: 'Webhooks', exact: true }).click();

      await expect(page).toHaveURL(/\/ws\/webhooks__responders/, { timeout: 15000 });
    });

    test('guide links point to documentation', async ({ page }) => {
      await goto(page, '/ws');

      await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });

      const guideLinks = page.getByRole('link', { name: 'Guide' });
      await expect(guideLinks).toHaveCount(4);

      const hrefs = await guideLinks.evaluateAll((links) => links.map((l) => (l as HTMLAnchorElement).href));
      expect(hrefs.some((h) => h.includes('/docs/guides/webhooks'))).toBeTruthy();
      expect(hrefs.some((h) => h.includes('/docs/category/digital-certificates'))).toBeTruthy();
      expect(hrefs.some((h) => h.includes('/docs/guides/web_security/csp'))).toBeTruthy();
      expect(hrefs.some((h) => h.includes('/docs/category/web-scraping'))).toBeTruthy();
    });

    test('displays learn and community links', async ({ page }) => {
      await goto(page, '/ws');

      await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });

      await expect(page.getByRole('link', { name: 'Getting Started' })).toBeVisible();
      await expect(page.getByRole('link', { name: "What's New" })).toBeVisible();
      await expect(page.getByRole('link', { name: 'Contribute' })).toBeVisible();
    });

    test('web scraping card shows Pages and APIs sub-links', async ({ page }) => {
      await goto(page, '/ws');

      await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });

      const webScrapingCard = page.locator('.euiCard.euiPanel').filter({ hasText: 'Web Scraping' });
      await expect(webScrapingCard.getByRole('button', { name: 'Pages' })).toBeVisible();
      await expect(webScrapingCard.getByRole('button', { name: 'APIs' })).toBeVisible();

      await webScrapingCard.getByRole('button', { name: 'Pages' }).click();
      await expect(page).toHaveURL(/\/ws\/web_scraping__page/, { timeout: 15000 });

      await goto(page, '/ws');

      await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });

      const webScrapingCard2 = page.locator('.euiCard.euiPanel').filter({ hasText: 'Web Scraping' });
      await webScrapingCard2.getByRole('button', { name: 'APIs' }).click();
      await expect(page).toHaveURL(/\/ws\/web_scraping__api/, { timeout: 15000 });
    });

    test('shows get started checklist for a new user', async ({ page }) => {
      await goto(page, '/ws');

      await expect(page.getByRole('heading', { name: 'Get started', level: 3 })).toBeVisible({ timeout: 15000 });

      for (const prompt of [
        'Create your first webhook responder',
        'Generate a certificate template',
        'Set up a content security policy',
        'Track your first web page or API',
      ]) {
        await expect(page.getByText(prompt)).toBeVisible();
      }

      // Each incomplete tool should have a "Try it" button.
      const tryItButtons = page.getByRole('button', { name: 'Try it' });
      await expect(tryItButtons).toHaveCount(4);
    });

    test('checklist and recent items update after creating a responder', async ({ page }) => {
      await goto(page, '/ws');

      await expect(page.getByText('using 0 of 4 tools')).toBeVisible({ timeout: 15000 });

      // Create a responder via API.
      const createResponse = await page.request.post('/api/utils/webhooks/responders', {
        data: {
          name: 'Home Page Test',
          location: { pathType: '=', path: '/home-test' },
          method: 'GET',
          enabled: true,
          settings: { requestsToTrack: 3, statusCode: 200, body: 'ok' },
        },
      });
      expect(createResponse.ok()).toBeTruthy();

      // Navigate back to home and verify.
      await goto(page, '/ws');

      await expect(page.getByText('using 1 of 4 tools')).toBeVisible({ timeout: 15000 });
      await expect(page.getByText('1 item')).toBeVisible();

      // Checklist: webhooks prompt is replaced with the tool title (completed).
      await expect(page.getByText('Create your first webhook responder')).not.toBeVisible();

      // Checklist still shows for the remaining 3 uncompleted tools.
      const tryItButtons = page.getByRole('button', { name: 'Try it' });
      await expect(tryItButtons).toHaveCount(3);

      // Recent items section appears with the new responder.
      await expect(page.getByRole('heading', { name: 'Recent items', level: 3 })).toBeVisible();
      await expect(page.getByRole('button', { name: 'Home Page Test' })).toBeVisible();
    });
  });
});
