import { resolve } from 'path';

import { expect, test } from '@playwright/test';

import { EMAIL, ensureUserAndLogin, goto, PASSWORD } from '../helpers';

const IMG_DIR = resolve(__dirname, '../../components/secutils-docs/static/img/docs/home');

test.describe('Home page screenshots', () => {
  test.beforeEach(async ({ page, request }) => {
    await ensureUserAndLogin(request, page, { email: EMAIL, password: PASSWORD });
  });

  test('workspace hub with active and unexplored tools', async ({ page }) => {
    // Create items in two tool categories so the screenshot shows a mix of
    // "active" (with count badges) and "unexplored" cards.

    // Webhooks: create two responders.
    for (const [name, path] of [
      ['API Mock', '/api-mock'],
      ['Honeypot', '/honeypot'],
    ]) {
      const res = await page.request.post('/api/utils/webhooks/responders', {
        data: {
          name,
          location: { pathType: '=', path },
          method: 'ANY',
          enabled: true,
          settings: { requestsToTrack: 10, statusCode: 200, body: 'ok' },
        },
      });
      expect(res.ok()).toBeTruthy();
    }

    // CSP: create one policy.
    const cspRes = await page.request.post('/api/utils/web_security/csp', {
      data: {
        name: 'secutils.dev',
        content: { type: 'directives', value: [{ name: 'default-src', value: ["'self'"] }] },
      },
    });
    expect(cspRes.ok()).toBeTruthy();

    // Navigate to the home page and wait for it to fully render.
    await goto(page, '/ws');
    await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });

    // Wait for data to load - progress text confirms the fetch completed.
    await expect(page.getByText('using 2 of 4 tools')).toBeVisible({ timeout: 15000 });

    // Verify badges are visible before capturing.
    await expect(page.getByText('2 items')).toBeVisible();
    await expect(page.getByText('1 item')).toBeVisible();

    // Verify the checklist and recent items are rendered.
    await expect(page.getByRole('heading', { name: 'Get started', level: 3 })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Recent items', level: 3 })).toBeVisible();

    await page.screenshot({ path: `${IMG_DIR}/workspace_hub.png` });
  });
});
