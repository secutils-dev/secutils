import { expect, test } from '@playwright/test';

import { ensureUserAndLogin, goto } from '../helpers';

test.describe('Workspace links', () => {
  test.beforeEach(async ({ page, request }) => {
    await ensureUserAndLogin(request, page);
  });

  test('sidebar leaf utilities are rendered as links', async ({ page }) => {
    await goto(page, '/ws');
    await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });

    const sidebar = page.locator('aside');
    // "Policies" is nested under a collapsible "CSP" group in Web Security.
    const policiesLink = sidebar.getByRole('link', { name: 'Policies', exact: true });
    if (!(await policiesLink.isVisible())) {
      const cspButton = sidebar.getByRole('button', { name: 'CSP', exact: true });
      if (await cspButton.isVisible()) {
        await cspButton.click();
      }
    }

    const expectedLinks: Record<string, string> = {
      Responders: '/ws/webhooks__responders',
      'Certificate templates': '/ws/certificates__certificate_templates',
      'Private keys': '/ws/certificates__private_keys',
      Policies: '/ws/web_security__csp__policies',
      'Page trackers': '/ws/web_scraping__page',
      'API trackers': '/ws/web_scraping__api',
    };

    for (const [name, path] of Object.entries(expectedLinks)) {
      const link = sidebar.getByRole('link', { name, exact: true });
      await expect(link).toBeVisible();
      await expect(link).toHaveAttribute('href', path);
    }
  });

  test('responder name links to filtered grid view by ID', async ({ page }) => {
    const primaryName = 'linkable-responder-primary';
    const secondaryName = 'linkable-responder-secondary';

    const createPrimaryResponderResponse = await page.request.post('/api/webhooks/responders', {
      data: {
        name: primaryName,
        location: { pathType: '=', path: '/linkable-primary' },
        method: 'GET',
        enabled: true,
        settings: { requestsToTrack: 0, statusCode: 200 },
      },
    });
    expect(createPrimaryResponderResponse.ok()).toBeTruthy();

    const createSecondaryResponderResponse = await page.request.post('/api/webhooks/responders', {
      data: {
        name: secondaryName,
        location: { pathType: '=', path: '/linkable-secondary' },
        method: 'GET',
        enabled: true,
        settings: { requestsToTrack: 0, statusCode: 200 },
      },
    });
    expect(createSecondaryResponderResponse.ok()).toBeTruthy();

    const respondersResponse = await page.request.get('/api/webhooks/responders');
    expect(respondersResponse.ok()).toBeTruthy();
    const responders = (await respondersResponse.json()) as { id: string; name: string }[];
    const primaryResponder = responders.find((responder) => responder.name === primaryName);
    const secondaryResponder = responders.find((responder) => responder.name === secondaryName);
    expect(primaryResponder).toBeDefined();
    expect(secondaryResponder).toBeDefined();
    if (!primaryResponder) {
      throw new Error(`Cannot find responder with name: ${primaryName}`);
    }

    const primaryResponderLinkPath = `/ws/webhooks__responders?q=${encodeURIComponent(primaryResponder.id)}`;

    await goto(page, '/ws/webhooks__responders');
    const primaryResponderLink = page.getByRole('link', { name: primaryName, exact: true });
    await expect(primaryResponderLink).toBeVisible({ timeout: 15000 });
    await expect(primaryResponderLink).toHaveAttribute('href', primaryResponderLinkPath);

    await goto(page, primaryResponderLinkPath);
    await expect.poll(() => new URL(page.url()).searchParams.get('q'), { timeout: 15000 }).toBe(primaryResponder.id);
    await expect(page.getByRole('link', { name: primaryName, exact: true })).toBeVisible();
    await expect(page.getByRole('link', { name: secondaryName, exact: true })).not.toBeVisible();
  });
});
