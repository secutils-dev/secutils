import type { Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

import { goto, OPERATOR_TOKEN } from '../helpers';

// Secutils supports passwordless passkeys via Kratos (`webauthn = { passwordless = true, rp.id = "localhost" }`).
// These tests drive the real "Sign up with passkey" / "Sign in with passkey" UI flows, which call
// `navigator.credentials.create()` / `navigator.credentials.get()`. Playwright 1.61's virtual WebAuthn authenticator
// (`browserContext.credentials`) answers those ceremonies without any real hardware key.
//
// See https://playwright.dev/docs/release-notes#-webauthn-passkeys

// Relying party id, must match Kratos' `selfservice.methods.webauthn.config.rp.id`.
const RP_ID = 'localhost';

function randomEmail(): string {
  const id = Math.random().toString(36).slice(2, 10);
  return `e2e-passkey-${id}@secutils.dev`;
}

async function signupWithPasskey(page: Page, email: string) {
  await goto(page, '/signup');
  await page.getByPlaceholder('Email').fill(email);
  await page.getByRole('button', { name: 'Sign up with passkey' }).click();

  await expect(page).toHaveURL(/\/ws/, { timeout: 30000 });
  await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });
}

test.describe.serial('Passkey signup and login', () => {
  let email: string;

  test.beforeEach(async ({ request, page }) => {
    test.setTimeout(60_000);
    email = randomEmail();
    // Start each test from a clean slate (no account, no session).
    await page.context().clearCookies();
    await request.post('/api/users/remove', {
      headers: { Authorization: `Bearer ${OPERATOR_TOKEN}` },
      data: { email },
    });
  });

  test('registers with a passkey, then signs out and logs back in with the passkey', async ({ page }) => {
    // Install the virtual authenticator before the page touches `navigator.credentials`.
    await page.context().credentials.install();

    // 1. Register a brand-new account using a passkey. The app call navigator.credentials.create(), answered by the
    // virtual authenticator which creates a discoverable (resident) credential.
    await signupWithPasskey(page, email);

    // The signup web_hook provisions the Secutils user, verify the authenticated state.
    const stateResponse = await page.request.get('/api/ui/state');
    expect(stateResponse.ok()).toBeTruthy();
    const state = await stateResponse.json();
    expect(state.user?.email).toBe(email);

    // The authenticator should now hold exactly one passkey for this relying party.
    const heldCredentials = await page.context().credentials.get({ rpId: RP_ID });
    expect(heldCredentials.length).toBe(1);

    // 2. Sign out via the Account menu.
    await page.getByRole('button', { name: 'Account menu' }).click();
    const signOutButton = page.getByText('Sign out');
    await expect(signOutButton).toBeVisible();
    await signOutButton.click();
    await expect(page).toHaveURL(/\/signin/, { timeout: 15000 });

    // 3. Log back in with the passkey. The app calls navigator.credentials.get(), which the virtual authenticator
    // resolves from the held credential.
    const loginEmailInput = page.getByPlaceholder('Email');
    await expect(loginEmailInput).toBeVisible({ timeout: 15000 });
    await loginEmailInput.fill(email);
    await page.getByRole('button', { name: 'Sign in with passkey' }).click();

    await expect(page).toHaveURL(/\/ws/, { timeout: 30000 });
    await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });
  });

  test('captures a registered passkey and re-seeds it into a fresh context to log in', async ({
    page,
    browser,
    baseURL,
  }) => {
    // Register in the first context, capturing the passkey the page creates.
    await page.context().credentials.install();
    await signupWithPasskey(page, email);

    const [captured] = await page.context().credentials.get({ rpId: RP_ID });
    expect(captured).toBeTruthy();

    // A fresh context with no session: seed the captured passkey so the app starts already enrolled,
    // then log in with it.
    const seededContext = await browser.newContext({ baseURL });
    try {
      await seededContext.credentials.create(captured.rpId, captured);
      await seededContext.credentials.install();

      const seededPage = await seededContext.newPage();
      await seededPage.goto('/signin');
      await seededPage.getByPlaceholder('Email').fill(email);
      await seededPage.getByRole('button', { name: 'Sign in with passkey' }).click();

      await expect(seededPage).toHaveURL(/\/ws/, { timeout: 30000 });
      await expect(seededPage.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });
    } finally {
      await seededContext.close();
    }
  });
});
