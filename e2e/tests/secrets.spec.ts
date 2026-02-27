import { expect, test } from '@playwright/test';

import { dismissAllToasts, ensureUserAndLogin, OPERATOR_TOKEN } from '../helpers';

test.describe('User Secrets CRUD', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('create, list, update, and delete a secret', async ({ page }) => {
    // Open settings flyout and go to Secrets tab.
    await page.getByRole('button', { name: 'Account menu' }).click();
    const settingsButton = page.getByText('Settings');
    await expect(settingsButton).toBeVisible();
    await settingsButton.click();

    const secretsTab = page.getByRole('tab', { name: 'Secrets' });
    await expect(secretsTab).toBeVisible({ timeout: 15000 });
    await secretsTab.click();

    // Empty state.
    await expect(page.getByText('No secrets yet')).toBeVisible({ timeout: 15000 });

    // Create a secret.
    await page.getByRole('button', { name: 'Add secret' }).click();
    const modal = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add secret' }) });
    await expect(modal).toBeVisible({ timeout: 15000 });
    await modal.getByPlaceholder('MY_API_KEY').fill('TEST_SECRET');
    await modal.getByPlaceholder('Enter secret value…').fill('super-secret-value');
    await modal.getByRole('button', { name: 'Create' }).click();

    // Verify secret appears in the table.
    await expect(page.getByText('TEST_SECRET', { exact: true })).toBeVisible({ timeout: 15000 });

    // Verify via API that the list includes the secret but not the value.
    const listResponse = await page.request.get('/api/user/secrets');
    expect(listResponse.ok()).toBeTruthy();
    const secrets = await listResponse.json();
    expect(secrets).toHaveLength(1);
    expect(secrets[0].name).toBe('TEST_SECRET');
    expect(secrets[0]).not.toHaveProperty('value');

    // Update the secret.
    await page.getByRole('button', { name: 'Edit' }).click();
    const editModal = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Update secret' }) });
    await expect(editModal).toBeVisible({ timeout: 15000 });
    await editModal.getByPlaceholder('Enter secret value…').fill('updated-value');
    await editModal.getByRole('button', { name: 'Update' }).click();

    // Verify toast appears.
    await expect(page.getByText('Secret "TEST_SECRET" updated')).toBeVisible({ timeout: 15000 });

    // Delete the secret.
    await page.getByRole('button', { name: 'Delete' }).click();
    const confirmModal = page.getByRole('alertdialog');
    await expect(confirmModal).toBeVisible({ timeout: 15000 });
    await confirmModal.getByRole('button', { name: 'Delete' }).click();

    // Verify empty state again.
    await expect(page.getByText('No secrets yet')).toBeVisible({ timeout: 15000 });
  });
});

