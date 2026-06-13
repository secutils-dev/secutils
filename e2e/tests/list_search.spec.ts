import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

// Regression guard for the "nondeterministic collations are not supported for ILIKE" 500.
// Several entity name columns inherit the database's default (nondeterministic) ICU collation,
// which Postgres rejects for ILIKE. The shared pagination `WHERE` clause forces `COLLATE "C"`
// so case-insensitive search works uniformly. This spec hits every paginated list endpoint with
// a search query (using the default `sort=updatedAt`, exactly as the UI does on first load) and
// asserts the request never 500s and returns the paginated `{ items, total }` shape.
const LIST_ENDPOINTS = [
  '/api/web_scraping/page_trackers',
  '/api/web_scraping/api_trackers',
  '/api/webhooks/responders',
  '/api/certificates/templates',
  '/api/certificates/private_keys',
  '/api/web_security/csp',
  '/api/user/secrets',
  '/api/user/scripts',
  '/api/user/tags',
];

test.describe('Paginated list search', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('every list endpoint accepts a search query without a 500', async ({ page }) => {
    for (const endpoint of LIST_ENDPOINTS) {
      // Mirror the exact request the UI issues when a user types into the search box on first
      // load: default sort by `updatedAt`, descending, with a free-text query.
      const url = `${endpoint}?page=0&pageSize=15&sort=updatedAt&order=desc&q=x`;
      const response = await page.request.get(url);
      expect(response.status(), `${endpoint} → ${await response.text()}`).toBe(200);

      const body = await response.json();
      expect(body, endpoint).toHaveProperty('items');
      expect(body, endpoint).toHaveProperty('total');
      expect(Array.isArray(body.items), endpoint).toBeTruthy();
    }
  });

  test('search matches a seeded item case-insensitively', async ({ page }) => {
    const secretRes = await page.request.post('/api/user/secrets', {
      data: { name: 'SEARCHABLE_KEY', value: 'value' },
    });
    expect(secretRes.ok()).toBeTruthy();

    // Lower-case query must match the upper-case name (ILIKE is case-insensitive).
    const response = await page.request.get('/api/user/secrets?page=0&pageSize=15&q=searchable');
    expect(response.status(), await response.text()).toBe(200);
    const body = await response.json();
    expect(body.total).toBe(1);
    expect(body.items.map((s: { name: string }) => s.name)).toContain('SEARCHABLE_KEY');
  });
});
