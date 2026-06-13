import type { APIResponse, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

import { ensureUserAndLogin, goto } from '../helpers';

// 21 items at a page size of 10 spans exactly three pages (10 + 10 + 1), which lets us
// exercise first/last/random page access in addition to Next/Previous navigation.
const ITEM_COUNT = 21;
// Only the first few seeded items carry the shared tag, so tag filtering is observable.
const TAGGED_COUNT = 2;

// Lowercase on purpose: the Tags grid normalizes names to lowercase on save, while every other
// grid preserves them verbatim, so a lowercase name renders identically across all grids.
const itemName = (index: number) => `item_${String(index).padStart(3, '0')}`;

interface GridConfig {
  /** Human-readable grid name used in the test title. */
  label: string;
  /** Workspace route that renders the grid. */
  route: string;
  /** Exact placeholder of the grid's search input. */
  searchPlaceholder: string;
  /** Whether the grid offers server-side tag filtering (every grid except the Tags grid). */
  supportsTags: boolean;
  /** Creates a single entity via the API. `tagIds` is empty for untagged items. */
  create(page: Page, name: string, index: number, tagIds: string[]): Promise<APIResponse>;
}

const grids: GridConfig[] = [
  {
    label: 'Tags',
    route: '/ws/workspace__tags',
    searchPlaceholder: 'Search tags…',
    supportsTags: false,
    create: (page, name) => page.request.post('/api/user/tags', { data: { name, color: 'primary' } }),
  },
  {
    label: 'Secrets',
    route: '/ws/workspace__secrets',
    searchPlaceholder: 'Search secrets…',
    supportsTags: true,
    create: (page, name, _index, tagIds) =>
      page.request.post('/api/user/secrets', { data: { name, value: 'value', tagIds } }),
  },
  {
    label: 'Scripts',
    route: '/ws/workspace__scripts',
    searchPlaceholder: 'Search scripts…',
    supportsTags: true,
    create: (page, name, _index, tagIds) =>
      page.request.post('/api/user/scripts', {
        data: { name, scriptType: 'responder', content: 'console.log("x")', tagIds },
      }),
  },
  {
    label: 'Responders',
    route: '/ws/webhooks__responders',
    searchPlaceholder: 'Search by name, path, or ID...',
    supportsTags: true,
    create: (page, name, index, tagIds) =>
      page.request.post('/api/webhooks/responders', {
        data: {
          name,
          location: { pathType: '=', path: `/grid-pagination-${index}` },
          method: 'ANY',
          enabled: true,
          settings: { requestsToTrack: 0, statusCode: 200 },
          tagIds,
        },
      }),
  },
  {
    label: 'Certificate templates',
    route: '/ws/certificates__certificate_templates',
    searchPlaceholder: 'Search by name or ID...',
    supportsTags: true,
    create: (page, name, _index, tagIds) =>
      page.request.post('/api/certificates/templates', {
        data: {
          templateName: name,
          attributes: {
            commonName: name,
            keyAlgorithm: { keyType: 'ed25519' },
            signatureAlgorithm: 'ed25519',
            notValidBefore: 946720800,
            notValidAfter: 1893456000,
            version: 3,
            isCa: false,
          },
          tagIds,
        },
      }),
  },
  {
    label: 'Private keys',
    route: '/ws/certificates__private_keys',
    searchPlaceholder: 'Search by name or ID...',
    supportsTags: true,
    create: (page, name, _index, tagIds) =>
      page.request.post('/api/certificates/private_keys', {
        data: { keyName: name, alg: { keyType: 'ed25519' }, tagIds },
      }),
  },
  {
    label: 'Content security policies',
    route: '/ws/web_security__csp',
    searchPlaceholder: 'Search by name, ID, or policy content...',
    supportsTags: true,
    create: (page, name, _index, tagIds) =>
      page.request.post('/api/web_security/csp', {
        data: {
          name,
          content: { type: 'directives', value: [{ name: 'default-src', value: ["'self'"] }] },
          tagIds,
        },
      }),
  },
  {
    label: 'Page trackers',
    route: '/ws/web_scraping__page',
    searchPlaceholder: 'Search by name or ID...',
    supportsTags: true,
    create: (page, name, _index, tagIds) =>
      page.request.post('/api/web_scraping/page_trackers', {
        data: {
          name,
          config: { revisions: 1 },
          target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
          tagIds,
        },
      }),
  },
  {
    label: 'API trackers',
    route: '/ws/web_scraping__api',
    searchPlaceholder: 'Search by name or ID...',
    supportsTags: true,
    create: (page, name, _index, tagIds) =>
      page.request.post('/api/web_scraping/api_trackers', {
        data: {
          name,
          config: { revisions: 1 },
          target: { url: 'https://secutils.dev/' },
          tagIds,
        },
      }),
  },
];

/** Seeds a shared tag (when supported) and {@link ITEM_COUNT} entities, tagging the first few. */
async function seedGrid(page: Page, grid: GridConfig): Promise<string | undefined> {
  let tagId: string | undefined;
  if (grid.supportsTags) {
    const tagRes = await page.request.post('/api/user/tags', {
      data: { name: 'grid-pagination-tag', color: 'primary' },
    });
    expect(tagRes.ok(), `create tag for ${grid.label}`).toBeTruthy();
    tagId = (await tagRes.json()).id as string;
  }

  for (let index = 0; index < ITEM_COUNT; index++) {
    const tagIds = tagId && index < TAGGED_COUNT ? [tagId] : [];
    const res = await grid.create(page, itemName(index), index, tagIds);
    expect(res.ok(), `create ${itemName(index)} for ${grid.label}`).toBeTruthy();
  }

  return tagId;
}

for (const grid of grids) {
  test.describe(`${grid.label} grid pagination, sorting, search, and tag filtering`, () => {
    test.beforeEach(async ({ request, page }) => {
      await ensureUserAndLogin(request, page);
    });

    test('server-side paging, random page access, sort, search, and tag filter', async ({ page }) => {
      // Seeding (especially trackers, which register with Retrack) plus the full UI walk-through
      // can take a while on the Dockerized stack.
      test.setTimeout(120_000);

      const tagId = await seedGrid(page, grid);

      await goto(page, grid.route);

      const search = page.getByPlaceholder(grid.searchPlaceholder);
      await expect(search).toBeVisible({ timeout: 15000 });

      // EUI tags its controls with `data-test-subj` (not Playwright's default `data-testid`),
      // and renders page/arrow controls as a mix of links and disabled buttons, so we target the
      // stable `data-test-subj` attributes directly.
      const bySubj = (subj: string) => page.locator(`[data-test-subj="${subj}"]`);

      // Standardize the page size to 10 so the seeded items span exactly three pages. This also
      // exercises the "rows per page" control itself.
      const rowsPerPage = bySubj('tablePaginationPopoverButton');
      await expect(rowsPerPage).toBeVisible({ timeout: 15000 });
      await rowsPerPage.click();
      await bySubj('tablePagination-10-rows').click();

      // Sort by Name ascending for deterministic ordering across pages. EUI inherits the previous
      // column's sort direction when switching columns (the default sort is "updated" descending),
      // so the first click can land on descending — force ascending via the header's `aria-sort`.
      const nameHeader = page.getByRole('button', { name: 'Name', exact: true });
      const nameColumn = page.getByRole('columnheader', { name: 'Name', exact: true });
      await nameHeader.click();
      await expect(nameColumn).toHaveAttribute('aria-sort', /ascending|descending/, { timeout: 15000 });
      if ((await nameColumn.getAttribute('aria-sort')) !== 'ascending') {
        await nameHeader.click();
      }
      await expect(nameColumn).toHaveAttribute('aria-sort', 'ascending', { timeout: 15000 });
      await expect(page.getByText(itemName(0), { exact: true })).toBeVisible({ timeout: 15000 });
      // The last item lives on the third page and must not be present on the first.
      await expect(page.getByText(itemName(ITEM_COUNT - 1), { exact: true })).toHaveCount(0);

      // Random page access: jump straight to the last page (0-based index 2 == page 3).
      await bySubj('pagination-button-2').click();
      await expect(page.getByText(itemName(ITEM_COUNT - 1), { exact: true })).toBeVisible({ timeout: 15000 });
      await expect(page.getByText(itemName(0), { exact: true })).toHaveCount(0);

      // Previous-page navigation lands on the middle page (items 10..19).
      await bySubj('pagination-button-previous').click();
      await expect(page.getByText(itemName(10), { exact: true })).toBeVisible({ timeout: 15000 });

      // Jump back to the first page via the page-number button.
      await bySubj('pagination-button-0').click();
      await expect(page.getByText(itemName(0), { exact: true })).toBeVisible({ timeout: 15000 });

      // Next-page navigation advances to the middle page again.
      await bySubj('pagination-button-next').click();
      await expect(page.getByText(itemName(10), { exact: true })).toBeVisible({ timeout: 15000 });

      // Toggling the sort to descending resets to the first page; the largest name now leads.
      await nameHeader.click();
      await expect(nameColumn).toHaveAttribute('aria-sort', 'descending', { timeout: 15000 });
      await expect(page.getByText(itemName(ITEM_COUNT - 1), { exact: true })).toBeVisible({ timeout: 15000 });
      await expect(page.getByText(itemName(0), { exact: true })).toHaveCount(0);

      // Server-side search spans the whole dataset regardless of the current page. "item_01"
      // matches exactly the ten items item_010..item_019.
      await search.fill('item_01');
      await expect(page.getByText(itemName(10), { exact: true })).toBeVisible({ timeout: 15000 });
      await expect(page.getByText(itemName(19), { exact: true })).toBeVisible();
      await expect(page.getByText(itemName(0), { exact: true })).toHaveCount(0);
      await search.fill('');
      // The sort is still descending here, so the largest name leads the (reset) first page.
      await expect(page.getByText(itemName(ITEM_COUNT - 1), { exact: true })).toBeVisible({ timeout: 15000 });

      // Server-side tag filtering (where supported): only the first TAGGED_COUNT items carry the tag.
      // The grid's tag filter button is labelled "Tags <n> available filters"; anchor to the
      // leading "Tags" so we don't match the global top-bar "Filter all lists by tags" button.
      if (tagId) {
        await page.getByRole('button', { name: /^Tags/ }).click();
        await page.getByRole('option', { name: 'grid-pagination-tag' }).click();
        await expect(page.getByText(itemName(0), { exact: true })).toBeVisible({ timeout: 15000 });
        await expect(page.getByText(itemName(1), { exact: true })).toBeVisible();
        await expect(page.getByText(itemName(5), { exact: true })).toHaveCount(0);
      }
    });
  });
}
