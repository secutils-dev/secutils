import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

const REVISION_OLD_CONTENT =
  '<html>\n<head><title>Old Title</title></head>\n<body>\n<p>Hello World</p>\n</body>\n</html>';
const REVISION_NEW_CONTENT =
  '<html>\n<head><title>New Title</title></head>\n<body>\n<p>Hello Changed World</p>\n</body>\n</html>';

function mockRevisions() {
  return [
    {
      id: '00000000-0000-7000-8000-000000000002',
      trackerId: '00000000-0000-7000-8000-000000000000',
      data: { original: REVISION_NEW_CONTENT },
      createdAt: 1740000100,
    },
    {
      id: '00000000-0000-7000-8000-000000000001',
      trackerId: '00000000-0000-7000-8000-000000000000',
      data: { original: REVISION_OLD_CONTENT },
      createdAt: 1740000000,
    },
  ];
}

test.describe.serial('Page Tracker Diff View', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('renders Monaco DiffEditor in diff mode with changed content', async ({ page }) => {
    const createResponse = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'Diff Test Tracker',
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
      },
    });
    expect(createResponse.ok()).toBeTruthy();

    await page.route('**/api/utils/web_scraping/page/*/history', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(mockRevisions()),
      });
    });

    await page.goto('/ws/web_scraping__page');
    const trackerRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Diff Test Tracker' }) });
    await expect(trackerRow).toBeVisible({ timeout: 15000 });

    await trackerRow.getByRole('button', { name: 'Show history' }).click();

    // Wait for the view mode toggle to appear, indicating revisions loaded.
    const defaultButton = page.getByRole('button', { name: 'Default', exact: true });
    await expect(defaultButton).toBeVisible({ timeout: 15000 });

    const diffButton = page.getByRole('button', { name: 'Diff', exact: true });
    await diffButton.click();

    const diffEditorContainer = page.locator('.monaco-diff-editor');
    await expect(diffEditorContainer).toBeVisible({ timeout: 15000 });

    const layoutToggle = page.getByRole('button', { name: 'Side by side' });
    await expect(layoutToggle).toBeVisible();

    const inlineButton = page.getByRole('button', { name: 'Inline' });
    await expect(inlineButton).toBeVisible();
  });

  test('shows "No changes" prompt when revisions are identical', async ({ page }) => {
    const identicalContent = '<html><body>Same</body></html>';
    const identicalRevisions = [
      {
        id: '00000000-0000-7000-8000-000000000002',
        trackerId: '00000000-0000-7000-8000-000000000000',
        data: { original: identicalContent },
        createdAt: 1740000100,
      },
      {
        id: '00000000-0000-7000-8000-000000000001',
        trackerId: '00000000-0000-7000-8000-000000000000',
        data: { original: identicalContent },
        createdAt: 1740000000,
      },
    ];

    const createResponse = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'No Changes Tracker',
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
      },
    });
    expect(createResponse.ok()).toBeTruthy();

    await page.route('**/api/utils/web_scraping/page/*/history', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(identicalRevisions),
      });
    });

    await page.goto('/ws/web_scraping__page');
    const trackerRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'No Changes Tracker' }) });
    await expect(trackerRow).toBeVisible({ timeout: 15000 });

    await trackerRow.getByRole('button', { name: 'Show history' }).click();

    // Wait for the view mode toggle to appear, indicating revisions loaded.
    const defaultButton = page.getByRole('button', { name: 'Default', exact: true });
    await expect(defaultButton).toBeVisible({ timeout: 15000 });

    const diffButton = page.getByRole('button', { name: 'Diff', exact: true });
    await diffButton.click();

    await expect(page.getByRole('heading', { name: 'No changes' })).toBeVisible({ timeout: 15000 });
    await expect(page.getByText('The content is identical between these two revisions.')).toBeVisible();
  });

  test('switches between side-by-side and inline layout', async ({ page }) => {
    const createResponse = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'Layout Toggle Tracker',
        config: { revisions: 3 },
        target: { extractor: 'export async function execute() { return "<p>test</p>"; }' },
      },
    });
    expect(createResponse.ok()).toBeTruthy();

    await page.route('**/api/utils/web_scraping/page/*/history', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(mockRevisions()),
      });
    });

    await page.goto('/ws/web_scraping__page');
    const trackerRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Layout Toggle Tracker' }) });
    await expect(trackerRow).toBeVisible({ timeout: 15000 });

    await trackerRow.getByRole('button', { name: 'Show history' }).click();

    // Wait for the view mode toggle to appear, indicating revisions loaded.
    const defaultButton = page.getByRole('button', { name: 'Default', exact: true });
    await expect(defaultButton).toBeVisible({ timeout: 15000 });

    const diffButton = page.getByRole('button', { name: 'Diff', exact: true });
    await diffButton.click();

    const diffEditorContainer = page.locator('.monaco-diff-editor');
    await expect(diffEditorContainer).toBeVisible({ timeout: 15000 });

    // The default is side-by-side: two editors should be visible.
    await expect(page.locator('.monaco-diff-editor .editor.original')).toBeVisible();
    await expect(page.locator('.monaco-diff-editor .editor.modified')).toBeVisible();

    // Switch to inline mode.
    const inlineButton = page.getByRole('button', { name: 'Inline' });
    await inlineButton.click();

    // In inline mode, Monaco uses a single editor pane.
    await expect(diffEditorContainer).toBeVisible();
  });
});
