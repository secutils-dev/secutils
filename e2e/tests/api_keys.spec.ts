import type { Locator, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

import { dismissAllToasts, ensureUserAndLogin } from '../helpers';

/** Open the Settings flyout and switch to the Security tab. */
async function openSecuritySettings(page: Page) {
  await page.getByRole('button', { name: 'Account menu' }).click();
  const settingsButton = page.getByText('Settings');
  await expect(settingsButton).toBeVisible();
  await settingsButton.click();

  const securityTab = page.getByRole('tab', { name: 'Security' });
  await expect(securityTab).toBeVisible({ timeout: 15000 });
  await securityTab.click();

  await expect(page.getByRole('button', { name: 'Manage API keys' })).toBeVisible({ timeout: 15000 });
}

/** Open the API Keys modal from an already-visible Security tab. */
async function openApiKeysModal(page: Page) {
  await page.getByRole('button', { name: 'Manage API keys' }).click();
  const modal = page.locator('.euiModal').filter({ has: page.getByText('API keys') });
  await expect(modal).toBeVisible({ timeout: 15000 });
  return modal;
}

/** Close the API Keys modal using the footer Close button. */
async function closeModal(modal: Locator) {
  await modal.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(modal).not.toBeVisible();
}

/** Find the table row containing the given key name. */
function getKeyRow(modal: Locator, keyName: string) {
  return modal.locator('tr', { hasText: keyName });
}

test.describe('API Keys CRUD', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
    await page.goto('/ws/workspace__overview');
    await expect(page.getByRole('heading', { name: 'Welcome', level: 2 })).toBeVisible({ timeout: 15000 });
  });

  test('navigate to Security tab and open API keys modal', async ({ page }) => {
    await openSecuritySettings(page);
    const modal = await openApiKeysModal(page);

    await expect(modal.getByText('No API keys yet.')).toBeVisible({ timeout: 15000 });
    await expect(modal.getByRole('button', { name: 'Create API key' })).toBeVisible();

    await closeModal(modal);
  });

  test('create an API key and see the token', async ({ page }) => {
    await openSecuritySettings(page);
    const modal = await openApiKeysModal(page);

    await modal.getByRole('button', { name: 'Create API key' }).click();

    const nameInput = modal.getByPlaceholder('e.g. CI deployment key');
    await expect(nameInput).toBeVisible({ timeout: 15000 });
    await nameInput.fill('Test Key');
    await modal.getByRole('button', { name: 'Save' }).click();

    // Token reveal callout should be visible.
    await expect(modal.getByText('API key created')).toBeVisible({ timeout: 15000 });
    await expect(modal.getByText('This token will not be shown again.')).toBeVisible();
    await expect(modal.getByText(/su_ak_/)).toBeVisible();

    // The key should appear in the table.
    await expect(modal.getByText('Test Key', { exact: true })).toBeVisible();
    await expect(modal.getByText('Never')).toBeVisible();

    // Dismiss the token reveal.
    await modal.getByRole('button', { name: 'Dismiss' }).click();
    await expect(modal.getByText('API key created')).not.toBeVisible();

    await closeModal(modal);
  });

  test('create an API key with expiration', async ({ page }) => {
    const futureTs = Math.floor(Date.now() / 1000) + 90 * 24 * 60 * 60;
    const createRes = await page.request.post('/api/user/api_keys', {
      data: { name: 'Expiring Key', expiresAt: futureTs },
    });
    expect(createRes.ok()).toBeTruthy();

    await openSecuritySettings(page);
    const modal = await openApiKeysModal(page);

    await expect(modal.getByText('Expiring Key')).toBeVisible({ timeout: 15000 });
    await expect(modal.getByText('Never')).not.toBeVisible();

    await closeModal(modal);
  });

  test('edit (rename) an API key', async ({ page }) => {
    const createRes = await page.request.post('/api/user/api_keys', {
      data: { name: 'Original Name' },
    });
    expect(createRes.ok()).toBeTruthy();

    await openSecuritySettings(page);
    const modal = await openApiKeysModal(page);
    await expect(modal.getByText('Original Name')).toBeVisible({ timeout: 15000 });

    // Edit is a primary action - visible as an icon button on the row.
    const row = getKeyRow(modal, 'Original Name');
    await row.getByRole('button', { name: 'Edit' }).click();

    const editNameInput = modal.locator('input[value="Original Name"]');
    await expect(editNameInput).toBeVisible({ timeout: 15000 });
    await editNameInput.fill('Renamed Key');
    await modal.getByRole('button', { name: 'Save' }).click();

    await expect(page.getByText('API key updated.')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);

    await expect(modal.getByText('Renamed Key')).toBeVisible({ timeout: 15000 });
    await expect(modal.getByText('Original Name')).not.toBeVisible();

    await closeModal(modal);
  });

  test('delete an API key', async ({ page }) => {
    const createRes = await page.request.post('/api/user/api_keys', {
      data: { name: 'To Delete' },
    });
    expect(createRes.ok()).toBeTruthy();

    await openSecuritySettings(page);
    const modal = await openApiKeysModal(page);
    await expect(modal.getByText('To Delete')).toBeVisible({ timeout: 15000 });

    // Delete is a primary action - visible as an icon button on the row.
    const row = getKeyRow(modal, 'To Delete');
    await row.getByRole('button', { name: 'Delete' }).click();

    const confirmModal = page.getByRole('alertdialog');
    await expect(confirmModal).toBeVisible({ timeout: 15000 });
    await expect(confirmModal.getByText('This action cannot be undone.')).toBeVisible();
    await confirmModal.getByRole('button', { name: 'Delete' }).click();

    await expect(page.getByText('API key "To Delete" deleted.')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);

    await expect(modal.getByText('No API keys yet.')).toBeVisible({ timeout: 15000 });

    await closeModal(modal);
  });

  test('regenerate an API key', async ({ page }) => {
    const createRes = await page.request.post('/api/user/api_keys', {
      data: { name: 'Regen Key' },
    });
    expect(createRes.ok()).toBeTruthy();

    await openSecuritySettings(page);
    const modal = await openApiKeysModal(page);
    await expect(modal.getByText('Regen Key')).toBeVisible({ timeout: 15000 });

    // Regenerate is a non-primary action - hidden in the "All actions" dropdown.
    const row = getKeyRow(modal, 'Regen Key');
    await row.getByRole('button', { name: /All actions/ }).click();
    await page.getByText('Regenerate', { exact: true }).click();

    const confirmModal = page.getByRole('alertdialog');
    await expect(confirmModal).toBeVisible({ timeout: 15000 });
    await expect(confirmModal.getByText('immediately invalidated')).toBeVisible();
    await confirmModal.getByRole('button', { name: 'Regenerate' }).click();

    // Token reveal should show with "regenerated" flavor.
    await expect(modal.getByText('API key regenerated')).toBeVisible({ timeout: 15000 });
    await expect(modal.getByText(/su_ak_/)).toBeVisible();

    await modal.getByRole('button', { name: 'Dismiss' }).click();
    await closeModal(modal);
  });

  test('duplicate name rejection', async ({ page }) => {
    await page.request.post('/api/user/api_keys', {
      data: { name: 'Unique Key' },
    });

    await openSecuritySettings(page);
    const modal = await openApiKeysModal(page);
    await expect(modal.getByText('Unique Key')).toBeVisible({ timeout: 15000 });

    await modal.getByRole('button', { name: 'Create API key' }).click();
    const nameInput = modal.getByPlaceholder('e.g. CI deployment key');
    await expect(nameInput).toBeVisible({ timeout: 15000 });
    await nameInput.fill('Unique Key');
    await modal.getByRole('button', { name: 'Save' }).click();

    await expect(page.getByText(/already exists/i)).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);

    await modal.getByRole('button', { name: 'Cancel' }).click();
    await closeModal(modal);
  });

  test('copy token to clipboard', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);

    await openSecuritySettings(page);
    const modal = await openApiKeysModal(page);

    await modal.getByRole('button', { name: 'Create API key' }).click();
    const nameInput = modal.getByPlaceholder('e.g. CI deployment key');
    await expect(nameInput).toBeVisible({ timeout: 15000 });
    await nameInput.fill('Copy Test Key');
    await modal.getByRole('button', { name: 'Save' }).click();

    await expect(modal.getByText('API key created')).toBeVisible({ timeout: 15000 });

    await modal.getByRole('button', { name: 'Copy' }).click();

    const clipboardText = await page.evaluate(() => navigator.clipboard.readText());
    expect(clipboardText).toMatch(/^su_ak_/);

    await closeModal(modal);
  });

  test('verify API keys via backend', async ({ page }) => {
    const res1 = await page.request.post('/api/user/api_keys', { data: { name: 'Backend Key 1' } });
    expect(res1.ok()).toBeTruthy();
    const res2 = await page.request.post('/api/user/api_keys', { data: { name: 'Backend Key 2' } });
    expect(res2.ok()).toBeTruthy();

    const listRes = await page.request.get('/api/user/api_keys');
    expect(listRes.ok()).toBeTruthy();
    const keys = await listRes.json();
    expect(keys).toHaveLength(2);

    const names = keys.map((k: { name: string }) => k.name).sort();
    expect(names).toEqual(['Backend Key 1', 'Backend Key 2']);

    for (const key of keys) {
      expect(key).toHaveProperty('id');
      expect(key).toHaveProperty('createdAt');
      expect(key).toHaveProperty('updatedAt');
      expect(key.lastUsedAt).toBeNull();
      expect(key).not.toHaveProperty('token');
    }
  });

  test('use API key to authenticate against utility APIs', async ({ page, request }) => {
    const createRes = await page.request.post('/api/user/api_keys', {
      data: { name: 'Auth Test Key' },
    });
    expect(createRes.ok()).toBeTruthy();
    const { token } = await createRes.json();
    expect(token).toMatch(/^su_ak_/);

    // Use the token to call /api/ui/state (user/self equivalent).
    const stateRes = await request.get('/api/ui/state', {
      headers: { Authorization: `Bearer ${token}` },
    });
    expect(stateRes.ok()).toBeTruthy();
    const state = await stateRes.json();
    expect(state).toHaveProperty('user');
    expect(state.user).toHaveProperty('email');

    // Use the token to list user secrets.
    const secretsRes = await request.get('/api/user/secrets', {
      headers: { Authorization: `Bearer ${token}` },
    });
    expect(secretsRes.ok()).toBeTruthy();
    const secrets = await secretsRes.json();
    expect(Array.isArray(secrets)).toBeTruthy();

    // Use the token to list user tags.
    const tagsRes = await request.get('/api/user/tags', {
      headers: { Authorization: `Bearer ${token}` },
    });
    expect(tagsRes.ok()).toBeTruthy();
    const tags = await tagsRes.json();
    expect(Array.isArray(tags)).toBeTruthy();

    // Verify the token CANNOT manage API keys (403).
    const selfKeysRes = await request.get('/api/user/api_keys', {
      headers: { Authorization: `Bearer ${token}` },
    });
    expect(selfKeysRes.status()).toBe(403);
  });
});
