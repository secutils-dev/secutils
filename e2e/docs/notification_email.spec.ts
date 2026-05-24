import { join } from 'path';

import { expect, test } from '@playwright/test';

import { DOCS_IMG_DIR, EMAIL, ensureUserAndLogin, goto, highlightOn, PASSWORD } from '../helpers';

const IMG_DIR = join(DOCS_IMG_DIR, 'notification_email');

const NOTIFICATION_ADDRESS = 'alerts@example.com';

// A pinned UNIX timestamp used for `verifiedAt` / `createdAt` in mocked responses so the
// rendered "Verified" badge surface stays byte-stable across runs. Same value as
// FIXED_ENTITY_TIMESTAMP in the screenshot helpers.
const FIXED_TS = 1740000000;
// Far-future pinned expiry used for mocked pending rows so the UI's
// `isVerificationPending` check (`verificationExpiresAt * 1000 > Date.now()`) is always
// true regardless of when the test runs. 2099-01-01 in unix seconds.
const FIXED_FUTURE_TS = 4070908800;

test.describe('Notification email guide screenshots', () => {
  test.beforeEach(async ({ page, request }) => {
    await ensureUserAndLogin(request, page, { email: EMAIL, password: PASSWORD });
    await page.request.delete('/api/user/notification_email');
  });

  test('configure a notification email', async ({ page }) => {
    // Step 1: Open Settings -> Account and show the empty Notification email section.
    await goto(page, '/ws/workspace__overview');
    await page.getByRole('button', { name: 'Account menu' }).click();
    const settingsButton = page.getByText('Settings');
    await expect(settingsButton).toBeVisible({ timeout: 15000 });
    await settingsButton.click();

    const accountTab = page.getByRole('tab', { name: 'Account' });
    await expect(accountTab).toBeVisible({ timeout: 15000 });

    // Settings opens as an EuiFlyout (`role="dialog"`). EuiFlyout does not propagate its
    // title as an accessible name, so we identify it the same way other docs specs do for
    // EUI dialogs: by filtering on the visible heading inside it. EuiDescribedFormGroup
    // itself has no implicit ARIA role, so once scoped we rely on the unique inner controls
    // (placeholder, button labels) to pin the right widgets.
    const settingsFlyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Settings' }) });
    await expect(settingsFlyout).toBeVisible({ timeout: 15000 });
    await expect(settingsFlyout.getByRole('heading', { name: 'Notification email', level: 3 })).toBeVisible({
      timeout: 15000,
    });

    const sendCodeButton = settingsFlyout.getByRole('button', { name: 'Send verification code' });
    await highlightOn(sendCodeButton);
    await page.screenshot({ path: join(IMG_DIR, 'notification_email_step1_empty.png') });

    // Step 2: Fill in an address. The Notification email input uses the login email as
    // placeholder, while the Account email input has it as value, so `getByPlaceholder` is
    // sufficient to disambiguate.
    const addressInput = settingsFlyout.getByPlaceholder(EMAIL);
    await addressInput.fill(NOTIFICATION_ADDRESS);
    await page.screenshot({ path: join(IMG_DIR, 'notification_email_step2_address_filled.png') });

    // Step 3: Mock the PUT response so the screenshot pipeline never depends on a working
    // outbound SMTP path; we only document the UI states here.
    await page.route('**/api/user/notification_email', async (route) => {
      if (route.request().method() !== 'PUT') {
        await route.continue();
        return;
      }
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          id: '00000000-0000-0000-0000-000000000001',
          kind: 'email',
          address: NOTIFICATION_ADDRESS,
          verificationExpiresAt: FIXED_FUTURE_TS,
          verificationSentAt: FIXED_TS,
          createdAt: FIXED_TS,
          updatedAt: FIXED_TS,
        }),
      });
    });
    await sendCodeButton.click();

    const codeInput = settingsFlyout.getByPlaceholder('123456');
    await expect(codeInput).toBeVisible({ timeout: 15000 });
    const verifyButton = settingsFlyout.getByRole('button', { name: 'Verify' });
    await highlightOn(verifyButton);
    await page.screenshot({ path: join(IMG_DIR, 'notification_email_step3_pending.png') });

    // Step 4: Mock the verification flip and show the verified state.
    await codeInput.fill('123456');
    await page.route('**/api/user/notification_email/_verify', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          id: '00000000-0000-0000-0000-000000000001',
          kind: 'email',
          address: NOTIFICATION_ADDRESS,
          verifiedAt: FIXED_TS,
          createdAt: FIXED_TS,
          updatedAt: FIXED_TS,
        }),
      });
    });
    await page.route('**/api/ui/state', async (route) => {
      const response = await route.fetch();
      const json = await response.json();
      json.notificationEmail = {
        id: '00000000-0000-0000-0000-000000000001',
        kind: 'email',
        address: NOTIFICATION_ADDRESS,
        verifiedAt: FIXED_TS,
        createdAt: FIXED_TS,
        updatedAt: FIXED_TS,
      };
      await route.fulfill({ response, json });
    });
    await verifyButton.click();

    const changeButton = settingsFlyout.getByRole('button', { name: 'Change' });
    await expect(changeButton).toBeVisible({ timeout: 15000 });
    await highlightOn(changeButton);
    await page.screenshot({ path: join(IMG_DIR, 'notification_email_step4_verified.png') });
  });
});
