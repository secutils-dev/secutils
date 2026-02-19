import { expect, test } from '@playwright/test';

const EMAIL = 'e2e@secutils.dev';
const PASSWORD = 'e2e_secutils_pass';

// 10-year operator JWT for @secutils, generated with:
// cargo run -p secutils-jwt-tools -- generate --secret <JWT_SECRET> --sub @secutils --exp 10years
const OPERATOR_TOKEN =
  'eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJleHAiOjIwODcxMDY2MDQsInN1YiI6IkBzZWN1dGlscyJ9.7UT-E9YkTqTiktTtZal6wbjsgB8PTjmdATxNaQPG9zs';

test.describe('User Registration and Login Flow', () => {
  test.beforeEach(async ({ request }) => {
    await request.post('/api/users/remove', {
      headers: { Authorization: `Bearer ${OPERATOR_TOKEN}` },
      data: { email: EMAIL },
    });
  });

  test('complete registration, logout, and re-login', async ({ page }) => {
    await page.goto('/');

    // Navigate to registration.
    const createAccountButton = page.getByRole('button', { name: 'Create account' });
    await expect(createAccountButton).toBeVisible({ timeout: 15000 });
    await createAccountButton.click();
    await expect(page).toHaveURL(/signup/);

    // Fill in the email and proceed to the password step.
    const emailInput = page.getByPlaceholder('Email');
    await expect(emailInput).toBeVisible({ timeout: 15000 });
    await emailInput.fill(EMAIL);
    await page.getByRole('button', { name: 'Continue with password' }).click();

    // Fill in password fields and submit.
    const passwordInput = page.getByPlaceholder('Password', { exact: true });
    const repeatPasswordInput = page.getByPlaceholder('Repeat password');
    await expect(passwordInput).toBeVisible({ timeout: 15000 });
    await expect(repeatPasswordInput).toBeVisible({ timeout: 15000 });
    await passwordInput.fill(PASSWORD);
    await repeatPasswordInput.fill(PASSWORD);
    await page.getByRole('button', { name: 'Sign up', exact: true }).click();

    // Wait for the workspace page with the Welcome heading.
    await expect(page).toHaveURL(/\/ws/, { timeout: 30000 });
    await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });

    // Verify the authenticated UI state includes user info.
    const stateResponse = await page.request.get('/api/ui/state');
    expect(stateResponse.ok()).toBeTruthy();
    const state = await stateResponse.json();
    expect(state.user).toBeDefined();
    expect(state.user.email).toBe(EMAIL);
    expect(state.user.isActivated).toBe(false);
    expect(state).toHaveProperty('subscription');

    // Log out via the Account menu.
    await page.getByRole('button', { name: 'Account menu' }).click();
    const signOutButton = page.getByText('Sign out');
    await expect(signOutButton).toBeVisible();
    await signOutButton.click();

    // Should be redirected back to the sign-in page.
    await expect(page).toHaveURL(/\/signin/, { timeout: 15000 });

    // Re-login with the newly created account.
    const loginEmailInput = page.getByPlaceholder('Email');
    await expect(loginEmailInput).toBeVisible({ timeout: 15000 });
    await loginEmailInput.fill(EMAIL);
    await page.getByPlaceholder('Password', { exact: true }).fill(PASSWORD);
    await page.getByRole('button', { name: 'Sign in', exact: true }).click();

    // Verify the Welcome page loads again after re-login.
    await expect(page).toHaveURL(/\/ws/, { timeout: 30000 });
    await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });
  });
});
