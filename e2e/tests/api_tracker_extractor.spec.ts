import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

const API_URL = 'http://host.docker.internal:7171/api/ui/state';
const SECRET_NAME = 'E2E_API_KEY';
const SECRET_VALUE = 'e2e-secret-42';

// IIFE extractor that proves the full pipeline:
//  1. Reads and parses the HTTP response body
//  2. Reads the secret from context.params.secrets
//  3. Returns a combined JSON with evidence of both
const EXTRACTOR_SCRIPT = `(() => {
  const resp = context.responses?.[0];
  const raw = resp?.body ? Deno.core.decode(new Uint8Array(resp.body)) : "{}";
  const parsed = JSON.parse(raw);
  const secret = context.params?.secrets?.${SECRET_NAME} ?? "NO_SECRET";
  return {
    body: Deno.core.encode(JSON.stringify({
      extracted: true,
      secret: secret,
      status: resp?.status ?? null,
      keys: Object.keys(parsed).sort()
    }, null, 2))
  };
})();`;

test.describe.serial('API Tracker Extractor with Secrets', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('custom extractor script transforms response and injects secret', async ({ page }) => {
    // 1. Create a user secret via API.
    const secretRes = await page.request.post('/api/user/secrets', {
      data: { name: SECRET_NAME, value: SECRET_VALUE },
    });
    expect(secretRes.ok()).toBeTruthy();

    // 2. Create an API tracker with a custom extractor and "All secrets" access.
    const createRes = await page.request.post('/api/utils/web_scraping/api', {
      data: {
        name: 'Extractor E2E',
        config: { revisions: 3 },
        target: { url: API_URL, extractor: EXTRACTOR_SCRIPT },
        secrets: { type: 'all' },
      },
    });
    expect(createRes.ok()).toBeTruthy();

    // 3. Navigate to API trackers and expand the tracker's history.
    await page.goto('/ws/web_scraping__api');
    const trackerRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Extractor E2E' }) });
    await expect(trackerRow).toBeVisible({ timeout: 15000 });

    await trackerRow.getByRole('button', { name: 'Show history' }).click();
    const updateButton = page.getByRole('button', { name: 'Update', exact: true });
    await expect(updateButton).toBeVisible({ timeout: 10000 });

    // 4. Click Update to trigger Retrack: HTTP request → extractor script → store result.
    await updateButton.click();

    // 5. Verify the extractor output is visible in the UI.
    // The script returns JSON with "extracted": true and the secret value.
    await expect(page.getByText(SECRET_VALUE)).toBeVisible({ timeout: 60000 });
    await expect(page.getByText('"extracted"')).toBeVisible({ timeout: 5000 });
  });
});