test.describe('Secrets access in tracker edit flyout', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('changing secrets mode to "All" enables Save and persists', async ({ page }) => {
    // Create a tracker via API (no secrets).
    const createRes = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'secrets-tracker',
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
      },
    });
    expect(createRes.ok()).toBeTruthy();

    await page.goto('/ws/web_scraping__page');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'secrets-tracker' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout).toBeVisible();

    // Save should be disabled initially (no changes).
    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await expect(saveButton).toBeDisabled();

    // Scroll to the Secrets section and change mode.
    const accessModeSelect = flyout.getByLabel('Access mode');
    await accessModeSelect.scrollIntoViewIfNeeded();
    await accessModeSelect.selectOption('all');

    // Save should now be enabled.
    await expect(saveButton).toBeEnabled();
    await saveButton.click();

    await expect(page.getByText('Successfully updated')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);

    // Verify via API that secrets were persisted.
    const listRes = await page.request.get('/api/utils/web_scraping/page');
    expect(listRes.ok()).toBeTruthy();
    const trackers = await listRes.json();
    const tracker = trackers.find((t: { name: string }) => t.name === 'secrets-tracker');
    expect(tracker).toBeDefined();
    expect(tracker.secrets).toEqual({ type: 'all' });
  });

  test('changing secrets mode from "All" back to "None" persists correctly', async ({ page }) => {
    // Create a tracker with "All" secrets via API.
    const createRes = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'secrets-tracker',
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
        secrets: { type: 'all' },
      },
    });
    expect(createRes.ok()).toBeTruthy();

    await page.goto('/ws/web_scraping__page');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'secrets-tracker' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout).toBeVisible();

    // Verify it opens with "All" selected.
    const accessModeSelect = flyout.getByLabel('Access mode');
    await accessModeSelect.scrollIntoViewIfNeeded();
    await expect(accessModeSelect).toHaveValue('all');

    // Switch back to "No secrets".
    await accessModeSelect.selectOption('none');

    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await expect(saveButton).toBeEnabled();
    await saveButton.click();

    await expect(page.getByText('Successfully updated')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);

    // Verify via API that secrets mode is now "none" (omitted from JSON when none).
    const listRes = await page.request.get('/api/utils/web_scraping/page');
    expect(listRes.ok()).toBeTruthy();
    const trackers = await listRes.json();
    const tracker = trackers.find((t: { name: string }) => t.name === 'secrets-tracker');
    expect(tracker).toBeDefined();
    expect(tracker.secrets).toBeUndefined();

    // Re-open the edit flyout and confirm it shows "None".
    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout2 = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout2).toBeVisible();
    const accessModeSelect2 = flyout2.getByLabel('Access mode');
    await accessModeSelect2.scrollIntoViewIfNeeded();
    await expect(accessModeSelect2).toHaveValue('none');
  });

  test('changing secrets mode to "Selected" loads combo box and persists', async ({ page }) => {
    // Create a secret and a tracker via API.
    const secretRes = await page.request.post('/api/user/secrets', {
      data: { name: 'TRACKER_KEY', value: 'tracker-secret-value' },
    });
    expect(secretRes.ok()).toBeTruthy();

    const createRes = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'selected-secrets-tracker',
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
      },
    });
    expect(createRes.ok()).toBeTruthy();

    await page.goto('/ws/web_scraping__page');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'selected-secrets-tracker' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout).toBeVisible();

    // Change mode to "Selected secrets".
    const accessModeSelect = flyout.getByLabel('Access mode');
    await accessModeSelect.scrollIntoViewIfNeeded();
    await accessModeSelect.selectOption('selected');

    // The secrets combo box should appear and finish loading.
    const secretsCombo = flyout.getByRole('combobox', { name: 'Secrets' });
    await expect(secretsCombo).toBeVisible({ timeout: 15000 });

    // Select the secret from the combo box (retry in case loading resets the dropdown).
    const trackerOption = page.getByRole('option', { name: 'TRACKER_KEY' });
    await expect(async () => {
      await secretsCombo.click();
      await expect(trackerOption).toBeVisible({ timeout: 3000 });
    }).toPass({ timeout: 15000 });
    await trackerOption.click();
    await page.keyboard.press('Escape');

    // Save and verify.
    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await expect(saveButton).toBeEnabled();
    await saveButton.click();

    await expect(page.getByText('Successfully updated')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);

    // Verify via API.
    const listRes = await page.request.get('/api/utils/web_scraping/page');
    expect(listRes.ok()).toBeTruthy();
    const trackers = await listRes.json();
    const tracker = trackers.find((t: { name: string }) => t.name === 'selected-secrets-tracker');
    expect(tracker).toBeDefined();
    expect(tracker.secrets).toEqual({ type: 'selected', secrets: ['TRACKER_KEY'] });
  });
});

