import { expect, test } from '@playwright/test';

import { ensureUserAndLogin, goto } from '../helpers';

test.describe('Sidebar section collapse persistence', () => {
  test.beforeEach(async ({ page, request }) => {
    await ensureUserAndLogin(request, page);
  });

  test('collapsed section state persists across page navigations', async ({ page }) => {
    await goto(page, '/ws');
    await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });

    const sidebar = page.locator('aside');

    // Verify "Webhooks" section is expanded by default (Responders link is visible).
    const respondersLink = sidebar.getByRole('link', { name: 'Responders', exact: true });
    await expect(respondersLink).toBeVisible({ timeout: 15000 });

    // Collapse the "Webhooks" section.
    const webhooksButton = sidebar.getByRole('button', { name: 'Webhooks', exact: true });
    await webhooksButton.click();

    // Responders link should now be hidden.
    await expect(respondersLink).not.toBeVisible();

    // Navigate to a different page (e.g. CSP) and back to workspace overview.
    const cspLink = sidebar.getByRole('link', { name: 'CSP', exact: true });
    if (!(await cspLink.isVisible())) {
      const webSecButton = sidebar.getByRole('button', { name: 'Web Security', exact: true });
      if (await webSecButton.isVisible()) {
        await webSecButton.click();
      }
    }
    await cspLink.click();
    await expect(page).toHaveURL(/web_security__csp/, { timeout: 15000 });

    // Navigate back to overview.
    await goto(page, '/ws');
    await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });

    // "Webhooks" section should still be collapsed.
    await expect(respondersLink).not.toBeVisible();

    // Expand it again.
    await sidebar.getByRole('button', { name: 'Webhooks', exact: true }).click();
    await expect(respondersLink).toBeVisible();
  });

  test('collapsed section state persists across full page reloads', async ({ page }) => {
    await goto(page, '/ws');
    await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });

    const sidebar = page.locator('aside');
    const respondersLink = sidebar.getByRole('link', { name: 'Responders', exact: true });
    await expect(respondersLink).toBeVisible({ timeout: 15000 });

    // Collapse "Webhooks".
    await sidebar.getByRole('button', { name: 'Webhooks', exact: true }).click();
    await expect(respondersLink).not.toBeVisible();

    // Wait for settings to be saved, then do a full page reload.
    await page.waitForTimeout(1000);
    await goto(page, '/ws');
    await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });

    // "Webhooks" should still be collapsed after the reload.
    await expect(respondersLink).not.toBeVisible();
  });

  test('collapsed state is reflected in user settings API', async ({ page }) => {
    await goto(page, '/ws');
    await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });

    const sidebar = page.locator('aside');

    // Collapse "Digital Certificates" section.
    const certsButton = sidebar.getByRole('button', { name: 'Digital Certificates', exact: true });
    await expect(certsButton).toBeVisible({ timeout: 15000 });
    await certsButton.click();

    // Wait for the setting to propagate.
    await page.waitForTimeout(1000);

    // Verify via API that the setting was saved.
    const settingsResponse = await page.request.get('/api/user/settings');
    expect(settingsResponse.ok()).toBeTruthy();
    const settings = await settingsResponse.json();
    const sidebarCollapsed = settings?.['common.sidebarCollapsed'];
    expect(sidebarCollapsed).toBeDefined();
    expect(sidebarCollapsed).toHaveProperty('sections');
    expect(sidebarCollapsed.sections).toContain('certificates');
  });
});
