import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

test.describe('Data Export and Import', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('can navigate to Account tab with data actions in settings', async ({ page }) => {
    // Open settings flyout.
    await page.getByRole('button', { name: 'Account menu' }).click();
    const settingsButton = page.getByText('Settings');
    await expect(settingsButton).toBeVisible();
    await settingsButton.click();

    // Navigate to Account tab.
    const accountTab = page.getByRole('tab', { name: 'Account' });
    await expect(accountTab).toBeVisible({ timeout: 15000 });
    await accountTab.click();

    // Verify export and import buttons are visible.
    await expect(page.getByRole('button', { name: 'Export data' })).toBeVisible({ timeout: 15000 });
    await expect(page.getByRole('button', { name: 'Import data' })).toBeVisible({ timeout: 15000 });
  });

  test('can open export modal', async ({ page }) => {
    // Open settings flyout and navigate to Account tab.
    await page.getByRole('button', { name: 'Account menu' }).click();
    await page.getByText('Settings').click();
    const accountTab = page.getByRole('tab', { name: 'Account' });
    await expect(accountTab).toBeVisible({ timeout: 15000 });
    await accountTab.click();

    // Click the export button.
    await page.getByRole('button', { name: 'Export data' }).click();

    // Verify the export modal is visible (use .euiModal to distinguish from the flyout dialog).
    const modal = page.locator('.euiModal').filter({ has: page.getByText('Export data') });
    await expect(modal).toBeVisible({ timeout: 15000 });

    // Verify Cancel button works.
    await modal.getByRole('button', { name: 'Cancel' }).click();
    await expect(modal).not.toBeVisible();
  });

  test('can open import modal', async ({ page }) => {
    // Open settings flyout and navigate to the Account tab.
    await page.getByRole('button', { name: 'Account menu' }).click();
    await page.getByText('Settings').click();
    const accountTab = page.getByRole('tab', { name: 'Account' });
    await expect(accountTab).toBeVisible({ timeout: 15000 });
    await accountTab.click();

    // Click import button.
    await page.getByRole('button', { name: 'Import data' }).click();

    // Verify the import modal is visible (use .euiModal to distinguish from the flyout dialog).
    const modal = page.locator('.euiModal').filter({ has: page.getByText('Import data') });
    await expect(modal).toBeVisible({ timeout: 15000 });

    // Verify mode selection is present.
    await expect(modal.getByText('Merge')).toBeVisible();
    await expect(modal.getByText('Apply')).toBeVisible();

    // Verify Cancel button works.
    await modal.getByRole('button', { name: 'Cancel' }).click();
    await expect(modal).not.toBeVisible();
  });

  test('export API returns valid data', async ({ page }) => {
    // First, create a script via the API so there's data to export.
    const createResponse = await page.request.post('/api/user/scripts', {
      data: { name: 'export_test_script', scriptType: 'responder', content: 'console.log("test")' },
    });
    expect(createResponse.ok()).toBeTruthy();
    const script = await createResponse.json();

    // Set some user settings to export.
    const setSettingsRes = await page.request.post('/api/user/settings', {
      data: { 'common.uiTheme': 'dark' },
    });
    expect(setSettingsRes.ok()).toBeTruthy();

    // Call the export API.
    const exportResponse = await page.request.post('/api/user/data/_export', {
      data: {
        include: {
          scripts: { type: 'selected', ids: [script.id] },
          settings: true,
        },
      },
    });
    expect(exportResponse.ok()).toBeTruthy();

    const exportData = await exportResponse.json();
    expect(exportData.version).toBe(1);
    expect(exportData.exportedAt).toBeTruthy();
    expect(exportData.data.scripts).toHaveLength(1);
    expect(exportData.data.scripts[0].name).toBe('export_test_script');
    expect(exportData.data.scripts[0].content).toBe('console.log("test")');
    expect(exportData.data.settings).toBeDefined();
    expect(exportData.data.settings['common.uiTheme']).toBe('dark');

    // Clean up.
    await page.request.delete(`/api/user/scripts/${encodeURIComponent(script.id)}`);
    // Reset settings.
    await page.request.post('/api/user/settings', {
      data: { 'common.uiTheme': null },
    });
  });

  test('import preview API detects conflicts', async ({ page }) => {
    // Create a script.
    const createResponse = await page.request.post('/api/user/scripts', {
      data: { name: 'conflict_script', scriptType: 'responder', content: 'original' },
    });
    expect(createResponse.ok()).toBeTruthy();
    const script = await createResponse.json();

    // Create an import file that conflicts with the existing script.
    const importFile = {
      version: 1,
      exportedAt: 1577880000,
      data: {
        scripts: [
          {
            id: '019568f0-0000-7000-8000-000000000001',
            name: 'conflict_script',
            scriptType: 'responder',
            content: 'new content',
            createdAt: 1577836800,
            updatedAt: 1577836800,
          },
        ],
      },
    };

    const previewResponse = await page.request.post('/api/user/data/_import_preview', {
      data: { data: importFile, mode: 'merge' },
    });
    expect(previewResponse.ok()).toBeTruthy();

    const preview = await previewResponse.json();
    expect(preview.valid).toBe(true);
    expect(preview.summary.scripts.total).toBe(1);
    expect(preview.summary.scripts.conflicts).toHaveLength(1);
    expect(preview.summary.scripts.conflicts[0].name).toBe('conflict_script');

    // Clean up.
    await page.request.delete(`/api/user/scripts/${encodeURIComponent(script.id)}`);
  });

  test('import with rename conflict resolution', async ({ page }) => {
    // Create a script.
    const createResponse = await page.request.post('/api/user/scripts', {
      data: { name: 'rename_test', scriptType: 'responder', content: 'original' },
    });
    expect(createResponse.ok()).toBeTruthy();

    // Import a script with the same name, using rename resolution.
    const importFile = {
      version: 1,
      exportedAt: 1577880000,
      data: {
        scripts: [
          {
            id: '019568f0-0000-7000-8000-000000000010',
            name: 'rename_test',
            scriptType: 'responder',
            content: 'imported content',
            createdAt: 1577836800,
            updatedAt: 1577836800,
          },
        ],
      },
    };

    const importResponse = await page.request.post('/api/user/data/_import', {
      data: {
        data: importFile,
        mode: 'merge',
        selections: {
          scripts: [
            { sourceId: '019568f0-0000-7000-8000-000000000010', action: 'import', conflictResolution: 'rename' },
          ],
          secrets: [],
          responders: [],
          certificateTemplates: [],
          privateKeys: [],
          contentSecurityPolicies: [],
          pageTrackers: [],
          apiTrackers: [],
        },
      },
    });
    expect(importResponse.ok()).toBeTruthy();
    const result = await importResponse.json();
    expect(result.results.scripts.imported).toBe(1);

    // Verify both scripts exist - original and renamed copy.
    const listResponse = await page.request.get('/api/user/scripts');
    const scripts = await listResponse.json();
    expect(scripts.find((s: { name: string }) => s.name === 'rename_test')).toBeTruthy();
    expect(scripts.find((s: { name: string }) => s.name === 'rename_test (Copy 1)')).toBeTruthy();

    // Clean up.
    for (const s of scripts.filter((s: { name: string }) => s.name.startsWith('rename_test'))) {
      await page.request.delete(`/api/user/scripts/${encodeURIComponent(s.id)}`);
    }
  });

  test('import with overwrite conflict resolution', async ({ page }) => {
    // Create a script.
    const createResponse = await page.request.post('/api/user/scripts', {
      data: { name: 'overwrite_test', scriptType: 'responder', content: 'original' },
    });
    expect(createResponse.ok()).toBeTruthy();

    // Import a script with the same name, using overwrite resolution.
    const importFile = {
      version: 1,
      exportedAt: 1577880000,
      data: {
        scripts: [
          {
            id: '019568f0-0000-7000-8000-000000000011',
            name: 'overwrite_test',
            scriptType: 'responder',
            content: 'overwritten content',
            createdAt: 1577836800,
            updatedAt: 1577836800,
          },
        ],
      },
    };

    const importResponse = await page.request.post('/api/user/data/_import', {
      data: {
        data: importFile,
        mode: 'merge',
        selections: {
          scripts: [
            { sourceId: '019568f0-0000-7000-8000-000000000011', action: 'import', conflictResolution: 'overwrite' },
          ],
          secrets: [],
          responders: [],
          certificateTemplates: [],
          privateKeys: [],
          contentSecurityPolicies: [],
          pageTrackers: [],
          apiTrackers: [],
        },
      },
    });
    expect(importResponse.ok()).toBeTruthy();
    const result = await importResponse.json();
    expect(result.results.scripts.imported).toBe(1);

    // Verify only one script exists with the new content.
    const listResponse = await page.request.get('/api/user/scripts');
    const scripts = await listResponse.json();
    const found = scripts.filter((s: { name: string }) => s.name === 'overwrite_test');
    expect(found).toHaveLength(1);
    expect(found[0].content).toBe('overwritten content');

    // Clean up.
    await page.request.delete(`/api/user/scripts/${encodeURIComponent(found[0].id)}`);
  });

  test('apply mode with deletions via API', async ({ page }) => {
    // Create two scripts.
    const create1 = await page.request.post('/api/user/scripts', {
      data: { name: 'keep_script', scriptType: 'responder', content: 'keep' },
    });
    expect(create1.ok()).toBeTruthy();

    const create2 = await page.request.post('/api/user/scripts', {
      data: { name: 'delete_script', scriptType: 'responder', content: 'delete me' },
    });
    expect(create2.ok()).toBeTruthy();
    const script2 = await create2.json();

    // Import file only has keep_script.
    const importFile = {
      version: 1,
      exportedAt: 1577880000,
      data: {
        scripts: [
          {
            id: '019568f0-0000-7000-8000-000000000020',
            name: 'keep_script',
            scriptType: 'responder',
            content: 'keep',
            createdAt: 1577836800,
            updatedAt: 1577836800,
          },
        ],
      },
    };

    // Preview should detect delete_script for deletion.
    const previewResponse = await page.request.post('/api/user/data/_import_preview', {
      data: { data: importFile, mode: 'apply' },
    });
    expect(previewResponse.ok()).toBeTruthy();
    const preview = await previewResponse.json();
    expect(preview.toDelete.scripts).toHaveLength(1);
    expect(preview.toDelete.scripts[0].name).toBe('delete_script');

    // Import with apply mode, confirming the deletion.
    const importResponse = await page.request.post('/api/user/data/_import', {
      data: {
        data: importFile,
        mode: 'apply',
        selections: {
          scripts: [
            { sourceId: '019568f0-0000-7000-8000-000000000020', action: 'import', conflictResolution: 'overwrite' },
          ],
          secrets: [],
          responders: [],
          certificateTemplates: [],
          privateKeys: [],
          contentSecurityPolicies: [],
          pageTrackers: [],
          apiTrackers: [],
        },
        applyDeletions: {
          scripts: [script2.id],
          secrets: [],
          responders: [],
          certificateTemplates: [],
          privateKeys: [],
          contentSecurityPolicies: [],
          pageTrackers: [],
          apiTrackers: [],
        },
      },
    });
    expect(importResponse.ok()).toBeTruthy();
    const result = await importResponse.json();
    expect(result.results.scripts.deleted).toBe(1);

    // Verify delete_script is gone.
    const listResponse = await page.request.get('/api/user/scripts');
    const scripts = await listResponse.json();
    expect(scripts.find((s: { name: string }) => s.name === 'delete_script')).toBeUndefined();
    expect(scripts.find((s: { name: string }) => s.name === 'keep_script')).toBeTruthy();

    // Clean up.
    for (const s of scripts.filter((s: { name: string }) => s.name === 'keep_script')) {
      await page.request.delete(`/api/user/scripts/${encodeURIComponent(s.id)}`);
    }
  });

  test('_import_preview endpoint in `apply` mode returns preview without changes', async ({ page }) => {
    // Create a script.
    const createResponse = await page.request.post('/api/user/scripts', {
      data: { name: 'dryrun_script', scriptType: 'responder', content: 'test' },
    });
    expect(createResponse.ok()).toBeTruthy();
    const script = await createResponse.json();

    // Preview with `apply` mode and an empty file - should show dryrun_script for deletion.
    const previewResponse = await page.request.post('/api/user/data/_import_preview', {
      data: {
        data: {
          version: 1,
          exportedAt: 1577880000,
          data: { scripts: [] },
        },
        mode: 'apply',
      },
    });
    expect(previewResponse.ok()).toBeTruthy();
    const result = await previewResponse.json();
    expect(result.toDelete.scripts.find((s: { name: string }) => s.name === 'dryrun_script')).toBeTruthy();

    // Verify the script still exists (dry-run didn't delete it).
    const listResponse = await page.request.get('/api/user/scripts');
    const scripts = await listResponse.json();
    expect(scripts.find((s: { name: string }) => s.name === 'dryrun_script')).toBeTruthy();

    // Clean up.
    await page.request.delete(`/api/user/scripts/${encodeURIComponent(script.id)}`);
  });

  test('full export-import round trip via API', async ({ page }) => {
    // Create a script.
    const createResponse = await page.request.post('/api/user/scripts', {
      data: { name: 'roundtrip_script', scriptType: 'responder', content: 'roundtrip content' },
    });
    expect(createResponse.ok()).toBeTruthy();
    const script = await createResponse.json();

    // Export.
    const exportResponse = await page.request.post('/api/user/data/_export', {
      data: {
        include: {
          scripts: { type: 'selected', ids: [script.id] },
        },
      },
    });
    expect(exportResponse.ok()).toBeTruthy();
    const exportData = await exportResponse.json();

    // Delete the original script.
    await page.request.delete(`/api/user/scripts/${encodeURIComponent(script.id)}`);

    // Verify it's gone.
    const listResponse = await page.request.get('/api/user/scripts');
    const scripts = await listResponse.json();
    expect(scripts.find((s: { name: string }) => s.name === 'roundtrip_script')).toBeUndefined();

    // Import.
    const importResponse = await page.request.post('/api/user/data/_import', {
      data: {
        data: exportData,
        mode: 'merge',
        selections: {
          scripts: [
            {
              sourceId: exportData.data.scripts[0].id,
              action: 'import',
            },
          ],
          secrets: [],
          responders: [],
          certificateTemplates: [],
          privateKeys: [],
          contentSecurityPolicies: [],
          pageTrackers: [],
          apiTrackers: [],
        },
      },
    });
    expect(importResponse.ok()).toBeTruthy();

    const result = await importResponse.json();
    expect(result.results.scripts.imported).toBe(1);

    // Verify the script exists again.
    const listAfter = await page.request.get('/api/user/scripts');
    const scriptsAfter = await listAfter.json();
    expect(scriptsAfter.find((s: { name: string }) => s.name === 'roundtrip_script')).toBeTruthy();
  });

  test('comprehensive import and validation of all entity types', async ({ page }) => {
    // ── UUIDs for import entities ──────────────────────────────────────
    const UUID_SCRIPT = '019568f0-0000-7000-8000-000000000101';
    const UUID_SECRET = '019568f0-0000-7000-8000-000000000102';
    const UUID_CSP = '019568f0-0000-7000-8000-000000000103';
    const UUID_CERT_TEMPLATE = '019568f0-0000-7000-8000-000000000104';
    const UUID_PK = '019568f0-0000-7000-8000-000000000105';
    const UUID_RESPONDER = '019568f0-0000-7000-8000-000000000106';
    const UUID_RESPONDER_NO_HISTORY = '019568f0-0000-7000-8000-000000000109';
    const UUID_RESP_HISTORY = '019568f0-0000-7000-8000-000000000116';
    const UUID_PAGE_TRACKER = '019568f0-0000-7000-8000-000000000107';
    const UUID_PAGE_REV = '019568f0-0000-7000-8000-000000000117';
    const UUID_API_TRACKER = '019568f0-0000-7000-8000-000000000108';
    const UUID_API_REV_1 = '019568f0-0000-7000-8000-000000000118';
    const UUID_API_REV_2 = '019568f0-0000-7000-8000-000000000128';
    const UUID_TAG_1 = '019568f0-0000-7000-8000-000000000201';
    const UUID_TAG_2 = '019568f0-0000-7000-8000-000000000202';

    const SECRET_NAME = 'IMPORT_SECRET';
    const SECRET_VALUE = 'e2e-secret-42';
    const SECRETS_PASSPHRASE = 'e2e-secrets-passphrase';
    const PK_PASSPHRASE = 'test-pass-123';

    // ── Step 1: Create secret + private key, export both with passphrase, delete originals ──
    const secretCreateRes = await page.request.post('/api/user/secrets', {
      data: { name: SECRET_NAME, value: SECRET_VALUE },
    });
    expect(secretCreateRes.ok()).toBeTruthy();

    const pkCreateRes = await page.request.post('/api/certificates/private_keys', {
      data: { keyName: 'temp-pk-for-export', alg: { keyType: 'ed25519' }, passphrase: PK_PASSPHRASE },
    });
    expect(pkCreateRes.ok()).toBeTruthy();
    const tempKey = await pkCreateRes.json();

    // Export secret (encrypted) + private key together to get real crypto blobs.
    const helperExportRes = await page.request.post('/api/user/data/_export', {
      data: {
        include: {
          secrets: { type: 'all' },
          privateKeys: { type: 'selected', ids: [tempKey.id] },
        },
        secretsPassphrase: SECRETS_PASSPHRASE,
      },
    });
    expect(helperExportRes.ok()).toBeTruthy();
    const helperExport = await helperExportRes.json();

    const pkcs8Base64 = helperExport.data.privateKeys[0].pkcs8;
    const secretsEncryption = helperExport.secretsEncryption;
    const encryptedSecretValue = helperExport.data.secrets[0].encryptedValue;
    expect(secretsEncryption).toBeDefined();
    expect(encryptedSecretValue).toBeDefined();

    // Delete the originals so the import starts fresh.
    const exportedSecretId = helperExport.data.secrets[0].id;
    await page.request.delete(`/api/user/secrets/${encodeURIComponent(exportedSecretId)}`);
    await page.request.delete(`/api/certificates/private_keys/${encodeURIComponent(tempKey.id)}`);

    // ── Step 3: Get a user handle for webhook URLs ───────────────────────
    const stateRes = await page.request.get('/api/ui/state');
    const state = await stateRes.json();
    const userHandle = state.user.handle;

    // ── Step 4: Build the comprehensive import JSON ────────────────────
    const RESPONDER_SCRIPT = [
      '(() => {',
      '  return {',
      "    body: Deno.core.encode('secret:' + context.secrets.IMPORT_SECRET),",
      "    headers: { 'content-type': 'text/plain' },",
      '    statusCode: 200,',
      '    trackResponse: true',
      '  };',
      '})()',
    ].join('\n');

    const PAGE_EXTRACTOR_SCRIPT = [
      'export async function execute(p, context) {',
      "  const secret = context?.params?.secrets?.IMPORT_SECRET ?? 'NO_SECRET';",
      "  return '<p>secret:' + secret + '</p>';",
      '}',
    ].join('\n');

    const API_CONFIGURATOR_SCRIPT = ['(() => {', '  return { requests: context.requests };', '})()'].join('\n');

    const API_EXTRACTOR_SCRIPT = [
      '(() => {',
      '  const resp = context.responses?.[0];',
      "  const raw = resp?.body ? Deno.core.decode(new Uint8Array(resp.body)) : '{}';",
      "  const secret = context.params?.secrets?.IMPORT_SECRET ?? 'NO_SECRET';",
      '  return {',
      '    body: Deno.core.encode(JSON.stringify({',
      '      extracted: true,',
      '      secret: secret,',
      '      status: resp?.status ?? null',
      '    }, null, 2))',
      '  };',
      '})()',
    ].join('\n');

    const importFile = {
      version: 1,
      exportedAt: 1577880000,
      secretsEncryption,
      data: {
        tags: [
          {
            id: UUID_TAG_1,
            name: 'import-tag-prod',
            color: '#54B399',
            createdAt: 1577836800,
            updatedAt: 1577836800,
          },
          {
            id: UUID_TAG_2,
            name: 'import-tag-staging',
            color: '#6092C0',
            createdAt: 1577836800,
            updatedAt: 1577836800,
          },
        ],
        scripts: [
          {
            id: UUID_SCRIPT,
            name: 'import-test-script',
            scriptType: 'responder',
            content: "console.log('imported-script')",
            tags: [{ id: UUID_TAG_1, name: 'import-tag-prod', color: '#54B399' }],
            createdAt: 1577836800,
            updatedAt: 1577836800,
          },
        ],
        secrets: [
          {
            id: UUID_SECRET,
            name: SECRET_NAME,
            encryptedValue: encryptedSecretValue,
            createdAt: 1577836800,
            updatedAt: 1577836800,
          },
        ],
        contentSecurityPolicies: [
          {
            id: UUID_CSP,
            name: 'import-test-csp',
            directives: [
              { name: 'default-src', value: ["'self'"] },
              { name: 'script-src', value: ["'self'", 'https://cdn.example.com'] },
            ],
            createdAt: 1577836800,
            updatedAt: 1577836800,
          },
        ],
        certificateTemplates: [
          {
            id: UUID_CERT_TEMPLATE,
            name: 'import-test-cert-template',
            attributes: {
              commonName: 'Import Test CA',
              country: 'US',
              keyAlgorithm: { keyType: 'ed25519' },
              signatureAlgorithm: 'ed25519',
              notValidBefore: 1577836800,
              notValidAfter: 1893456000,
              version: 3,
              isCa: true,
              keyUsage: ['digitalSignature'],
              extendedKeyUsage: ['tlsWebServerAuthentication'],
            },
            createdAt: 1577836800,
            updatedAt: 1577836800,
          },
        ],
        privateKeys: [
          {
            id: UUID_PK,
            name: 'import-test-private-key',
            alg: { keyType: 'ed25519' },
            pkcs8: pkcs8Base64,
            encrypted: true,
            createdAt: 1577836800,
            updatedAt: 1577836800,
          },
        ],
        responders: [
          {
            id: UUID_RESPONDER,
            name: 'import-test-responder',
            location: { pathType: '=', path: '/import-resp-test' },
            method: 'GET',
            enabled: true,
            settings: {
              requestsToTrack: 10,
              statusCode: 200,
              script: RESPONDER_SCRIPT,
              secrets: { type: 'all' },
            },
            createdAt: 1577836800,
            updatedAt: 1577836800,
            history: [
              {
                id: UUID_RESP_HISTORY,
                responderId: UUID_RESPONDER,
                clientAddress: '172.18.0.1:12345',
                method: 'GET',
                url: '/import-resp-test',
                createdAt: 1577836800,
                responseStatusCode: 200,
                responseBody: 'pre-import-response',
              },
            ],
          },
          {
            id: UUID_RESPONDER_NO_HISTORY,
            name: 'import-test-responder-no-history',
            location: { pathType: '=', path: '/import-resp-no-hist' },
            method: 'POST',
            enabled: true,
            settings: { requestsToTrack: 5, statusCode: 204 },
            createdAt: 1577836800,
            updatedAt: 1577836800,
            // No history field - simulates export of a responder with empty history.
          },
        ],
        pageTrackers: [
          {
            id: UUID_PAGE_TRACKER,
            name: 'import-test-page-tracker',
            retrack: {
              enabled: true,
              config: { revisions: 3 },
              target: { type: 'page', extractor: PAGE_EXTRACTOR_SCRIPT },
              notifications: false,
            },
            secrets: { type: 'all' },
            createdAt: 1577836800,
            updatedAt: 1577836800,
            history: [
              {
                id: UUID_PAGE_REV,
                trackerId: UUID_PAGE_TRACKER,
                data: { original: '<p>imported-revision-content</p>' },
                createdAt: 1577836800,
              },
            ],
          },
        ],
        settings: {
          'common.uiTheme': 'dark',
          'common.showOnlyFavorites': true,
          'common.globalScopeTagIds': [UUID_TAG_1, UUID_TAG_2],
        },
        apiTrackers: [
          {
            id: UUID_API_TRACKER,
            name: 'import-test-api-tracker',
            retrack: {
              enabled: true,
              config: { revisions: 5 },
              target: {
                type: 'api',
                requests: [{ url: 'http://host.docker.internal:7171/api/ui/state', method: 'GET' }],
                configurator: API_CONFIGURATOR_SCRIPT,
                extractor: API_EXTRACTOR_SCRIPT,
              },
              notifications: false,
            },
            secrets: { type: 'all' },
            createdAt: 1577836800,
            updatedAt: 1577836800,
            history: [
              {
                id: UUID_API_REV_1,
                trackerId: UUID_API_TRACKER,
                data: { original: { revision: 1, data: 'first' } },
                createdAt: 1577836800,
              },
              {
                id: UUID_API_REV_2,
                trackerId: UUID_API_TRACKER,
                data: { original: { revision: 2, data: 'second' } },
                createdAt: 1577836900,
              },
            ],
          },
        ],
      },
    };

    // ── Step 5: Import preview ─────────────────────────────────────────
    const previewRes = await page.request.post('/api/user/data/_import_preview', {
      data: { data: importFile, mode: 'merge' },
    });
    if (!previewRes.ok()) {
      const errorBody = await previewRes.text();
      throw new Error(`Import preview failed (${previewRes.status()}): ${errorBody}`);
    }
    const preview = await previewRes.json();

    expect(preview.valid).toBe(true);
    expect(preview.summary.tags.total).toBe(2);
    expect(preview.summary.scripts.total).toBe(1);
    expect(preview.summary.secrets.total).toBe(1);
    expect(preview.summary.secrets.total).toBe(1);
    expect(preview.summary.responders.total).toBe(2);
    expect(preview.summary.certificateTemplates.total).toBe(1);
    expect(preview.summary.privateKeys.total).toBe(1);
    expect(preview.summary.contentSecurityPolicies.total).toBe(1);
    expect(preview.summary.pageTrackers.total).toBe(1);
    expect(preview.summary.apiTrackers.total).toBe(1);
    expect(preview.summary.settings.included).toBe(true);

    // ── Step 6: Execute import ─────────────────────────────────────────
    const importRes = await page.request.post('/api/user/data/_import', {
      data: {
        data: importFile,
        mode: 'merge',
        secretsPassphrase: SECRETS_PASSPHRASE,
        selections: {
          scripts: [{ sourceId: UUID_SCRIPT, action: 'import' }],
          secrets: [{ sourceId: UUID_SECRET, action: 'import' }],
          responders: [
            { sourceId: UUID_RESPONDER, action: 'import' },
            { sourceId: UUID_RESPONDER_NO_HISTORY, action: 'import' },
          ],
          certificateTemplates: [{ sourceId: UUID_CERT_TEMPLATE, action: 'import' }],
          privateKeys: [{ sourceId: UUID_PK, action: 'import' }],
          contentSecurityPolicies: [{ sourceId: UUID_CSP, action: 'import' }],
          pageTrackers: [{ sourceId: UUID_PAGE_TRACKER, action: 'import' }],
          apiTrackers: [{ sourceId: UUID_API_TRACKER, action: 'import' }],
          importSettings: true,
        },
      },
    });
    expect(importRes.ok()).toBeTruthy();
    const importResult = await importRes.json();

    expect(importResult.results.settings.imported).toBe(1);
    expect(importResult.results.tags.imported).toBe(2);
    expect(importResult.results.scripts.imported).toBe(1);
    expect(importResult.results.secrets.imported).toBe(1);
    expect(importResult.results.responders.imported).toBe(2);
    expect(importResult.results.certificateTemplates.imported).toBe(1);
    expect(importResult.results.privateKeys.imported).toBe(1);
    expect(importResult.results.contentSecurityPolicies.imported).toBe(1);
    expect(importResult.results.pageTrackers.imported).toBe(1);
    expect(importResult.results.apiTrackers.imported).toBe(1);

    // ── Step 7-tags: Validate imported tags ────────────────────────────
    const tagsRes = await page.request.get('/api/user/tags');
    expect(tagsRes.ok()).toBeTruthy();
    const tags = await tagsRes.json();
    const importedTag1 = tags.find((t: { name: string }) => t.name === 'import-tag-prod');
    const importedTag2 = tags.find((t: { name: string }) => t.name === 'import-tag-staging');
    expect(importedTag1).toBeDefined();
    expect(importedTag1.color).toBe('#54B399');
    expect(importedTag2).toBeDefined();
    expect(importedTag2.color).toBe('#6092C0');

    // ── Step 7-settings: Validate imported settings ────────────────────
    const settingsRes = await page.request.get('/api/user/settings');
    expect(settingsRes.ok()).toBeTruthy();
    const importedSettings = await settingsRes.json();
    expect(importedSettings['common.uiTheme']).toBe('dark');
    expect(importedSettings['common.showOnlyFavorites']).toBe(true);
    // globalScopeTagIds should contain the remapped (new) tag IDs, not the original UUIDs.
    const importedScopeTagIds = importedSettings['common.globalScopeTagIds'] as string[];
    expect(importedScopeTagIds).toHaveLength(2);
    expect(importedScopeTagIds).toContain(importedTag1.id);
    expect(importedScopeTagIds).toContain(importedTag2.id);
    // The original UUIDs from the file should NOT be present.
    expect(importedScopeTagIds).not.toContain(UUID_TAG_1);
    expect(importedScopeTagIds).not.toContain(UUID_TAG_2);

    // ── Step 7a: Validate script ───────────────────────────────────────
    const scriptsRes = await page.request.get('/api/user/scripts');
    const scripts = await scriptsRes.json();
    const importedScript = scripts.find((s: { name: string }) => s.name === 'import-test-script');
    expect(importedScript).toBeDefined();
    expect(importedScript.scriptType).toBe('responder');
    expect(importedScript.content).toBe("console.log('imported-script')");
    // Validate script has the remapped tag.
    expect(importedScript.tags).toHaveLength(1);
    expect(importedScript.tags[0].name).toBe('import-tag-prod');

    // ── Step 7b: Validate secret ───────────────────────────────────────
    const secretsRes = await page.request.get('/api/user/secrets');
    const secrets = await secretsRes.json();
    expect(secrets.find((s: { name: string }) => s.name === SECRET_NAME)).toBeDefined();

    // ── Step 7c: Validate CSP ──────────────────────────────────────────
    const cspRes = await page.request.get('/api/utils/web_security/csp');
    const csps = await cspRes.json();
    const importedCsp = csps.find((c: { name: string }) => c.name === 'import-test-csp');
    expect(importedCsp).toBeDefined();
    expect(importedCsp.directives).toHaveLength(2);
    expect(importedCsp.directives).toEqual(
      expect.arrayContaining([
        { name: 'default-src', value: ["'self'"] },
        { name: 'script-src', value: ["'self'", 'https://cdn.example.com'] },
      ]),
    );

    // ── Step 7d: Validate certificate template ─────────────────────────
    const templatesRes = await page.request.get('/api/certificates/templates');
    const templates = await templatesRes.json();
    const importedTemplate = templates.find((t: { name: string }) => t.name === 'import-test-cert-template');
    expect(importedTemplate).toBeDefined();
    expect(importedTemplate.attributes.commonName).toBe('Import Test CA');
    expect(importedTemplate.attributes.country).toBe('US');
    expect(importedTemplate.attributes.keyAlgorithm).toEqual({ keyType: 'ed25519' });
    expect(importedTemplate.attributes.signatureAlgorithm).toBe('ed25519');
    expect(importedTemplate.attributes.isCa).toBe(true);
    expect(importedTemplate.attributes.version).toBe(3);
    expect(importedTemplate.attributes.keyUsage).toEqual(['digitalSignature']);
    expect(importedTemplate.attributes.extendedKeyUsage).toEqual(['tlsWebServerAuthentication']);

    // ── Step 7e: Validate private key + passphrase export ──────────────
    const keysRes = await page.request.get('/api/certificates/private_keys');
    const keys = await keysRes.json();
    const importedKey = keys.find((k: { name: string }) => k.name === 'import-test-private-key');
    expect(importedKey).toBeDefined();
    expect(importedKey.alg).toEqual({ keyType: 'ed25519' });
    expect(importedKey.encrypted).toBe(true);

    // Export with the original passphrase to prove it's correct.
    const keyExportRes = await page.request.post(
      `/api/certificates/private_keys/${encodeURIComponent(importedKey.id)}/_export`,
      { data: { format: 'pkcs8', passphrase: PK_PASSPHRASE } },
    );
    expect(keyExportRes.ok()).toBeTruthy();

    // ── Step 7f: Validate responder config ─────────────────────────────
    const respondersRes = await page.request.get('/api/utils/webhooks/responders');
    const responders = await respondersRes.json();
    const importedResponder = responders.find((r: { name: string }) => r.name === 'import-test-responder');
    expect(importedResponder).toBeDefined();
    expect(importedResponder.location).toEqual({ pathType: '=', path: '/import-resp-test' });
    expect(importedResponder.method).toBe('GET');
    expect(importedResponder.enabled).toBe(true);
    expect(importedResponder.settings.statusCode).toBe(200);
    expect(importedResponder.settings.requestsToTrack).toBe(10);
    expect(importedResponder.settings.script).toContain('context.secrets.IMPORT_SECRET');
    expect(importedResponder.settings.secrets).toEqual({ type: 'all' });

    // ── Step 7f-2: Validate no-history responder config ──────────────────
    const importedResponderNoHistory = responders.find(
      (r: { name: string }) => r.name === 'import-test-responder-no-history',
    );
    expect(importedResponderNoHistory).toBeDefined();
    expect(importedResponderNoHistory.location).toEqual({ pathType: '=', path: '/import-resp-no-hist' });
    expect(importedResponderNoHistory.method).toBe('POST');
    expect(importedResponderNoHistory.enabled).toBe(true);
    expect(importedResponderNoHistory.settings.statusCode).toBe(204);
    expect(importedResponderNoHistory.settings.requestsToTrack).toBe(5);

    // Validate that the no-history responder has empty history.
    const noHistoryRes = await page.request.get(
      `/api/utils/webhooks/responders/${encodeURIComponent(importedResponderNoHistory.id)}/history`,
    );
    expect(noHistoryRes.ok()).toBeTruthy();
    const noHistory = await noHistoryRes.json();
    expect(noHistory).toHaveLength(0);

    // ── Step 7g: Validate imported responder history ────────────────────
    const historyRes = await page.request.get(
      `/api/utils/webhooks/responders/${encodeURIComponent(importedResponder.id)}/history`,
    );
    expect(historyRes.ok()).toBeTruthy();
    const history = await historyRes.json();
    expect(history).toHaveLength(1);
    expect(history[0].method).toBe('GET');
    expect(history[0].url).toContain('/import-resp-test');
    expect(history[0].responseStatusCode).toBe(200);
    // responseBody is returned as a byte array by the API.
    const importedResponseBody = new TextDecoder().decode(new Uint8Array(history[0].responseBody));
    expect(importedResponseBody).toBe('pre-import-response');

    // ── Step 7h: Call responder webhook + validate a new tracked entry ────
    const webhookUrl = `/api/webhooks/${userHandle}/import-resp-test`;
    const webhookRes = await page.request.fetch(webhookUrl, { method: 'GET' });
    expect(webhookRes.ok()).toBeTruthy();
    const webhookBody = await webhookRes.text();
    expect(webhookBody).toBe(`secret:${SECRET_VALUE}`);

    // Re-fetch history: should now have 2 entries.
    const historyAfterRes = await page.request.get(
      `/api/utils/webhooks/responders/${encodeURIComponent(importedResponder.id)}/history`,
    );
    const historyAfter = await historyAfterRes.json();
    expect(historyAfter).toHaveLength(2);
    // The newest entry should have the secret-based response (byte array).
    const newestEntry = historyAfter.find((e: { responseBody?: number[] }) => {
      if (!e.responseBody) return false;
      const body = new TextDecoder().decode(new Uint8Array(e.responseBody));
      return body === `secret:${SECRET_VALUE}`;
    });
    expect(newestEntry).toBeDefined();
    expect(newestEntry.responseStatusCode).toBe(200);

    // ── Step 7i: Validate page tracker config + imported revision ───────
    const pageTrackersRes = await page.request.get('/api/utils/web_scraping/page');
    const pageTrackers = await pageTrackersRes.json();
    const importedPageTracker = pageTrackers.find((t: { name: string }) => t.name === 'import-test-page-tracker');
    expect(importedPageTracker).toBeDefined();
    expect(importedPageTracker.secrets).toEqual({ type: 'all' });

    const pageHistoryUrl = `/api/utils/web_scraping/page/${encodeURIComponent(importedPageTracker.id)}/history`;
    const pageHistoryRes = await page.request.post(pageHistoryUrl, {
      data: { refresh: false },
    });
    expect(pageHistoryRes.ok()).toBeTruthy();
    const pageHistory = await pageHistoryRes.json();
    expect(pageHistory).toHaveLength(1);
    expect(pageHistory[0].data.original).toBe('<p>imported-revision-content</p>');

    // ── Step 7j: Trigger page tracker update + validate new revision ────
    const res = await page.request.post(pageHistoryUrl, {
      data: { refresh: true },
      timeout: 60000,
    });
    if (!res.ok()) {
      throw new Error(`Page tracker update failed after 3 attempts (${res.status()}): ${await res.text()}`);
    }

    const pageHistoryAfterRes = await page.request.post(pageHistoryUrl, {
      data: { refresh: false },
    });
    const pageHistoryAfter = await pageHistoryAfterRes.json();
    expect(pageHistoryAfter).toHaveLength(2);
    // The new revision should contain the secret value picked up from the imported secret.
    const newPageRevision = pageHistoryAfter.find((r: { data: { original: string } }) =>
      r.data.original?.includes(`secret:${SECRET_VALUE}`),
    );
    expect(newPageRevision).toBeDefined();

    // ── Step 7k: Validate API tracker config + imported revisions ───────
    const apiTrackersRes = await page.request.get('/api/utils/web_scraping/api');
    const apiTrackers = await apiTrackersRes.json();
    const importedApiTracker = apiTrackers.find((t: { name: string }) => t.name === 'import-test-api-tracker');
    expect(importedApiTracker).toBeDefined();
    expect(importedApiTracker.secrets).toEqual({ type: 'all' });

    const apiHistoryUrl = `/api/utils/web_scraping/api/${encodeURIComponent(importedApiTracker.id)}/history`;
    const apiHistoryRes = await page.request.post(apiHistoryUrl, {
      data: { refresh: false },
    });
    expect(apiHistoryRes.ok()).toBeTruthy();
    const apiHistory = await apiHistoryRes.json();
    expect(apiHistory).toHaveLength(2);
    const apiRevisionData = apiHistory.map((r: { data: { original: unknown } }) => r.data.original);
    expect(apiRevisionData).toEqual(
      expect.arrayContaining([
        { revision: 1, data: 'first' },
        { revision: 2, data: 'second' },
      ]),
    );

    // ── Step 7l: Trigger API tracker update + validate new revision ─────
    const apiUpdateRes = await page.request.post(apiHistoryUrl, {
      data: { refresh: true },
      timeout: 60000,
    });
    expect(apiUpdateRes.ok()).toBeTruthy();

    const apiHistoryAfterRes = await page.request.post(apiHistoryUrl, {
      data: { refresh: false },
    });
    const apiHistoryAfter = await apiHistoryAfterRes.json();
    expect(apiHistoryAfter).toHaveLength(3);
    // The newest revision should contain the secret value from the extractor.
    const newestApiRevision = apiHistoryAfter.find((r: { data: { original: unknown } }) => {
      const original = r.data.original;
      if (typeof original === 'object' && original !== null && 'secret' in original) {
        return (original as { secret: string }).secret === SECRET_VALUE;
      }
      // The extractor returns a JSON-encoded body, so the original might be a string.
      if (typeof original === 'string') {
        return original.includes(SECRET_VALUE);
      }
      return false;
    });
    expect(newestApiRevision).toBeDefined();
  });

  test('export-import round trip preserves tags and entity-tag associations', async ({ page }) => {
    // ── Step 1: Create tags ────────────────────────────────────────────
    const tag1Res = await page.request.post('/api/user/tags', {
      data: { name: 'e2e-production', color: '#e74c3c' },
    });
    expect(tag1Res.ok()).toBeTruthy();
    const tag1 = await tag1Res.json();

    const tag2Res = await page.request.post('/api/user/tags', {
      data: { name: 'e2e-staging', color: '#3498db' },
    });
    expect(tag2Res.ok()).toBeTruthy();
    const tag2 = await tag2Res.json();

    // ── Step 2: Create entities with tags ──────────────────────────────
    const scriptRes = await page.request.post('/api/user/scripts', {
      data: {
        name: 'tagged_script',
        scriptType: 'responder',
        content: 'console.log("tagged")',
        tagIds: [tag1.id, tag2.id],
      },
    });
    expect(scriptRes.ok()).toBeTruthy();
    const script = await scriptRes.json();
    expect(script.tags).toHaveLength(2);

    const cspRes = await page.request.post('/api/utils/web_security/csp', {
      data: {
        name: 'tagged_csp',
        content: { type: 'directives', value: [{ name: 'default-src', value: ["'self'"] }] },
        tagIds: [tag1.id],
      },
    });
    expect(cspRes.ok()).toBeTruthy();
    const csp = await cspRes.json();
    expect(csp.tags).toHaveLength(1);

    // ── Step 3: Export all data ────────────────────────────────────────
    const exportRes = await page.request.post('/api/user/data/_export', {
      data: {
        include: {
          tags: { type: 'all' },
          scripts: { type: 'selected', ids: [script.id] },
          contentSecurityPolicies: { type: 'selected', ids: [csp.id] },
        },
      },
    });
    expect(exportRes.ok()).toBeTruthy();
    const exportData = await exportRes.json();

    // Verify tags are in the export.
    expect(exportData.data.tags).toHaveLength(2);
    expect(exportData.data.tags.map((t: { name: string }) => t.name).sort()).toEqual(['e2e-production', 'e2e-staging']);

    // Verify entities carry tags.
    expect(exportData.data.scripts[0].tags).toHaveLength(2);
    // Note: CSP bulk export does not populate tags (bulk_get goes directly to DB
    // without tag population). This is a known limitation.

    // ── Step 4: Delete originals ───────────────────────────────────────
    await page.request.delete(`/api/user/scripts/${encodeURIComponent(script.id)}`);
    await page.request.delete(`/api/utils/web_security/csp/${encodeURIComponent(csp.id)}`);
    await page.request.delete(`/api/user/tags/${encodeURIComponent(tag1.id)}`);
    await page.request.delete(`/api/user/tags/${encodeURIComponent(tag2.id)}`);

    // Verify everything is gone.
    const tagsAfterDelete = await (await page.request.get('/api/user/tags')).json();
    expect(tagsAfterDelete.filter((t: { name: string }) => t.name.startsWith('e2e-'))).toHaveLength(0);

    // ── Step 5: Import ─────────────────────────────────────────────────
    const importRes = await page.request.post('/api/user/data/_import', {
      data: {
        data: exportData,
        mode: 'merge',
        selections: {
          scripts: [{ sourceId: exportData.data.scripts[0].id, action: 'import' }],
          secrets: [],
          responders: [],
          certificateTemplates: [],
          privateKeys: [],
          contentSecurityPolicies: [{ sourceId: exportData.data.contentSecurityPolicies[0].id, action: 'import' }],
          pageTrackers: [],
          apiTrackers: [],
        },
      },
    });
    expect(importRes.ok()).toBeTruthy();
    const importResult = await importRes.json();
    expect(importResult.results.tags.imported).toBe(2);
    expect(importResult.results.scripts.imported).toBe(1);
    expect(importResult.results.contentSecurityPolicies.imported).toBe(1);

    // ── Step 6: Verify tags restored ───────────────────────────────────
    const restoredTags = await (await page.request.get('/api/user/tags')).json();
    const e2eTags = restoredTags.filter((t: { name: string }) => t.name.startsWith('e2e-'));
    expect(e2eTags).toHaveLength(2);

    // ── Step 7: Verify entity-tag associations ─────────────────────────
    const restoredScripts = await (await page.request.get('/api/user/scripts')).json();
    const restoredScript = restoredScripts.find((s: { name: string }) => s.name === 'tagged_script');
    expect(restoredScript).toBeDefined();
    expect(restoredScript.tags).toHaveLength(2);

    const restoredCsps = await (await page.request.get('/api/utils/web_security/csp')).json();
    const restoredCsp = restoredCsps.find((c: { name: string }) => c.name === 'tagged_csp');
    expect(restoredCsp).toBeDefined();
    // Note: CSP list/bulk API does not populate tags (known limitation).
  });

  test('apply mode deletes tags not in import file', async ({ page }) => {
    // ── Step 1: Create tags ────────────────────────────────────────────
    const keepTagRes = await page.request.post('/api/user/tags', {
      data: { name: 'apply-keep', color: '#2ecc71' },
    });
    expect(keepTagRes.ok()).toBeTruthy();
    const keepTag = await keepTagRes.json();

    const deleteTagRes = await page.request.post('/api/user/tags', {
      data: { name: 'apply-delete', color: '#e67e22' },
    });
    expect(deleteTagRes.ok()).toBeTruthy();

    // ── Step 2: Build import file with only keep tag ───────────────────
    const importFile = {
      version: 1,
      exportedAt: 1577880000,
      data: {
        tags: [
          {
            id: keepTag.id,
            name: 'apply-keep',
            color: '#2ecc71',
            createdAt: 1577836800,
            updatedAt: 1577836800,
          },
        ],
      },
    };

    // ── Step 3: Preview should detect apply-delete tag for deletion ────
    const previewRes = await page.request.post('/api/user/data/_import_preview', {
      data: { data: importFile, mode: 'apply' },
    });
    expect(previewRes.ok()).toBeTruthy();
    const preview = await previewRes.json();
    expect(preview.toDelete.tags.find((t: { name: string }) => t.name === 'apply-delete')).toBeTruthy();

    // ── Step 4: Import with apply mode ─────────────────────────────────
    const importRes = await page.request.post('/api/user/data/_import', {
      data: {
        data: importFile,
        mode: 'apply',
        selections: {
          scripts: [],
          secrets: [],
          responders: [],
          certificateTemplates: [],
          privateKeys: [],
          contentSecurityPolicies: [],
          pageTrackers: [],
          apiTrackers: [],
        },
      },
    });
    expect(importRes.ok()).toBeTruthy();
    const result = await importRes.json();
    expect(result.results.tags.skipped).toBe(1);
    expect(result.results.tags.deleted).toBe(1);

    // ── Step 5: Verify only keep tag remains ───────────────────────────
    const tagsAfter = await (await page.request.get('/api/user/tags')).json();
    expect(tagsAfter.find((t: { name: string }) => t.name === 'apply-keep')).toBeTruthy();
    expect(tagsAfter.find((t: { name: string }) => t.name === 'apply-delete')).toBeUndefined();
  });

  test('conflict resolution options adapt to conflict type', async ({ page }) => {
    // ── Setup: create existing entities that will conflict ──────────────
    const scriptRes = await page.request.post('/api/user/scripts', {
      data: { name: 'conflict-script', scriptType: 'responder', content: 'console.log("existing")' },
    });
    expect(scriptRes.ok()).toBeTruthy();

    // Responder that will conflict by location+method only (different name).
    const responderRes = await page.request.post('/api/utils/webhooks/responders', {
      data: {
        name: 'existing-resp',
        location: { pathType: '=', path: '/conflict-path' },
        method: 'GET',
        enabled: true,
        settings: { requestsToTrack: 1, statusCode: 200 },
      },
    });
    expect(responderRes.ok()).toBeTruthy();

    // Responder that will conflict by BOTH name AND location+method (the typical
    // "import from your own account" scenario).
    const responderBothRes = await page.request.post('/api/utils/webhooks/responders', {
      data: {
        name: 'same-name-resp',
        location: { pathType: '=', path: '/both-conflict' },
        method: 'POST',
        enabled: true,
        settings: { requestsToTrack: 1, statusCode: 200 },
      },
    });
    expect(responderBothRes.ok()).toBeTruthy();

    // ── Build import file with conflicting entities ─────────────────────
    const importFile = {
      version: 1,
      exportedAt: 1577880000,
      data: {
        scripts: [
          {
            id: '019568f0-0000-7000-8000-000000000201',
            name: 'conflict-script', // Same name → name conflict, rename allowed
            scriptType: 'responder',
            content: 'console.log("imported")',
            createdAt: 1577836800,
            updatedAt: 1577836800,
          },
        ],
        responders: [
          {
            id: '019568f0-0000-7000-8000-000000000202',
            name: 'imported-resp', // Different name, same location+method → rename NOT allowed
            location: { pathType: '=', path: '/conflict-path' },
            method: 'GET',
            enabled: true,
            settings: { requestsToTrack: 1, statusCode: 200 },
            createdAt: 1577836800,
            updatedAt: 1577836800,
          },
          {
            id: '019568f0-0000-7000-8000-000000000203',
            name: 'same-name-resp', // Same name AND same location+method → rename NOT allowed
            location: { pathType: '=', path: '/both-conflict' },
            method: 'POST',
            enabled: true,
            settings: { requestsToTrack: 1, statusCode: 200 },
            createdAt: 1577836800,
            updatedAt: 1577836800,
          },
        ],
      },
    };

    // ── Step 1: Verify preview API returns correct renameAllowed flags ──
    const previewRes = await page.request.post('/api/user/data/_import_preview', {
      data: { data: importFile, mode: 'merge' },
    });
    expect(previewRes.ok()).toBeTruthy();
    const preview = await previewRes.json();
    expect(preview.valid).toBe(true);

    // Script conflict: rename is allowed (name-only conflict).
    expect(preview.summary.scripts.conflicts).toHaveLength(1);
    expect(preview.summary.scripts.conflicts[0].renameAllowed).toBeUndefined(); // true is omitted

    // Responder conflicts: rename is NOT allowed for either type.
    expect(preview.summary.responders.conflicts).toHaveLength(2);
    // Location-only conflict (different name, same path+method).
    const locOnlyConflict = preview.summary.responders.conflicts.find(
      (c: { sourceId: string }) => c.sourceId === '019568f0-0000-7000-8000-000000000202',
    );
    expect(locOnlyConflict.renameAllowed).toBe(false);
    // Both name AND location+method conflict - rename still NOT allowed.
    const bothConflict = preview.summary.responders.conflicts.find(
      (c: { sourceId: string }) => c.sourceId === '019568f0-0000-7000-8000-000000000203',
    );
    expect(bothConflict.renameAllowed).toBe(false);

    // ── Step 2: Open import modal and upload file ──────────────────────
    await page.getByRole('button', { name: 'Account menu' }).click();
    await page.getByText('Settings').click();
    const accountTab = page.getByRole('tab', { name: 'Account' });
    await expect(accountTab).toBeVisible({ timeout: 15000 });
    await accountTab.click();
    await page.getByRole('button', { name: 'Import data' }).click();

    const modal = page.locator('.euiModal[role="dialog"]');
    await expect(modal).toBeVisible({ timeout: 15000 });
    await expect(modal.getByText('Import data')).toBeVisible();

    // Create a temporary file and upload it via the file picker.
    const fileBuffer = Buffer.from(JSON.stringify(importFile));
    const filePicker = modal.locator('input[type="file"]');
    await filePicker.setInputFiles({
      name: 'test-conflicts.secutils.json',
      mimeType: 'application/json',
      buffer: fileBuffer,
    });

    // Click Preview.
    await modal.getByRole('button', { name: 'Preview' }).click();
    await expect(modal.getByText('Review import')).toBeVisible({ timeout: 15000 });

    // ── Step 3: Verify bulk Rename button is disabled ────────────────────
    const renameButton = modal.getByRole('button', { name: 'Rename' });
    await expect(renameButton).toBeVisible();
    await expect(renameButton).toBeDisabled();

    // Overwrite and Skip should be enabled.
    await expect(modal.getByRole('button', { name: 'Overwrite' })).toBeEnabled();
    await expect(modal.getByRole('button', { name: 'Skip' })).toBeEnabled();

    // ── Step 4: Verify help text about location+method conflicts ────────
    await expect(modal.getByText('Some conflicts cannot be resolved by renaming.')).toBeVisible();

    // ── Step 5: Expand Scripts section and check resolution options ─────
    const scriptsRow = modal.locator('tr', { has: page.getByText('Scripts') });
    await scriptsRow.getByRole('button', { name: 'Expand' }).click();

    // The script's resolution dropdown should have Rename, Overwrite, Skip.
    const scriptSelect = modal.locator('select').first();
    const scriptOptions = await scriptSelect.locator('option').allTextContents();
    expect(scriptOptions).toEqual(['Rename', 'Overwrite', 'Skip']);

    // ── Step 6: Expand Responders section and check resolution options ──
    const respondersRow = modal.locator('tr', { has: page.getByText('Responders') });
    await respondersRow.getByRole('button', { name: 'Expand' }).click();

    // Both responder dropdowns should NOT have Rename (location-only and name+location conflicts).
    const responderSelects = modal.locator('tr:has(td) select');
    // Skip the first select (script's), check the responder ones.
    const allSelects = await responderSelects.all();
    for (let i = 0; i < allSelects.length; i++) {
      // First select belongs to the script (expanded first).
      if (i === 0) continue;
      const options = await allSelects[i].locator('option').allTextContents();
      expect(options).toEqual(['Overwrite', 'Skip']);
    }

    // ── Step 7: Uncheck non-renameable responders → bulk Rename re-enabled ──
    // Click labels instead of using .uncheck() to avoid stale element issues
    // caused by React re-renders detaching/reattaching DOM elements.
    await modal.getByText('imported-resp').click();
    await modal.getByText('same-name-resp').click();

    // Now bulk Rename should be enabled (only script conflict remains, which is renameable).
    await expect(renameButton).toBeEnabled();

    // Help text about non-renameable conflicts should disappear.
    await expect(modal.getByText('Some conflicts cannot be resolved by renaming.')).not.toBeVisible();

    // Re-check the responders by clicking their labels (more reliable than .check()
    // which can conflict with React re-renders detaching/reattaching elements).
    await modal.getByText('imported-resp').click();
    await modal.getByText('same-name-resp').click();

    // Rename should be disabled again.
    await expect(renameButton).toBeDisabled();

    // ── Step 8: Import with Overwrite and verify success ────────────────
    // Click Overwrite in bulk to set all conflicts to overwrite.
    await modal.getByRole('button', { name: 'Overwrite' }).click();
    await modal.getByRole('button', { name: 'Import' }).click();

    // Wait for result.
    await expect(modal.getByText('Import complete')).toBeVisible({ timeout: 15000 });

    // Verify no errors.
    await expect(modal.getByText('Errors')).not.toBeVisible();
  });

  test('selective tag export and import filters entity tags', async ({ page }) => {
    // ── Step 1: Create tags ────────────────────────────────────────────
    const tagIds: Record<string, string> = {};
    for (const [name, color] of [
      ['alpha', '#54B399'],
      ['beta', '#6092C0'],
      ['gamma', '#D36086'],
    ] as const) {
      const res = await page.request.post('/api/user/tags', { data: { name, color } });
      expect(res.ok()).toBeTruthy();
      const tag = await res.json();
      tagIds[name] = tag.id;
    }

    // ── Step 2: Create a script with all 3 tags ────────────────────────
    const scriptRes = await page.request.post('/api/user/scripts', {
      data: {
        name: 'tagged-script',
        scriptType: 'responder',
        content: 'console.log("tagged")',
        tagIds: [tagIds['alpha'], tagIds['beta'], tagIds['gamma']],
      },
    });
    expect(scriptRes.ok()).toBeTruthy();
    const script = await scriptRes.json();
    expect(script.tags).toHaveLength(3);

    // ── Step 3: Export selecting only tags alpha + beta ─────────────────
    const exportRes = await page.request.post('/api/user/data/_export', {
      data: {
        include: {
          tags: { type: 'selected', ids: [tagIds['alpha'], tagIds['beta']] },
          scripts: { type: 'selected', ids: [script.id] },
        },
      },
    });
    expect(exportRes.ok()).toBeTruthy();
    const exportData = await exportRes.json();

    // Verify only alpha + beta in export tags.
    expect(exportData.data.tags).toHaveLength(2);
    const exportedTagNames = exportData.data.tags.map((t: { name: string }) => t.name).sort();
    expect(exportedTagNames).toEqual(['alpha', 'beta']);

    // Script still has all 3 entity-level tag refs in the file (stripping happens at import).
    expect(exportData.data.scripts).toHaveLength(1);

    // ── Step 4: Delete originals ───────────────────────────────────────
    await page.request.delete(`/api/user/scripts/${encodeURIComponent(script.id)}`);
    for (const id of Object.values(tagIds)) {
      await page.request.delete(`/api/user/tags/${encodeURIComponent(id)}`);
    }

    // ── Step 5: Import selecting only tag alpha ────────────────────────
    const alphaExportId = exportData.data.tags.find((t: { name: string }) => t.name === 'alpha').id;
    const betaExportId = exportData.data.tags.find((t: { name: string }) => t.name === 'beta').id;

    const importRes = await page.request.post('/api/user/data/_import', {
      data: {
        data: exportData,
        mode: 'merge',
        selections: {
          tags: [
            { sourceId: alphaExportId, action: 'import' },
            { sourceId: betaExportId, action: 'skip' },
          ],
          scripts: [{ sourceId: exportData.data.scripts[0].id, action: 'import' }],
          secrets: [],
          responders: [],
          certificateTemplates: [],
          privateKeys: [],
          contentSecurityPolicies: [],
          pageTrackers: [],
          apiTrackers: [],
        },
      },
    });
    expect(importRes.ok()).toBeTruthy();
    const importResult = await importRes.json();

    // Tag alpha imported, beta skipped.
    expect(importResult.results.tags.imported).toBe(1);
    expect(importResult.results.tags.skipped).toBe(1);
    expect(importResult.results.scripts.imported).toBe(1);

    // ── Step 6: Verify imported entities ────────────────────────────────
    // Only tag alpha should exist.
    const tagsRes = await page.request.get('/api/user/tags');
    const tags = await tagsRes.json();
    const importedTags = tags.filter((t: { name: string }) => ['alpha', 'beta', 'gamma'].includes(t.name));
    expect(importedTags).toHaveLength(1);
    expect(importedTags[0].name).toBe('alpha');

    // Script should have only tag alpha (beta was skipped, gamma wasn't in export).
    const scriptsRes = await page.request.get('/api/user/scripts');
    const scripts = await scriptsRes.json();
    const importedScript = scripts.find((s: { name: string }) => s.name === 'tagged-script');
    expect(importedScript).toBeDefined();
    expect(importedScript.tags).toHaveLength(1);
    expect(importedScript.tags[0].name).toBe('alpha');
  });

  test('import tag with rename conflict resolution creates copy', async ({ page }) => {
    // ── Step 1: Create a tag that will conflict ────────────────────────
    const tagRes = await page.request.post('/api/user/tags', { data: { name: 'conflict-tag', color: '#54B399' } });
    expect(tagRes.ok()).toBeTruthy();
    const existingTag = await tagRes.json();

    // ── Step 2: Create a script tagged with this tag ───────────────────
    const scriptRes = await page.request.post('/api/user/scripts', {
      data: {
        name: 'rename-test-script',
        scriptType: 'responder',
        content: 'console.log("rename-test")',
        tagIds: [existingTag.id],
      },
    });
    expect(scriptRes.ok()).toBeTruthy();
    const script = await scriptRes.json();

    // ── Step 3: Export with the tag ────────────────────────────────────
    const exportRes = await page.request.post('/api/user/data/_export', {
      data: {
        include: {
          tags: { type: 'all' },
          scripts: { type: 'selected', ids: [script.id] },
        },
      },
    });
    expect(exportRes.ok()).toBeTruthy();
    const exportData = await exportRes.json();
    expect(exportData.data.tags).toHaveLength(1);
    expect(exportData.data.tags[0].name).toBe('conflict-tag');

    // ── Step 4: Import with rename conflict resolution ─────────────────
    // The existing tag still exists, so this will trigger a name conflict.
    const importRes = await page.request.post('/api/user/data/_import', {
      data: {
        data: exportData,
        mode: 'merge',
        selections: {
          tags: [
            {
              sourceId: exportData.data.tags[0].id,
              action: 'import',
              conflictResolution: 'rename',
            },
          ],
          scripts: [{ sourceId: exportData.data.scripts[0].id, action: 'import', conflictResolution: 'rename' }],
          secrets: [],
          responders: [],
          certificateTemplates: [],
          privateKeys: [],
          contentSecurityPolicies: [],
          pageTrackers: [],
          apiTrackers: [],
        },
      },
    });
    expect(importRes.ok()).toBeTruthy();
    const importResult = await importRes.json();

    // Tag should be imported (renamed), not skipped.
    expect(importResult.results.tags.imported).toBe(1);
    expect(importResult.results.tags.skipped).toBe(0);
    expect(importResult.results.scripts.imported).toBe(1);

    // ── Step 5: Verify both tags exist ─────────────────────────────────
    const tagsRes = await page.request.get('/api/user/tags');
    const tags = await tagsRes.json();
    const conflictTags = tags.filter((t: { name: string }) => t.name.startsWith('conflict-tag'));
    expect(conflictTags).toHaveLength(2);
    expect(conflictTags.map((t: { name: string }) => t.name).sort()).toEqual(['conflict-tag', 'conflict-tag (copy 1)']);

    // The imported script should reference the renamed tag.
    const scriptsRes = await page.request.get('/api/user/scripts');
    const scripts = await scriptsRes.json();
    const renamedScript = scripts.find((s: { name: string }) => s.name === 'rename-test-script (Copy 1)');
    expect(renamedScript).toBeDefined();
    expect(renamedScript.tags).toHaveLength(1);
    expect(renamedScript.tags[0].name).toBe('conflict-tag (copy 1)');
  });
});
