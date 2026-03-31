import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

test.describe('Sharing', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test.describe('Certificate Templates', () => {
    test('shared template is accessible without authentication', async ({ page, request }) => {
      // 1. Create a certificate template.
      const createRes = await page.request.post('/api/certificates/templates', {
        data: {
          templateName: 'Shared Template',
          attributes: {
            commonName: 'test.example.com',
            keyAlgorithm: { keyType: 'ed25519' },
            signatureAlgorithm: 'ed25519',
            notValidBefore: 946720800,
            notValidAfter: 1893456000,
            version: 3,
            isCa: false,
          },
        },
      });
      expect(createRes.ok()).toBeTruthy();
      const template = await createRes.json();

      // 2. Share the template.
      const shareRes = await page.request.post(`/api/certificates/templates/${template.id}/_share`);
      expect(shareRes.ok()).toBeTruthy();
      const share = await shareRes.json();
      expect(share.id).toBeTruthy();

      // 3. Verify the template is accessible via share ID without authentication.
      const sharedGetRes = await request.get(`/api/certificates/templates/${template.id}`, {
        headers: { 'x-user-share-id': share.id },
      });
      expect(sharedGetRes.ok()).toBeTruthy();
      const sharedData = await sharedGetRes.json();
      expect(sharedData.template.id).toBe(template.id);
      expect(sharedData.template.name).toBe('Shared Template');
      expect(sharedData.userShare).toBeTruthy();
      expect(sharedData.userShare.id).toBe(share.id);

      // 4. Verify _generate is accessible via share ID without authentication.
      const generateRes = await request.post(`/api/certificates/templates/${template.id}/_generate`, {
        headers: {
          'x-user-share-id': share.id,
          'content-type': 'application/json',
        },
        data: { format: 'pem' },
      });
      expect(generateRes.ok()).toBeTruthy();

      // 5. Verify the template is NOT accessible without auth or share header.
      const noAuthRes = await request.get(`/api/certificates/templates/${template.id}`);
      expect(noAuthRes.ok()).toBeFalsy();

      // 6. Unshare the template.
      const unshareRes = await page.request.post(`/api/certificates/templates/${template.id}/_unshare`);
      expect(unshareRes.ok()).toBeTruthy();

      // 7. Verify the share ID no longer works.
      const revokedRes = await request.get(`/api/certificates/templates/${template.id}`, {
        headers: { 'x-user-share-id': share.id },
      });
      expect(revokedRes.ok()).toBeFalsy();

      // Clean up.
      await page.request.delete(`/api/certificates/templates/${template.id}`);
    });

    test('shared template is accessible by a different authenticated user', async ({ page, browser }) => {
      // 1. Create a certificate template as the first user.
      const createRes = await page.request.post('/api/certificates/templates', {
        data: {
          templateName: 'Cross-User Shared Template',
          attributes: {
            commonName: 'cross-user.example.com',
            keyAlgorithm: { keyType: 'ed25519' },
            signatureAlgorithm: 'ed25519',
            notValidBefore: 946720800,
            notValidAfter: 1893456000,
            version: 3,
            isCa: false,
          },
        },
      });
      expect(createRes.ok()).toBeTruthy();
      const template = await createRes.json();

      // 2. Share the template.
      const shareRes = await page.request.post(`/api/certificates/templates/${template.id}/_share`);
      expect(shareRes.ok()).toBeTruthy();
      const share = await shareRes.json();

      // 3. Create a second user in a new browser context.
      const context2 = await browser.newContext({
        baseURL: process.env.BASE_URL ?? 'http://localhost:7171',
      });
      const page2 = await context2.newPage();
      await ensureUserAndLogin(context2.request, page2);

      // 4. Access the shared template as the second user using share header.
      const sharedGetRes = await page2.request.get(`/api/certificates/templates/${template.id}`, {
        headers: { 'x-user-share-id': share.id },
      });
      expect(sharedGetRes.ok()).toBeTruthy();
      const sharedData = await sharedGetRes.json();
      expect(sharedData.template.id).toBe(template.id);
      expect(sharedData.template.name).toBe('Cross-User Shared Template');

      // Clean up.
      await context2.close();
      await page.request.post(`/api/certificates/templates/${template.id}/_unshare`);
      await page.request.delete(`/api/certificates/templates/${template.id}`);
    });
  });

  test.describe('Content Security Policies', () => {
    test('shared policy is accessible without authentication', async ({ page, request }) => {
      // 1. Create a CSP policy.
      const createRes = await page.request.post('/api/web_security/csp', {
        data: {
          name: 'Shared CSP',
          content: {
            type: 'serialized',
            value: "default-src 'self'; script-src 'none'",
          },
        },
      });
      expect(createRes.ok()).toBeTruthy();
      const policy = await createRes.json();

      // 2. Share the policy.
      const shareRes = await page.request.post(`/api/web_security/csp/${policy.id}/_share`);
      expect(shareRes.ok()).toBeTruthy();
      const share = await shareRes.json();
      expect(share.id).toBeTruthy();

      // 3. Verify the policy is accessible via share ID without authentication.
      const sharedGetRes = await request.get(`/api/web_security/csp/${policy.id}`, {
        headers: { 'x-user-share-id': share.id },
      });
      expect(sharedGetRes.ok()).toBeTruthy();
      const sharedData = await sharedGetRes.json();
      expect(sharedData.policy.id).toBe(policy.id);
      expect(sharedData.policy.name).toBe('Shared CSP');
      expect(sharedData.userShare).toBeTruthy();
      expect(sharedData.userShare.id).toBe(share.id);

      // 4. Verify _serialize is accessible via share ID without authentication.
      const serializeRes = await request.post(`/api/web_security/csp/${policy.id}/_serialize`, {
        headers: {
          'x-user-share-id': share.id,
          'content-type': 'application/json',
        },
        data: { source: 'enforcingHeader' },
      });
      expect(serializeRes.ok()).toBeTruthy();
      const serialized = await serializeRes.json();
      expect(serialized).toContain("default-src 'self'");

      // 5. Verify the policy is NOT accessible without auth or share header.
      const noAuthRes = await request.get(`/api/web_security/csp/${policy.id}`);
      expect(noAuthRes.ok()).toBeFalsy();

      // 6. Unshare the policy.
      const unshareRes = await page.request.post(`/api/web_security/csp/${policy.id}/_unshare`);
      expect(unshareRes.ok()).toBeTruthy();

      // 7. Verify the share ID no longer works.
      const revokedRes = await request.get(`/api/web_security/csp/${policy.id}`, {
        headers: { 'x-user-share-id': share.id },
      });
      expect(revokedRes.ok()).toBeFalsy();

      // Clean up.
      await page.request.delete(`/api/web_security/csp/${policy.id}`);
    });

    test('shared policy is accessible by a different authenticated user', async ({ page, browser }) => {
      // 1. Create a CSP policy as the first user.
      const createRes = await page.request.post('/api/web_security/csp', {
        data: {
          name: 'Cross-User Shared CSP',
          content: {
            type: 'serialized',
            value: "default-src 'self'",
          },
        },
      });
      expect(createRes.ok()).toBeTruthy();
      const policy = await createRes.json();

      // 2. Share the policy.
      const shareRes = await page.request.post(`/api/web_security/csp/${policy.id}/_share`);
      expect(shareRes.ok()).toBeTruthy();
      const share = await shareRes.json();

      // 3. Create a second user in a new browser context.
      const context2 = await browser.newContext({
        baseURL: process.env.BASE_URL ?? 'http://localhost:7171',
      });
      const page2 = await context2.newPage();
      await ensureUserAndLogin(context2.request, page2);

      // 4. Access the shared policy as the second user using share header.
      const sharedGetRes = await page2.request.get(`/api/web_security/csp/${policy.id}`, {
        headers: { 'x-user-share-id': share.id },
      });
      expect(sharedGetRes.ok()).toBeTruthy();
      const sharedData = await sharedGetRes.json();
      expect(sharedData.policy.id).toBe(policy.id);
      expect(sharedData.policy.name).toBe('Cross-User Shared CSP');

      // Clean up.
      await context2.close();
      await page.request.post(`/api/web_security/csp/${policy.id}/_unshare`);
      await page.request.delete(`/api/web_security/csp/${policy.id}`);
    });
  });
});
