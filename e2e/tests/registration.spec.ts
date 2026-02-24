import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

test.describe('User Registration and Login Flow', () => {
  test('complete registration, logout, and re-login', async ({ request, page }) => {
    const { email, password } = await ensureUserAndLogin(request, page);

    // Verify the authenticated UI state includes the registered email.
    const stateResponse = await page.request.get('/api/ui/state');
    expect(stateResponse.ok()).toBeTruthy();
    const state = await stateResponse.json();
    expect(state.user).toBeDefined();
    expect(state.user.email).toBe(email);
    expect(state.user.isActivated).toBe(false);
    expect(state).toHaveProperty('subscription');

    // Log out via the Account menu.
    await page.getByRole('button', { name: 'Account menu' }).click();
    const signOutButton = page.getByText('Sign out');
    await expect(signOutButton).toBeVisible();
    await signOutButton.click();

    // Should be redirected back to the sign-in page.
    await expect(page).toHaveURL(/\/signin/, { timeout: 15000 });

    // Re-login with the same credentials.
    const loginEmailInput = page.getByPlaceholder('Email');
    await expect(loginEmailInput).toBeVisible({ timeout: 15000 });
    await loginEmailInput.fill(email);
    await page.getByPlaceholder('Password', { exact: true }).fill(password);
    await page.getByRole('button', { name: 'Sign in', exact: true }).click();

    // Verify the Welcome page loads again after re-login.
    await expect(page).toHaveURL(/\/ws/, { timeout: 30000 });
    await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });
  });
});
