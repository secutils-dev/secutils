import { resolve } from 'path';

import type { APIRequestContext, Locator, Page } from '@playwright/test';
import { expect } from '@playwright/test';

export const DOCS_IMG_DIR = resolve(__dirname, '../../components/secutils-docs/static/img/docs/guides');

export const EMAIL = 'e2e@secutils.dev';
export const PASSWORD = 'e2e_secutils_pass';

// 10-year operator JWT for @secutils, generated with:
// cargo run -p secutils-jwt-tools -- generate --secret <JWT_SECRET> --sub @secutils --exp 10years
export const OPERATOR_TOKEN =
  'eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJleHAiOjIwODcxMDY2MDQsInN1YiI6IkBzZWN1dGlscyJ9.7UT-E9YkTqTiktTtZal6wbjsgB8PTjmdATxNaQPG9zs';

export async function ensureUserAndLogin(request: APIRequestContext, page: Page): Promise<void> {
  await page.context().clearCookies();
  await request.post('/api/users/remove', {
    headers: { Authorization: `Bearer ${OPERATOR_TOKEN}` },
    data: { email: EMAIL },
  });

  await goto(page, '/');

  const createAccountButton = page.getByRole('button', { name: 'Create account' });
  await expect(createAccountButton).toBeVisible({ timeout: 15000 });
  await createAccountButton.click();
  await expect(page).toHaveURL(/signup/);

  const emailInput = page.getByPlaceholder('Email');
  const continueButton = page.getByRole('button', { name: 'Continue with password' });
  await expect(emailInput).toBeVisible({ timeout: 15000 });
  // Use pressSequentially so React's synthetic event system tracks each keystroke and
  // properly updates form state. fill() sets the DOM value directly but can leave React's
  // internal state stale. The poll checks the button-enabled state (not just the DOM value)
  // so it retries the whole fill cycle if React resets the field before settling.
  await expect
    .poll(
      async () => {
        await emailInput.fill('');
        await emailInput.pressSequentially(EMAIL);
        return continueButton.isEnabled();
      },
      { timeout: 15000 },
    )
    .toBeTruthy();

  await continueButton.click();

  const passwordInput = page.getByPlaceholder('Password', { exact: true });
  const repeatPasswordInput = page.getByPlaceholder('Repeat password');
  await expect(passwordInput).toBeVisible({ timeout: 15000 });
  await passwordInput.fill(PASSWORD);
  await repeatPasswordInput.fill(PASSWORD);
  await page.getByRole('button', { name: 'Sign up', exact: true }).click();

  await expect(page).toHaveURL(/\/ws/, { timeout: 30000 });
  await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });
}

export async function goto(page: Page, url: string) {
  await page.goto(url);
  await page.addStyleTag({
    content: '*, *::before, *::after { animation-duration: 0s !important; transition-duration: 0s !important; }',
  });
}

export async function highlightOn(locator: Locator) {
  await locator.evaluate((el) => {
    el.style.outline = '3px dashed red';
    el.style.outlineOffset = '3px';
    el.style.borderRadius = '5px';
  });
}

export async function dismissAllToasts(page: Page) {
  const toasts = page.getByRole('button', { name: 'Dismiss toast' });
  for (const toast of await toasts.all()) {
    await toast.click();
  }
}

/**
 * Intercept responder history API responses and replace dynamic `createdAt` and
 * `clientAddress` fields with fixed values so screenshots are stable.
 */
export async function fixResponderRequestFields(page: Page) {
  await page.route('**/api/utils/webhooks/responders/*/history', async (route) => {
    const response = await route.fetch();
    const json = await response.json();
    for (const req of json) {
      req.createdAt = 1740000000;
      req.clientAddress = '172.18.0.1:12345';
    }
    await route.fulfill({ response, json });
  });
}

/**
 * Intercept certificate template API responses and pin the `notValidBefore` /
 * `notValidAfter` timestamps to fixed dates while preserving their distance.
 */
export async function fixCertificateTemplateValidityDates(page: Page) {
  const FIXED_NOT_VALID_BEFORE = 1735689600; // Jan 1, 2025 00:00:00 UTC
  await page.route('**/api/utils/certificates/templates**', async (route) => {
    const response = await route.fetch();
    const json = await response.json();
    const isArray = Array.isArray(json);
    const templates = isArray ? json : [json];
    for (const tpl of templates) {
      if (tpl.attributes?.notValidBefore != null && tpl.attributes?.notValidAfter != null) {
        const diff = tpl.attributes.notValidAfter - tpl.attributes.notValidBefore;
        tpl.attributes.notValidBefore = FIXED_NOT_VALID_BEFORE;
        tpl.attributes.notValidAfter = FIXED_NOT_VALID_BEFORE + diff;
      }
    }
    await route.fulfill({ response, json: isArray ? templates : templates[0] });
  });
}

/**
 * Intercept page tracker revision history responses and stabilize dynamic parts
 * (URLs, sizes, timestamps) so screenshots remain consistent across runs.
 */
export async function fixTrackerResourceRevisions(page: Page) {
  const FIXED_TIMESTAMP = 1735689600; // Jan 1, 2025 00:00:00 UTC
  await page.route('**/api/utils/web_scraping/page/*/history', async (route) => {
    const response = await route.fetch();
    const json = await response.json();
    if (!Array.isArray(json)) {
      await route.fulfill({ response, json });
      return;
    }
    for (const rev of json) {
      rev.createdAt = FIXED_TIMESTAMP;
      const original = rev.data?.original;
      if (!original?.rows) {
        continue;
      }

      for (const row of original.rows) {
        row.source = stabilizeResourceUrl(row.source);
        row.size = String(stableResourceSize(row.source));
      }

      for (const type of ['scripts', 'styles']) {
        for (const resource of original.source?.[type] ?? []) {
          resource.url = stabilizeResourceUrl(resource.url);
          if (resource.content) {
            resource.content.size = stableResourceSize(resource.url);
          }
        }
      }
    }
    await route.fulfill({ response, json });
  });
}

function stabilizeResourceUrl(url: string): string {
  if (!url) return url;
  return url.replace(/\?[^?]*$/, '').replace(/https:\/\/[^.]+\.webhooks\./, 'https://preview.webhooks.');
}

function stableResourceSize(url: string): number {
  let hash = 0;
  for (let i = 0; i < url.length; i++) {
    hash = ((hash << 5) - hash + url.charCodeAt(i)) | 0;
  }
  return (Math.abs(hash) % 9000) + 1000;
}

export async function highlightOff(locator: Locator) {
  await locator.evaluate((el) => {
    el.style.outline = 'unset';
    el.style.outlineOffset = 'unset';
    el.style.borderRadius = 'unset';
  });
}
