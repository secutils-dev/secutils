import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

test.describe('Unsaved changes confirmation', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test.describe('CSP policy flyout', () => {
    test('create - closes without confirmation when no changes were made', async ({ page }) => {
      await page.goto('/ws/web_security__csp__policies');
      const createButton = page.getByRole('button', { name: 'Create policy' });
      await expect(createButton).toBeVisible({ timeout: 15000 });

      await createButton.click();
      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add policy' }) });
      await expect(flyout).toBeVisible();

      await flyout.getByRole('button', { name: 'Close' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('create - shows confirmation when closing with unsaved changes via Close button', async ({ page }) => {
      await page.goto('/ws/web_security__csp__policies');
      const createButton = page.getByRole('button', { name: 'Create policy' });
      await expect(createButton).toBeVisible({ timeout: 15000 });

      await createButton.click();
      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add policy' }) });
      await expect(flyout).toBeVisible();

      const nameInput = flyout.getByLabel('Name');
      await nameInput.fill('test-policy');
      await expect(nameInput).toHaveValue('test-policy');

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });
      await expect(confirmModal.getByText('You have unsaved changes')).toBeVisible();

      await confirmModal.getByRole('button', { name: 'Keep editing' }).click();
      await expect(confirmModal).not.toBeVisible();
      await expect(flyout).toBeVisible();
    });

    test('create - discards changes when confirming the discard dialog', async ({ page }) => {
      await page.goto('/ws/web_security__csp__policies');
      const createButton = page.getByRole('button', { name: 'Create policy' });
      await expect(createButton).toBeVisible({ timeout: 15000 });

      await createButton.click();
      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add policy' }) });
      await expect(flyout).toBeVisible();

      const nameInput = flyout.getByLabel('Name');
      await nameInput.fill('test-policy');
      await expect(nameInput).toHaveValue('test-policy');

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Discard' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('create - shows confirmation when closing with unsaved changes via overlay click', async ({ page }) => {
      await page.goto('/ws/web_security__csp__policies');
      const createButton = page.getByRole('button', { name: 'Create policy' });
      await expect(createButton).toBeVisible({ timeout: 15000 });

      await createButton.click();
      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add policy' }) });
      await expect(flyout).toBeVisible();

      const nameInput = flyout.getByLabel('Name');
      await nameInput.fill('test-policy');
      await expect(nameInput).toHaveValue('test-policy');

      // Click outside the flyout (on the overlay mask).
      await page.mouse.click(10, 300);

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Discard' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('edit - closes without confirmation when no changes were made', async ({ page }) => {
      const res = await page.request.post('/api/utils/web_security/csp', {
        data: {
          name: 'existing-policy',
          content: { type: 'directives', value: [{ name: 'default-src', value: ["'self'"] }] },
        },
      });
      expect(res.ok()).toBeTruthy();

      await page.goto('/ws/web_security__csp__policies');
      const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'existing-policy' }) });
      await expect(row).toBeVisible({ timeout: 15000 });

      await row.getByRole('button', { name: 'Edit' }).click();
      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit policy' }) });
      await expect(flyout).toBeVisible();

      await flyout.getByRole('button', { name: 'Close' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('edit - shows confirmation when closing with unsaved changes', async ({ page }) => {
      const res = await page.request.post('/api/utils/web_security/csp', {
        data: {
          name: 'existing-policy',
          content: { type: 'directives', value: [{ name: 'default-src', value: ["'self'"] }] },
        },
      });
      expect(res.ok()).toBeTruthy();

      await page.goto('/ws/web_security__csp__policies');
      const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'existing-policy' }) });
      await expect(row).toBeVisible({ timeout: 15000 });

      await row.getByRole('button', { name: 'Edit' }).click();
      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit policy' }) });
      await expect(flyout).toBeVisible();

      const nameInput = flyout.getByLabel('Name');
      await nameInput.fill('modified-policy');
      await expect(nameInput).toHaveValue('modified-policy');

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Discard' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('duplicate - shows confirmation when closing', async ({ page }) => {
      const res = await page.request.post('/api/utils/web_security/csp', {
        data: {
          name: 'existing-policy',
          content: { type: 'directives', value: [{ name: 'default-src', value: ["'self'"] }] },
        },
      });
      expect(res.ok()).toBeTruthy();

      await page.goto('/ws/web_security__csp__policies');
      const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'existing-policy' }) });
      await expect(row).toBeVisible({ timeout: 15000 });

      await row.getByRole('button', { name: 'All actions, row' }).click();
      const duplicateButton = page.getByRole('button', { name: 'Duplicate', exact: true });
      await expect(duplicateButton).toBeVisible();
      await duplicateButton.click();

      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add policy' }) });
      await expect(flyout).toBeVisible();

      // Verify the duplicate flyout is pre-filled with the copy name.
      const nameInput = flyout.getByLabel('Name');
      await expect(nameInput).toHaveValue('existing-policy (Copy 1)');

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Discard' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });
  });

  test.describe('Private key flyout', () => {
    test('create - closes without confirmation when no changes were made', async ({ page }) => {
      await page.goto('/ws/certificates__private_keys');
      const createButton = page.getByRole('button', { name: 'Create private key' });
      await expect(createButton).toBeVisible({ timeout: 15000 });

      await createButton.click();
      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add private key' }) });
      await expect(flyout).toBeVisible();

      await flyout.getByRole('button', { name: 'Close' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('create - shows confirmation when closing with unsaved changes', async ({ page }) => {
      await page.goto('/ws/certificates__private_keys');
      const createButton = page.getByRole('button', { name: 'Create private key' });
      await expect(createButton).toBeVisible({ timeout: 15000 });

      await createButton.click();
      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add private key' }) });
      await expect(flyout).toBeVisible();

      const nameInput = flyout.getByLabel('Name');
      await nameInput.fill('test-key');
      await expect(nameInput).toHaveValue('test-key');

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Keep editing' }).click();
      await expect(confirmModal).not.toBeVisible();
      await expect(flyout).toBeVisible();
      await expect(flyout.getByLabel('Name')).toHaveValue('test-key');
    });

    test('edit - closes without confirmation when no changes were made', async ({ page }) => {
      const res = await page.request.post('/api/utils/certificates/private_keys', {
        data: { keyName: 'existing-key', alg: { keyType: 'ed25519' } },
      });
      expect(res.ok()).toBeTruthy();

      await page.goto('/ws/certificates__private_keys');
      const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'existing-key' }) });
      await expect(row).toBeVisible({ timeout: 15000 });

      await row.getByRole('button', { name: 'Edit' }).click();
      const flyout = page
        .getByRole('dialog')
        .filter({ has: page.getByRole('heading', { name: 'Edit private key' }) });
      await expect(flyout).toBeVisible();

      await flyout.getByRole('button', { name: 'Close' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('edit - shows confirmation when closing with unsaved changes', async ({ page }) => {
      const res = await page.request.post('/api/utils/certificates/private_keys', {
        data: { keyName: 'existing-key', alg: { keyType: 'ed25519' } },
      });
      expect(res.ok()).toBeTruthy();

      await page.goto('/ws/certificates__private_keys');
      const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'existing-key' }) });
      await expect(row).toBeVisible({ timeout: 15000 });

      await row.getByRole('button', { name: 'Edit' }).click();
      const flyout = page
        .getByRole('dialog')
        .filter({ has: page.getByRole('heading', { name: 'Edit private key' }) });
      await expect(flyout).toBeVisible();

      const nameInput = flyout.getByLabel('Name');
      await nameInput.fill('modified-key');
      await expect(nameInput).toHaveValue('modified-key');

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Discard' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('duplicate - shows confirmation when closing', async ({ page }) => {
      const res = await page.request.post('/api/utils/certificates/private_keys', {
        data: { keyName: 'existing-key', alg: { keyType: 'ed25519' } },
      });
      expect(res.ok()).toBeTruthy();

      await page.goto('/ws/certificates__private_keys');
      const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'existing-key' }) });
      await expect(row).toBeVisible({ timeout: 15000 });

      await row.getByRole('button', { name: 'All actions, row' }).click();
      const duplicateButton = page.getByRole('button', { name: 'Duplicate', exact: true });
      await expect(duplicateButton).toBeVisible();
      await duplicateButton.click();

      const flyout = page
        .getByRole('dialog')
        .filter({ has: page.getByRole('heading', { name: 'Add private key' }) });
      await expect(flyout).toBeVisible();

      // Verify the duplicate flyout is pre-filled with the copy name.
      await expect(flyout.getByLabel('Name')).toHaveValue('existing-key (Copy 1)');

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Discard' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });
  });

  test.describe('Certificate template flyout', () => {
    test('create - closes without confirmation when no changes were made', async ({ page }) => {
      await page.goto('/ws/certificates__certificate_templates');
      const createButton = page.getByRole('button', { name: 'Create template' });
      await expect(createButton).toBeVisible({ timeout: 15000 });

      await createButton.click();
      const flyout = page
        .getByRole('dialog')
        .filter({ has: page.getByRole('heading', { name: 'Add certificate template' }) });
      await expect(flyout).toBeVisible();

      await flyout.getByRole('button', { name: 'Close' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('create - shows confirmation when closing with unsaved changes', async ({ page }) => {
      await page.goto('/ws/certificates__certificate_templates');
      const createButton = page.getByRole('button', { name: 'Create template' });
      await expect(createButton).toBeVisible({ timeout: 15000 });

      await createButton.click();
      const flyout = page
        .getByRole('dialog')
        .filter({ has: page.getByRole('heading', { name: 'Add certificate template' }) });
      await expect(flyout).toBeVisible();

      const nameInput = flyout.getByRole('textbox', { name: 'Name', exact: true });
      await nameInput.fill('test-template');
      await expect(nameInput).toHaveValue('test-template');

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Discard' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('edit - closes without confirmation when no changes were made', async ({ page }) => {
      const now = Math.floor(Date.now() / 1000);
      const res = await page.request.post('/api/utils/certificates/templates', {
        data: {
          templateName: 'existing-template',
          attributes: {
            commonName: 'Test CN',
            country: 'US',
            keyAlgorithm: { keyType: 'ed25519' },
            signatureAlgorithm: 'ed25519',
            notValidBefore: now,
            notValidAfter: now + 86400 * 365,
            isCa: true,
          },
        },
      });
      expect(res.ok()).toBeTruthy();

      await page.goto('/ws/certificates__certificate_templates');
      const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'existing-template' }) });
      await expect(row).toBeVisible({ timeout: 15000 });

      await row.getByRole('button', { name: 'Edit' }).click();
      const flyout = page
        .getByRole('dialog')
        .filter({ has: page.getByRole('heading', { name: 'Edit certificate template' }) });
      await expect(flyout).toBeVisible();

      await flyout.getByRole('button', { name: 'Close' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('edit - shows confirmation when closing with unsaved changes', async ({ page }) => {
      const now = Math.floor(Date.now() / 1000);
      const res = await page.request.post('/api/utils/certificates/templates', {
        data: {
          templateName: 'existing-template',
          attributes: {
            commonName: 'Test CN',
            country: 'US',
            keyAlgorithm: { keyType: 'ed25519' },
            signatureAlgorithm: 'ed25519',
            notValidBefore: now,
            notValidAfter: now + 86400 * 365,
            isCa: true,
          },
        },
      });
      expect(res.ok()).toBeTruthy();

      await page.goto('/ws/certificates__certificate_templates');
      const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'existing-template' }) });
      await expect(row).toBeVisible({ timeout: 15000 });

      await row.getByRole('button', { name: 'Edit' }).click();
      const flyout = page
        .getByRole('dialog')
        .filter({ has: page.getByRole('heading', { name: 'Edit certificate template' }) });
      await expect(flyout).toBeVisible();

      const nameInput = flyout.getByRole('textbox', { name: 'Name', exact: true });
      await nameInput.fill('modified-template');
      await expect(nameInput).toHaveValue('modified-template');

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Discard' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('duplicate - shows confirmation when closing', async ({ page }) => {
      const now = Math.floor(Date.now() / 1000);
      const res = await page.request.post('/api/utils/certificates/templates', {
        data: {
          templateName: 'existing-template',
          attributes: {
            commonName: 'Test CN',
            country: 'US',
            keyAlgorithm: { keyType: 'ed25519' },
            signatureAlgorithm: 'ed25519',
            notValidBefore: now,
            notValidAfter: now + 86400 * 365,
            isCa: true,
          },
        },
      });
      expect(res.ok()).toBeTruthy();

      await page.goto('/ws/certificates__certificate_templates');
      const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'existing-template' }) });
      await expect(row).toBeVisible({ timeout: 15000 });

      await row.getByRole('button', { name: 'All actions, row' }).click();
      const duplicateButton = page.getByRole('button', { name: 'Duplicate', exact: true });
      await expect(duplicateButton).toBeVisible();
      await duplicateButton.click();

      const flyout = page
        .getByRole('dialog')
        .filter({ has: page.getByRole('heading', { name: 'Add certificate template' }) });
      await expect(flyout).toBeVisible();

      // Verify the duplicate flyout is pre-filled with the copy name.
      await expect(flyout.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
        'existing-template (Copy 1)',
      );

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Discard' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });
  });

  test.describe('Responder flyout', () => {
    test('create - closes without confirmation when no changes were made', async ({ page }) => {
      await page.goto('/ws/webhooks__responders');
      const createButton = page.getByRole('button', { name: 'Create responder' });
      await expect(createButton).toBeVisible({ timeout: 15000 });

      await createButton.click();
      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add responder' }) });
      await expect(flyout).toBeVisible();

      await flyout.getByRole('button', { name: 'Close' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('create - shows confirmation when closing with unsaved changes', async ({ page }) => {
      await page.goto('/ws/webhooks__responders');
      const createButton = page.getByRole('button', { name: 'Create responder' });
      await expect(createButton).toBeVisible({ timeout: 15000 });

      await createButton.click();
      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add responder' }) });
      await expect(flyout).toBeVisible();

      const nameInput = flyout.getByLabel('Name');
      await nameInput.fill('test-responder');
      await expect(nameInput).toHaveValue('test-responder');

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Discard' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('edit - closes without confirmation when no changes were made', async ({ page }) => {
      const res = await page.request.post('/api/utils/webhooks/responders', {
        data: {
          name: 'existing-responder',
          location: { pathType: '=', path: '/test' },
          method: 'ANY',
          enabled: true,
          settings: {
            requestsToTrack: 5,
            statusCode: 200,
            headers: [['Content-Type', 'text/html; charset=utf-8']],
          },
        },
      });
      expect(res.ok()).toBeTruthy();

      await page.goto('/ws/webhooks__responders');
      const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'existing-responder' }) });
      await expect(row).toBeVisible({ timeout: 15000 });

      await row.getByRole('button', { name: 'Edit' }).click();
      const flyout = page
        .getByRole('dialog')
        .filter({ has: page.getByRole('heading', { name: 'Edit responder' }) });
      await expect(flyout).toBeVisible();

      await flyout.getByRole('button', { name: 'Close' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('edit - shows confirmation when closing with unsaved changes', async ({ page }) => {
      const res = await page.request.post('/api/utils/webhooks/responders', {
        data: {
          name: 'existing-responder',
          location: { pathType: '=', path: '/test' },
          method: 'ANY',
          enabled: true,
          settings: {
            requestsToTrack: 5,
            statusCode: 200,
            headers: [['Content-Type', 'text/html; charset=utf-8']],
          },
        },
      });
      expect(res.ok()).toBeTruthy();

      await page.goto('/ws/webhooks__responders');
      const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'existing-responder' }) });
      await expect(row).toBeVisible({ timeout: 15000 });

      await row.getByRole('button', { name: 'Edit' }).click();
      const flyout = page
        .getByRole('dialog')
        .filter({ has: page.getByRole('heading', { name: 'Edit responder' }) });
      await expect(flyout).toBeVisible();

      const nameInput = flyout.getByLabel('Name');
      await nameInput.fill('modified-responder');
      await expect(nameInput).toHaveValue('modified-responder');

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Discard' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('duplicate - shows confirmation when closing', async ({ page }) => {
      const res = await page.request.post('/api/utils/webhooks/responders', {
        data: {
          name: 'existing-responder',
          location: { pathType: '=', path: '/test' },
          method: 'ANY',
          enabled: true,
          settings: {
            requestsToTrack: 5,
            statusCode: 200,
            headers: [['Content-Type', 'text/html; charset=utf-8']],
          },
        },
      });
      expect(res.ok()).toBeTruthy();

      await page.goto('/ws/webhooks__responders');
      const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'existing-responder' }) });
      await expect(row).toBeVisible({ timeout: 15000 });

      await row.getByRole('button', { name: 'All actions, row' }).click();
      const duplicateButton = page.getByRole('button', { name: 'Duplicate', exact: true });
      await expect(duplicateButton).toBeVisible();
      await duplicateButton.click();

      // Responder uses `responder ?` check so duplicate (object exists) shows "Edit responder".
      const flyout = page
        .getByRole('dialog')
        .filter({ has: page.getByRole('heading', { name: 'Edit responder' }) });
      await expect(flyout).toBeVisible();

      // Verify the duplicate flyout is pre-filled with the copy name.
      await expect(flyout.getByLabel('Name')).toHaveValue('existing-responder (Copy 1)');

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Discard' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });
  });

  test.describe('Page tracker flyout', () => {
    test('create - closes without confirmation when no changes were made', async ({ page }) => {
      await page.goto('/ws/web_scraping__page');
      const createButton = page.getByRole('button', { name: 'Track page' });
      await expect(createButton).toBeVisible({ timeout: 15000 });

      await createButton.click();
      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add tracker' }) });
      await expect(flyout).toBeVisible();

      await flyout.getByRole('button', { name: 'Close' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('create - shows confirmation when closing with unsaved changes', async ({ page }) => {
      await page.goto('/ws/web_scraping__page');
      const createButton = page.getByRole('button', { name: 'Track page' });
      await expect(createButton).toBeVisible({ timeout: 15000 });

      await createButton.click();
      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add tracker' }) });
      await expect(flyout).toBeVisible();

      const nameInput = flyout.getByLabel('Name');
      await nameInput.fill('test-tracker');
      await expect(nameInput).toHaveValue('test-tracker');

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Keep editing' }).click();
      await expect(confirmModal).not.toBeVisible();
      await expect(flyout).toBeVisible();
      await expect(flyout.getByLabel('Name')).toHaveValue('test-tracker');
    });

    test('edit - closes without confirmation when no changes were made', async ({ page }) => {
      const res = await page.request.post('/api/utils/web_scraping/page', {
        data: {
          name: 'existing-tracker',
          config: { revisions: 3 },
          target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
        },
      });
      expect(res.ok()).toBeTruthy();

      await page.goto('/ws/web_scraping__page');
      const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'existing-tracker' }) });
      await expect(row).toBeVisible({ timeout: 15000 });

      await row.getByRole('button', { name: 'Edit' }).click();
      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
      await expect(flyout).toBeVisible();

      await flyout.getByRole('button', { name: 'Close' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('edit - shows confirmation when closing with unsaved changes', async ({ page }) => {
      const res = await page.request.post('/api/utils/web_scraping/page', {
        data: {
          name: 'existing-tracker',
          config: { revisions: 3 },
          target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
        },
      });
      expect(res.ok()).toBeTruthy();

      await page.goto('/ws/web_scraping__page');
      const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'existing-tracker' }) });
      await expect(row).toBeVisible({ timeout: 15000 });

      await row.getByRole('button', { name: 'Edit' }).click();
      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
      await expect(flyout).toBeVisible();

      const nameInput = flyout.getByLabel('Name');
      await nameInput.fill('modified-tracker');
      await expect(nameInput).toHaveValue('modified-tracker');

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Discard' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });

    test('duplicate - shows confirmation when closing', async ({ page }) => {
      const res = await page.request.post('/api/utils/web_scraping/page', {
        data: {
          name: 'existing-tracker',
          config: { revisions: 3 },
          target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
        },
      });
      expect(res.ok()).toBeTruthy();

      await page.goto('/ws/web_scraping__page');
      const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'existing-tracker' }) });
      await expect(row).toBeVisible({ timeout: 15000 });

      await row.getByRole('button', { name: 'All actions, row' }).click();
      const duplicateButton = page.getByRole('button', { name: 'Duplicate', exact: true });
      await expect(duplicateButton).toBeVisible();
      await duplicateButton.click();

      // Tracker uses `tracker ?` check so duplicate (object exists) shows "Edit tracker".
      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
      await expect(flyout).toBeVisible();

      // Verify the duplicate flyout is pre-filled with the copy name.
      await expect(flyout.getByLabel('Name')).toHaveValue('existing-tracker (Copy 1)');

      await flyout.getByRole('button', { name: 'Close' }).click();

      const confirmModal = page.getByRole('alertdialog').filter({
        has: page.getByRole('heading', { name: 'Discard unsaved changes?' }),
      });
      await expect(confirmModal).toBeVisible({ timeout: 10000 });

      await confirmModal.getByRole('button', { name: 'Discard' }).click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
    });
  });
});