test.describe('Secrets access in responder edit flyout', () => {
  const SECRETS_EMAIL = 'e2e-secrets-responder@secutils.dev';
  const SECRETS_NONE_EMAIL = 'e2e-secrets-responder-none@secutils.dev';

  test.beforeEach(async ({ request }) => {
    for (const email of [SECRETS_EMAIL, SECRETS_NONE_EMAIL]) {
      await request.post('/api/users/remove', {
        headers: { Authorization: `Bearer ${OPERATOR_TOKEN}` },
        data: { email },
      });
    }
  });

  test('changing secrets mode to "Selected" loads combo box and persists', async ({ request, page }) => {
    await ensureUserAndLogin(request, page, { email: SECRETS_EMAIL, password: 'e2e_secutils_pass' });

    // Create a secret and a responder via API.
    const secretRes = await page.request.post('/api/user/secrets', {
      data: { name: 'RESPONDER_KEY', value: 'responder-secret-value' },
    });
    expect(secretRes.ok()).toBeTruthy();

    const createRes = await page.request.post('/api/utils/webhooks/responders', {
      data: {
        name: 'secrets-responder',
        location: { pathType: '=', path: '/secrets-test' },
        method: 'ANY',
        enabled: true,
        settings: { requestsToTrack: 0, statusCode: 200 },
      },
    });
    expect(createRes.ok()).toBeTruthy();

    await page.goto('/ws/webhooks__responders');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'secrets-responder' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit responder' }) });
    await expect(flyout).toBeVisible();

    // Enable advanced mode to reveal the Secrets section.
    await flyout.getByLabel('Advanced mode').click();

    const accessModeSelect = flyout.getByLabel('Access mode');
    await accessModeSelect.scrollIntoViewIfNeeded();
    await accessModeSelect.selectOption('selected');

    // The secrets combo box should appear and finish loading.
    const secretsCombo = flyout.getByRole('combobox', { name: 'Secrets' });
    await expect(secretsCombo).toBeVisible({ timeout: 15000 });

    // Select the secret (retry in case loading resets the dropdown).
    const responderOption = page.getByRole('option', { name: 'RESPONDER_KEY' });
    await expect(async () => {
      await secretsCombo.click();
      await expect(responderOption).toBeVisible({ timeout: 3000 });
    }).toPass({ timeout: 15000 });
    await responderOption.click();
    await page.keyboard.press('Escape');

    // Save.
    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await expect(saveButton).toBeEnabled();
    await saveButton.click();

    await expect(page.getByText('Successfully updated')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);

    // Verify via API that secrets were persisted.
    const listRes = await page.request.get('/api/utils/webhooks/responders');
    expect(listRes.ok()).toBeTruthy();
    const responders = await listRes.json();
    const responder = responders.find((r: { name: string }) => r.name === 'secrets-responder');
    expect(responder).toBeDefined();
    expect(responder.settings.secrets).toEqual({ type: 'selected', secrets: ['RESPONDER_KEY'] });
  });

  test('changing secrets mode from "All" back to "None" persists correctly', async ({ request, page }) => {
    await ensureUserAndLogin(request, page, { email: SECRETS_NONE_EMAIL, password: 'e2e_secutils_pass' });

    // Create a responder with "All" secrets via API.
    const createRes = await page.request.post('/api/utils/webhooks/responders', {
      data: {
        name: 'secrets-responder',
        location: { pathType: '=', path: '/secrets-none-test' },
        method: 'ANY',
        enabled: true,
        settings: { requestsToTrack: 0, statusCode: 200, secrets: { type: 'all' } },
      },
    });
    expect(createRes.ok()).toBeTruthy();

    await page.goto('/ws/webhooks__responders');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'secrets-responder' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit responder' }) });
    await expect(flyout).toBeVisible();

    // The responder has non-default secrets, so advanced mode should be auto-enabled.
    const accessModeSelect = flyout.getByLabel('Access mode');
    await accessModeSelect.scrollIntoViewIfNeeded();
    await expect(accessModeSelect).toHaveValue('all');

    // Switch back to "No secrets".
    await accessModeSelect.selectOption('none');

    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await expect(saveButton).toBeEnabled();
    await saveButton.click();

    await expect(page.getByText('Successfully updated')).toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);

    // Verify via API that secrets mode is now "none".
    const listRes = await page.request.get('/api/utils/webhooks/responders');
    expect(listRes.ok()).toBeTruthy();
    const responders = await listRes.json();
    const responder = responders.find((r: { name: string }) => r.name === 'secrets-responder');
    expect(responder).toBeDefined();
    expect(responder.settings.secrets).toBeUndefined();

    // Re-open and verify it shows "None".
    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout2 = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit responder' }) });
    await expect(flyout2).toBeVisible();

    // With secrets reset to none, advanced mode should not be auto-enabled.
    await flyout2.getByLabel('Advanced mode').click();
    const accessModeSelect2 = flyout2.getByLabel('Access mode');
    await accessModeSelect2.scrollIntoViewIfNeeded();
    await expect(accessModeSelect2).toHaveValue('none');
  });
});
