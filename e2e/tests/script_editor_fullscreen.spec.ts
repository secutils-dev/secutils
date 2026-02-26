import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

const EXTRACTOR_SCRIPT = 'export async function execute(page) { return await page.title(); }';

test.describe('Script editor full-screen mode', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('full-screen toggle button is visible in the tracker editor', async ({ page }) => {
    await page.goto('/ws/web_scraping__page');
    const createButton = page.getByRole('button', { name: 'Track page' });
    await expect(createButton).toBeVisible({ timeout: 15000 });

    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add tracker' }) });
    await expect(flyout).toBeVisible();

    // The Scripts section is below the fold; scroll it into view.
    const contentExtractorLabel = flyout.getByText('Content extractor');
    await contentExtractorLabel.scrollIntoViewIfNeeded();

    const fullScreenButton = flyout.getByRole('button', { name: 'Enter full screen' });
    await expect(fullScreenButton).toBeVisible({ timeout: 15000 });
  });

  test('enter and exit full-screen via button', async ({ page }) => {
    await page.goto('/ws/web_scraping__page');
    const createButton = page.getByRole('button', { name: 'Track page' });
    await expect(createButton).toBeVisible({ timeout: 15000 });

    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add tracker' }) });
    await expect(flyout).toBeVisible();

    const contentExtractorLabel = flyout.getByText('Content extractor');
    await contentExtractorLabel.scrollIntoViewIfNeeded();

    const fullScreenButton = flyout.getByRole('button', { name: 'Enter full screen' });
    await expect(fullScreenButton).toBeVisible({ timeout: 15000 });
    await fullScreenButton.click();

    const fullScreenOverlay = page.locator('[data-test-subj="scriptEditorFullScreen"]');
    await expect(fullScreenOverlay).toBeVisible({ timeout: 15000 });
    await expect(fullScreenOverlay.locator('.monaco-editor')).toBeVisible({ timeout: 15000 });

    const exitButton = fullScreenOverlay.getByRole('button', { name: 'Exit full screen' });
    await expect(exitButton).toBeVisible();
    await exitButton.click();

    await expect(fullScreenOverlay).not.toBeVisible();
    await expect(flyout).toBeVisible();
  });

  test('exit full-screen via Escape key', async ({ page }) => {
    await page.goto('/ws/web_scraping__page');
    const createButton = page.getByRole('button', { name: 'Track page' });
    await expect(createButton).toBeVisible({ timeout: 15000 });

    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add tracker' }) });
    await expect(flyout).toBeVisible();

    const contentExtractorLabel = flyout.getByText('Content extractor');
    await contentExtractorLabel.scrollIntoViewIfNeeded();

    const fullScreenButton = flyout.getByRole('button', { name: 'Enter full screen' });
    await expect(fullScreenButton).toBeVisible({ timeout: 15000 });
    await fullScreenButton.click();

    const fullScreenOverlay = page.locator('[data-test-subj="scriptEditorFullScreen"]');
    await expect(fullScreenOverlay).toBeVisible({ timeout: 15000 });

    await page.keyboard.press('Escape');

    await expect(fullScreenOverlay).not.toBeVisible();
    await expect(flyout).toBeVisible();
  });

  test('content persists across full-screen round-trip', async ({ page }) => {
    const createResponse = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'FullScreen Test Tracker',
        config: { revisions: 3 },
        target: { extractor: EXTRACTOR_SCRIPT },
      },
    });
    expect(createResponse.ok()).toBeTruthy();

    await page.goto('/ws/web_scraping__page');
    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'FullScreen Test Tracker' }) });
    await expect(row).toBeVisible({ timeout: 15000 });

    await row.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout).toBeVisible();

    // Scroll to the Content extractor section which is below the fold.
    const contentExtractorLabel = flyout.getByText('Content extractor');
    await contentExtractorLabel.scrollIntoViewIfNeeded();

    const inlineEditor = flyout.locator('.monaco-editor');
    await expect(inlineEditor).toBeVisible({ timeout: 15000 });
    await expect(inlineEditor).toContainText('page.title', { timeout: 15000 });

    const fullScreenButton = flyout.getByRole('button', { name: 'Enter full screen' });
    await expect(fullScreenButton).toBeVisible({ timeout: 15000 });
    await fullScreenButton.click();

    const fullScreenOverlay = page.locator('[data-test-subj="scriptEditorFullScreen"]');
    await expect(fullScreenOverlay).toBeVisible({ timeout: 15000 });

    const fullScreenEditor = fullScreenOverlay.locator('.monaco-editor');
    await expect(fullScreenEditor).toContainText('page.title', { timeout: 15000 });

    await fullScreenOverlay.getByRole('button', { name: 'Exit full screen' }).click();
    await expect(fullScreenOverlay).not.toBeVisible();
    await expect(flyout).toBeVisible();
    await expect(inlineEditor).toContainText('page.title', { timeout: 15000 });
  });
});
