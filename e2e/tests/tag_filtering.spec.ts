import { expect, test } from '@playwright/test';

import { ensureUserAndLogin, goto } from '../helpers';

test.describe('Tag filtering', () => {
  let tagIds: Record<string, string>;

  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);

    // Create three tags.
    tagIds = {};
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
  });

  test('global scope uses AND logic on responders', async ({ page }) => {
    // Create responders with different tag combinations.
    for (const [name, path, tags] of [
      ['R-alpha-beta', '/r-ab', [tagIds['alpha'], tagIds['beta']]],
      ['R-only-alpha', '/r-a', [tagIds['alpha']]],
      ['R-only-gamma', '/r-g', [tagIds['gamma']]],
    ] as const) {
      const res = await page.request.post('/api/utils/webhooks/responders', {
        data: {
          name,
          location: { pathType: '=', path },
          method: 'ANY',
          enabled: true,
          settings: { requestsToTrack: 10, statusCode: 200 },
          tagIds: tags,
        },
      });
      expect(res.ok()).toBeTruthy();
    }

    await goto(page, '/ws/webhooks__responders');
    await expect(page.getByText('R-alpha-beta')).toBeVisible({ timeout: 15000 });
    await expect(page.getByText('R-only-alpha')).toBeVisible();
    await expect(page.getByText('R-only-gamma')).toBeVisible();

    // Open global scope selector.
    const scopeButton = page.getByRole('button', { name: 'Filter all lists by tags' });
    await scopeButton.click();

    // Select "alpha" — shows R-alpha-beta and R-only-alpha (both have alpha).
    await page.getByRole('option', { name: 'alpha' }).click();
    await page.keyboard.press('Escape');
    await expect(page.getByText('R-alpha-beta')).toBeVisible();
    await expect(page.getByText('R-only-alpha')).toBeVisible();
    await expect(page.getByText('R-only-gamma')).not.toBeVisible();

    // Also select "beta" (AND logic) — only R-alpha-beta has BOTH alpha AND beta.
    await scopeButton.click();
    await page.getByRole('option', { name: 'beta' }).click();
    await page.keyboard.press('Escape');
    await expect(page.getByText('R-alpha-beta')).toBeVisible();
    await expect(page.getByText('R-only-alpha')).not.toBeVisible();
    await expect(page.getByText('R-only-gamma')).not.toBeVisible();

    // Deselect both — all visible again.
    await scopeButton.click();
    await page.getByRole('option', { name: 'alpha' }).click();
    await page.getByRole('option', { name: 'beta' }).click();
    await page.keyboard.press('Escape');
    await expect(page.getByText('R-alpha-beta')).toBeVisible();
    await expect(page.getByText('R-only-alpha')).toBeVisible();
    await expect(page.getByText('R-only-gamma')).toBeVisible();
  });

  test('page-level filter uses OR logic on responders', async ({ page }) => {
    for (const [name, path, tags] of [
      ['R-only-alpha', '/r-a', [tagIds['alpha']]],
      ['R-only-beta', '/r-b', [tagIds['beta']]],
      ['R-only-gamma', '/r-g', [tagIds['gamma']]],
    ] as const) {
      const res = await page.request.post('/api/utils/webhooks/responders', {
        data: {
          name,
          location: { pathType: '=', path },
          method: 'ANY',
          enabled: true,
          settings: { requestsToTrack: 10, statusCode: 200 },
          tagIds: tags,
        },
      });
      expect(res.ok()).toBeTruthy();
    }

    await goto(page, '/ws/webhooks__responders');
    await expect(page.getByText('R-only-alpha')).toBeVisible({ timeout: 15000 });

    // Open page-level tag filter.
    const tagFilter = page.getByRole('button', { name: /Tags/ });
    await tagFilter.click();

    // Select alpha and gamma (OR logic) — R-only-alpha and R-only-gamma visible, R-only-beta hidden.
    await page.getByRole('option', { name: 'alpha' }).click();
    await page.getByRole('option', { name: 'gamma' }).click();
    await page.keyboard.press('Escape');
    await expect(page.getByText('R-only-alpha')).toBeVisible();
    await expect(page.getByText('R-only-gamma')).toBeVisible();
    await expect(page.getByText('R-only-beta')).not.toBeVisible();
  });

  test('global AND page-level filters stack correctly', async ({ page }) => {
    for (const [name, path, tags] of [
      ['R-alpha-beta', '/r-ab', [tagIds['alpha'], tagIds['beta']]],
      ['R-alpha-gamma', '/r-ag', [tagIds['alpha'], tagIds['gamma']]],
      ['R-only-alpha', '/r-a', [tagIds['alpha']]],
      ['R-only-gamma', '/r-g', [tagIds['gamma']]],
    ] as const) {
      const res = await page.request.post('/api/utils/webhooks/responders', {
        data: {
          name,
          location: { pathType: '=', path },
          method: 'ANY',
          enabled: true,
          settings: { requestsToTrack: 10, statusCode: 200 },
          tagIds: tags,
        },
      });
      expect(res.ok()).toBeTruthy();
    }

    await goto(page, '/ws/webhooks__responders');
    await expect(page.getByText('R-alpha-beta')).toBeVisible({ timeout: 15000 });

    // Set global scope to "alpha" — shows R-alpha-beta, R-alpha-gamma, R-only-alpha.
    const scopeButton = page.getByRole('button', { name: 'Filter all lists by tags' });
    await scopeButton.click();
    await page.getByRole('option', { name: 'alpha' }).click();
    await page.keyboard.press('Escape');
    await expect(page.getByText('R-alpha-beta')).toBeVisible();
    await expect(page.getByText('R-alpha-gamma')).toBeVisible();
    await expect(page.getByText('R-only-alpha')).toBeVisible();
    await expect(page.getByText('R-only-gamma')).not.toBeVisible();

    // Set page-level filter to "beta" (OR within global-filtered results).
    // Only R-alpha-beta passes both: has alpha (global AND) and has beta (page OR).
    const tagFilter = page.getByRole('button', { name: /Tags/ });
    await tagFilter.click();
    // Use .last() because the global scope popover may still have options in the DOM.
    await page.getByRole('option', { name: 'beta' }).last().click();
    await page.keyboard.press('Escape');
    await expect(page.getByText('R-alpha-beta')).toBeVisible();
    await expect(page.getByText('R-alpha-gamma')).not.toBeVisible();
    await expect(page.getByText('R-only-alpha')).not.toBeVisible();
  });

  test('global scope selection persists across page refresh', async ({ page }) => {
    // Create responders with different tag combinations.
    for (const [name, path, tags] of [
      ['R-alpha-beta', '/r-ab', [tagIds['alpha'], tagIds['beta']]],
      ['R-only-alpha', '/r-a', [tagIds['alpha']]],
      ['R-only-gamma', '/r-g', [tagIds['gamma']]],
    ] as const) {
      const res = await page.request.post('/api/utils/webhooks/responders', {
        data: {
          name,
          location: { pathType: '=', path },
          method: 'ANY',
          enabled: true,
          settings: { requestsToTrack: 10, statusCode: 200 },
          tagIds: tags,
        },
      });
      expect(res.ok()).toBeTruthy();
    }

    await goto(page, '/ws/webhooks__responders');
    await expect(page.getByText('R-alpha-beta')).toBeVisible({ timeout: 15000 });

    // Select "alpha" in global scope.
    const scopeButton = page.getByRole('button', { name: 'Filter all lists by tags' });
    await scopeButton.click();
    await page.getByRole('option', { name: 'alpha' }).click();
    await page.keyboard.press('Escape');

    // Verify filter is active before refresh.
    await expect(page.getByText('R-alpha-beta')).toBeVisible();
    await expect(page.getByText('R-only-alpha')).toBeVisible();
    await expect(page.getByText('R-only-gamma')).not.toBeVisible();

    // Hard refresh the page.
    await goto(page, '/ws/webhooks__responders');

    // After refresh, the filter should still be active.
    await expect(page.getByText('R-alpha-beta')).toBeVisible({ timeout: 15000 });
    await expect(page.getByText('R-only-alpha')).toBeVisible();
    await expect(page.getByText('R-only-gamma')).not.toBeVisible();
  });

  test('page filters use readable URL params and clear filters works across all pages', async ({ page }) => {
    // Create entities for every utility type tagged with "alpha" and "beta".
    await page.request.post('/api/utils/webhooks/responders', {
      data: {
        name: 'Resp-alpha',
        location: { pathType: '=', path: '/resp-a' },
        method: 'ANY',
        enabled: true,
        settings: { requestsToTrack: 10, statusCode: 200 },
        tagIds: [tagIds['alpha']],
      },
    });
    await page.request.post('/api/utils/webhooks/responders', {
      data: {
        name: 'Resp-beta',
        location: { pathType: '=', path: '/resp-b' },
        method: 'ANY',
        enabled: true,
        settings: { requestsToTrack: 10, statusCode: 200 },
        tagIds: [tagIds['beta']],
      },
    });

    await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'PT-alpha',
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
        tagIds: [tagIds['alpha']],
      },
    });
    await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'PT-beta',
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
        tagIds: [tagIds['beta']],
      },
    });

    await page.request.post('/api/utils/web_scraping/api', {
      data: {
        name: 'AT-alpha',
        config: { revisions: 3 },
        target: {
          url: 'https://example.com',
          extractor:
            '(() => { const r = context.responses ?? []; return { body: Deno.core.encode(JSON.stringify(r)) }; })();',
        },
        secrets: { type: 'all' },
        tagIds: [tagIds['alpha']],
      },
    });
    await page.request.post('/api/utils/web_scraping/api', {
      data: {
        name: 'AT-beta',
        config: { revisions: 3 },
        target: {
          url: 'https://example.com',
          extractor:
            '(() => { const r = context.responses ?? []; return { body: Deno.core.encode(JSON.stringify(r)) }; })();',
        },
        secrets: { type: 'all' },
        tagIds: [tagIds['beta']],
      },
    });

    await page.request.post('/api/utils/certificates/private_keys', {
      data: { keyName: 'PK-alpha', alg: { keyType: 'ed25519' }, tagIds: [tagIds['alpha']] },
    });
    await page.request.post('/api/utils/certificates/private_keys', {
      data: { keyName: 'PK-beta', alg: { keyType: 'ed25519' }, tagIds: [tagIds['beta']] },
    });

    const now = Math.floor(Date.now() / 1000);
    await page.request.post('/api/utils/certificates/templates', {
      data: {
        templateName: 'CT-alpha',
        attributes: {
          commonName: 'test.example.com',
          keyAlgorithm: { keyType: 'ed25519' },
          signatureAlgorithm: 'ed25519',
          notValidBefore: now,
          notValidAfter: now + 86400 * 365,
          isCa: false,
        },
        tagIds: [tagIds['alpha']],
      },
    });
    await page.request.post('/api/utils/certificates/templates', {
      data: {
        templateName: 'CT-beta',
        attributes: {
          commonName: 'test.example.com',
          keyAlgorithm: { keyType: 'ed25519' },
          signatureAlgorithm: 'ed25519',
          notValidBefore: now,
          notValidAfter: now + 86400 * 365,
          isCa: false,
        },
        tagIds: [tagIds['beta']],
      },
    });

    await page.request.post('/api/utils/web_security/csp', {
      data: {
        name: 'CSP-alpha',
        content: { type: 'directives', value: [{ name: 'default-src', value: ["'self'"] }] },
        tagIds: [tagIds['alpha']],
      },
    });
    await page.request.post('/api/utils/web_security/csp', {
      data: {
        name: 'CSP-beta',
        content: { type: 'directives', value: [{ name: 'default-src', value: ["'self'"] }] },
        tagIds: [tagIds['beta']],
      },
    });

    // Helper: verify page-level tag filter and search query for a given workspace page.
    async function verifyPageFilters(url: string, alphaName: string, betaName: string) {
      await goto(page, url);
      await expect(page.getByText(alphaName)).toBeVisible({ timeout: 15000 });
      await expect(page.getByText(betaName)).toBeVisible();

      // 1. Select "alpha" in page-level tag filter.
      const tagFilter = page.getByRole('button', { name: /Tags/ });
      await tagFilter.click();
      await page.getByRole('option', { name: 'alpha' }).click();
      await page.keyboard.press('Escape');
      await expect(page.getByText(alphaName)).toBeVisible();
      await expect(page.getByText(betaName)).not.toBeVisible();

      // Verify URL contains tag name (not UUID).
      expect(page.url()).toContain('tags=alpha');
      expect(page.url()).not.toContain(tagIds['alpha']);

      // 2. Type a search query.
      const searchInput = page.getByRole('searchbox', { name: 'Search' });
      await searchInput.fill(alphaName);
      await page.waitForTimeout(200); // debounce
      expect(page.url()).toContain(`q=${encodeURIComponent(alphaName)}`);

      // 3. Apply both filters — then click "Clear filters" from filtered empty state.
      // First make the search hide everything by searching for a non-existent term.
      await searchInput.fill('zzz-no-match');
      await page.waitForTimeout(200);
      expect(page.url()).toContain('q=zzz-no-match');
      expect(page.url()).toContain('tags=alpha');

      // The "No matching items" empty state should appear with "Clear filters" button.
      const clearButton = page.getByRole('button', { name: 'Clear filters' });
      await expect(clearButton).toBeVisible({ timeout: 5000 });
      await clearButton.click();

      // After clearing, URL should have no q or tags params.
      await expect(page.getByText(alphaName)).toBeVisible({ timeout: 5000 });
      await expect(page.getByText(betaName)).toBeVisible();
      expect(page.url()).not.toContain('q=');
      expect(page.url()).not.toContain('tags=');
    }

    // Verify on all workspace utility pages.
    await verifyPageFilters('/ws/webhooks__responders', 'Resp-alpha', 'Resp-beta');
    await verifyPageFilters('/ws/web_scraping__page', 'PT-alpha', 'PT-beta');
    await verifyPageFilters('/ws/web_scraping__api', 'AT-alpha', 'AT-beta');
    await verifyPageFilters('/ws/certificates__private_keys', 'PK-alpha', 'PK-beta');
    await verifyPageFilters('/ws/certificates__certificate_templates', 'CT-alpha', 'CT-beta');
    await verifyPageFilters('/ws/web_security__csp__policies', 'CSP-alpha', 'CSP-beta');
  });

  test('global filter applies to all workspace utility pages', async ({ page }) => {
    // Create one entity per utility type, all tagged with "alpha".
    // Create one entity per type tagged with "gamma" (should be filtered out).

    // Responders.
    for (const [name, path, tags] of [
      ['Resp-alpha', '/resp-a', [tagIds['alpha']]],
      ['Resp-gamma', '/resp-g', [tagIds['gamma']]],
    ] as const) {
      await page.request.post('/api/utils/webhooks/responders', {
        data: {
          name,
          location: { pathType: '=', path },
          method: 'ANY',
          enabled: true,
          settings: { requestsToTrack: 10, statusCode: 200 },
          tagIds: tags,
        },
      });
    }

    // Page trackers.
    for (const [name, tags] of [
      ['PT-alpha', [tagIds['alpha']]],
      ['PT-gamma', [tagIds['gamma']]],
    ] as const) {
      await page.request.post('/api/utils/web_scraping/page', {
        data: {
          name,
          config: { revisions: 3 },
          target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
          tagIds: tags,
        },
      });
    }

    // API trackers.
    for (const [name, tags] of [
      ['AT-alpha', [tagIds['alpha']]],
      ['AT-gamma', [tagIds['gamma']]],
    ] as const) {
      await page.request.post('/api/utils/web_scraping/api', {
        data: {
          name,
          config: { revisions: 3 },
          target: {
            url: 'https://example.com',
            extractor:
              '(() => { const r = context.responses ?? []; return { body: Deno.core.encode(JSON.stringify(r)) }; })();',
          },
          secrets: { type: 'all' },
          tagIds: tags,
        },
      });
    }

    // Private keys.
    for (const [name, tags] of [
      ['PK-alpha', [tagIds['alpha']]],
      ['PK-gamma', [tagIds['gamma']]],
    ] as const) {
      await page.request.post('/api/utils/certificates/private_keys', {
        data: { keyName: name, alg: { keyType: 'ed25519' }, tagIds: tags },
      });
    }

    // Certificate templates.
    for (const [name, tags] of [
      ['CT-alpha', [tagIds['alpha']]],
      ['CT-gamma', [tagIds['gamma']]],
    ] as const) {
      const now = Math.floor(Date.now() / 1000);
      const ctRes = await page.request.post('/api/utils/certificates/templates', {
        data: {
          templateName: name,
          attributes: {
            commonName: 'test.example.com',
            keyAlgorithm: { keyType: 'ed25519' },
            signatureAlgorithm: 'ed25519',
            notValidBefore: now,
            notValidAfter: now + 86400 * 365,
            isCa: false,
          },
          tagIds: tags,
        },
      });
      expect(ctRes.ok()).toBeTruthy();
    }

    // CSP policies.
    for (const [name, tags] of [
      ['CSP-alpha', [tagIds['alpha']]],
      ['CSP-gamma', [tagIds['gamma']]],
    ] as const) {
      await page.request.post('/api/utils/web_security/csp', {
        data: {
          name,
          content: { type: 'directives', value: [{ name: 'default-src', value: ["'self'"] }] },
          tagIds: tags,
        },
      });
    }

    // Secrets.
    for (const [name, tags] of [
      ['Secret-alpha', [tagIds['alpha']]],
      ['Secret-gamma', [tagIds['gamma']]],
    ] as const) {
      await page.request.post('/api/user/secrets', {
        data: { name, value: 'test-value', tagIds: tags },
      });
    }

    // Scripts.
    for (const [name, tags] of [
      ['Script-alpha', [tagIds['alpha']]],
      ['Script-gamma', [tagIds['gamma']]],
    ] as const) {
      await page.request.post('/api/user/scripts', {
        data: {
          name,
          scriptType: 'responder',
          content: '(() => { return { statusCode: 200, body: "test" }; })();',
          tagIds: tags,
        },
      });
    }

    // Navigate to responders and set global scope to "alpha".
    await goto(page, '/ws/webhooks__responders');
    await expect(page.getByText('Resp-alpha')).toBeVisible({ timeout: 15000 });

    const scopeButton = page.getByRole('button', { name: 'Filter all lists by tags' });
    await scopeButton.click();
    await page.getByRole('option', { name: 'alpha' }).click();
    await page.keyboard.press('Escape');

    // Verify on responders page.
    await expect(page.getByText('Resp-alpha')).toBeVisible();
    await expect(page.getByText('Resp-gamma')).not.toBeVisible();

    // Navigate using client-side links (preserves React state including global scope).
    // Use href-based selectors to reliably find the correct sidebar links.

    // Verify on page trackers.
    await page.locator('a[href*="web_scraping__page"]').click();
    await expect(page.getByText('PT-alpha')).toBeVisible({ timeout: 15000 });
    await expect(page.getByText('PT-gamma')).not.toBeVisible();

    // Verify on API trackers.
    await page.locator('a[href*="web_scraping__api"]').click();
    await expect(page.getByText('AT-alpha')).toBeVisible({ timeout: 15000 });
    await expect(page.getByText('AT-gamma')).not.toBeVisible();

    // Verify on private keys.
    await page.locator('a[href*="certificates__private_keys"]').click();
    await expect(page.getByText('PK-alpha')).toBeVisible({ timeout: 15000 });
    await expect(page.getByText('PK-gamma')).not.toBeVisible();

    // Verify on certificate templates.
    await page.locator('a[href*="certificates__certificate_templates"]').click();
    await expect(page.getByText('CT-alpha')).toBeVisible({ timeout: 15000 });
    await expect(page.getByText('CT-gamma')).not.toBeVisible();

    // Verify on CSP policies (nested under Web Security → CSP button → Policies link).
    await page.getByRole('button', { name: 'CSP' }).click();
    await page.locator('a[href*="web_security__csp__policies"]').click();
    await expect(page.getByText('CSP-alpha')).toBeVisible({ timeout: 15000 });
    await expect(page.getByText('CSP-gamma')).not.toBeVisible();

    // Verify on secrets tab (via settings flyout, now inside WorkspaceContext).
    await page.getByRole('button', { name: 'Account menu' }).click();
    await page.getByText('Settings').click();
    const secretsTab = page.getByRole('tab', { name: 'Secrets' });
    await expect(secretsTab).toBeVisible({ timeout: 15000 });
    await secretsTab.click();
    await expect(page.getByText('Secret-alpha')).toBeVisible({ timeout: 15000 });
    await expect(page.getByText('Secret-gamma')).not.toBeVisible();

    // Verify on scripts tab.
    const scriptsTab = page.getByRole('tab', { name: 'Scripts' });
    await scriptsTab.click();
    await expect(page.getByText('Script-alpha')).toBeVisible({ timeout: 15000 });
    await expect(page.getByText('Script-gamma')).not.toBeVisible();
  });
});
