import type { Locator, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

// ---------------------------------------------------------------------------
// Fixture data
// ---------------------------------------------------------------------------

const SAMPLE_PLAYWRIGHT_SCRIPT = `const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({
    headless: false
  });
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.goto('https://example.com');
  await page.getByRole('button', { name: 'Sign up' }).click();
  await page.getByPlaceholder('Email').fill('test@example.com');

  // ---------------------
  await context.close();
  await browser.close();
})();`;

const SAMPLE_DEVTOOLS_RECORDING = JSON.stringify({
  title: 'Test Recording',
  steps: [
    {
      type: 'navigate',
      url: 'https://example.com',
    },
    {
      type: 'click',
      selectors: [['aria/Get started']],
    },
    {
      type: 'change',
      selectors: [['#email']],
      value: 'test@example.com',
    },
  ],
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function triggerImportAction(page: Page, actionId: string) {
  await page.evaluate((id) => {
    const m = (
      window as Window & {
        __test_monaco?: { editor: { getEditors(): Array<{ getAction(id: string): { run(): void } | null }> } };
      }
    ).__test_monaco;
    if (!m) throw new Error('Monaco not available on window');
    for (const ed of m.editor.getEditors()) {
      const action = ed.getAction(id);
      if (action) {
        action.run();
        return;
      }
    }
    throw new Error(`Action ${id} not found on any editor`);
  }, actionId);
}

async function verifyImportMenuItemExists(editor: Locator, page: Page, label: string) {
  await editor.click();
  await editor.click({ button: 'right' });

  const menu = page.locator('.monaco-menu-container');
  await expect(menu).toBeVisible({ timeout: 5000 });
  await expect(menu.getByRole('menuitem', { name: label })).toBeVisible({ timeout: 5000 });

  await page.keyboard.press('Escape');
}

function editorLines(editor: Locator): Locator {
  return editor.locator('.view-lines');
}

async function openTrackerFlyout(page: Page) {
  await page.goto('/ws/web_scraping__page');
  const createButton = page.getByRole('button', { name: 'Track page' });
  await expect(createButton).toBeVisible({ timeout: 15000 });

  await createButton.click();
  const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add tracker' }) });
  await expect(flyout).toBeVisible();
  return flyout;
}

function getEditor(flyout: Locator) {
  return flyout.locator('.euiFormRow').filter({ hasText: 'Content extractor' }).locator('.monaco-editor');
}

async function openImportModal(page: Page, actionId: string, label: string) {
  // Trigger the import action via Monaco API
  await triggerImportAction(page, actionId);

  // The import modal should appear
  const modal = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: label }) });
  await expect(modal).toBeVisible({ timeout: 10000 });

  return modal;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

test.describe('Script Editor Import Functionality', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test.describe('Playwright script import', () => {
    test('opens import modal in full screen mode', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);

      const contentExtractorLabel = flyout.getByText('Content extractor', { exact: true });
      await contentExtractorLabel.scrollIntoViewIfNeeded();

      const editor = getEditor(flyout);
      await expect(editor).toBeVisible({ timeout: 15000 });

      const fullScreenButton = flyout.getByRole('button', { name: 'Enter full screen' });
      await fullScreenButton.click();

      const fullScreenOverlay = page.locator('[data-test-subj="scriptEditorFullScreen"]');
      await expect(fullScreenOverlay).toBeVisible({ timeout: 5000 });

      const modal = await openImportModal(page, 'import-playwright-recording', 'Import: Playwright recording');
      await expect(fullScreenOverlay).toBeVisible();

      const cancelButton = modal.getByRole('button', { name: 'Cancel' });
      await cancelButton.click();
      await expect(modal).not.toBeVisible();
    });

    test('imports Playwright recording via context menu', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);

      const contentExtractorLabel = flyout.getByText('Content extractor', { exact: true });
      await contentExtractorLabel.scrollIntoViewIfNeeded();

      const editor = getEditor(flyout);
      await expect(editor).toBeVisible({ timeout: 15000 });

      await verifyImportMenuItemExists(editor, page, 'Import: Playwright recording');

      const modal = await openImportModal(page, 'import-playwright-recording', 'Import: Playwright recording');

      // Paste the Playwright script
      const textarea = modal.getByPlaceholder('Paste recorded script or JSON here');
      await expect(textarea).toBeVisible();
      await textarea.fill(SAMPLE_PLAYWRIGHT_SCRIPT);

      // Wait for preview to appear
      const previewBlock = modal.getByText('export async function execute(page)');
      await expect(previewBlock).toBeVisible({ timeout: 5000 });

      // Verify the preview shows the transformed script
      const previewCode = modal.locator('.euiCodeBlock');
      await expect(previewCode).toContainText('export async function execute(page)');
      await expect(previewCode).toContainText("await page.goto('https://example.com')");
      await expect(previewCode).toContainText("await page.getByRole('button'");
      await expect(previewCode).toContainText('Sign up');

      // Click Import
      const importButton = modal.getByRole('button', { name: 'Import' });
      await expect(importButton).toBeEnabled();
      await importButton.click();

      // Modal should close
      await expect(modal).not.toBeVisible();

      // The editor should now contain the transformed script
      const lines = editorLines(editor);
      await expect(lines).toContainText('export async function execute(page)', { timeout: 5000 });
      // Use partial matches since Monaco's text rendering may not include all whitespace
      await expect(lines).toContainText('page.goto', { timeout: 5000 });
      await expect(lines).toContainText('https://example.com', { timeout: 5000 });
      await expect(lines).toContainText('getByRole', { timeout: 5000 });
    });

    test('handles unrecognizable Playwright script gracefully', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);

      const contentExtractorLabel = flyout.getByText('Content extractor', { exact: true });
      await contentExtractorLabel.scrollIntoViewIfNeeded();

      const editor = getEditor(flyout);
      await expect(editor).toBeVisible({ timeout: 15000 });

      const modal = await openImportModal(page, 'import-playwright-recording', 'Import: Playwright recording');

      // Paste content that doesn't contain any page.* calls
      const textarea = modal.getByPlaceholder('Paste recorded script or JSON here');
      await textarea.fill('console.log("hello world");');

      // Wait for preview to appear
      await expect(modal.locator('.euiCodeBlock')).toBeVisible({ timeout: 5000 });

      // The transformer is permissive - it will still generate output
      // The preview should show just the console.log wrapped in execute function
      const previewCode = modal.locator('.euiCodeBlock');
      await expect(previewCode).toContainText('export async function execute(page)');
      await expect(previewCode).toContainText('console.log');

      // Import button should be enabled since a preview was generated
      const importButton = modal.getByRole('button', { name: 'Import' });
      await expect(importButton).toBeEnabled();
    });

    test('cancels import via Cancel button', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);

      const contentExtractorLabel = flyout.getByText('Content extractor', { exact: true });
      await contentExtractorLabel.scrollIntoViewIfNeeded();

      const editor = getEditor(flyout);
      await expect(editor).toBeVisible({ timeout: 15000 });

      const modal = await openImportModal(page, 'import-playwright-recording', 'Import: Playwright recording');

      // Paste content
      const textarea = modal.getByPlaceholder('Paste recorded script or JSON here');
      await textarea.fill(SAMPLE_PLAYWRIGHT_SCRIPT);

      // Wait for preview to confirm valid content
      await expect(modal.locator('.euiCodeBlock')).toBeVisible({ timeout: 5000 });

      // Click Cancel
      const cancelButton = modal.getByRole('button', { name: 'Cancel' });
      await cancelButton.click();

      // Modal should close
      await expect(modal).not.toBeVisible();
    });

    test('cancels import via Escape key', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);

      const contentExtractorLabel = flyout.getByText('Content extractor', { exact: true });
      await contentExtractorLabel.scrollIntoViewIfNeeded();

      const editor = getEditor(flyout);
      await expect(editor).toBeVisible({ timeout: 15000 });

      const modal = await openImportModal(page, 'import-playwright-recording', 'Import: Playwright recording');

      // Paste content
      const textarea = modal.getByPlaceholder('Paste recorded script or JSON here');
      await textarea.fill(SAMPLE_PLAYWRIGHT_SCRIPT);

      // Wait for preview to confirm valid content
      await expect(modal.locator('.euiCodeBlock')).toBeVisible({ timeout: 5000 });

      // Press Escape
      await page.keyboard.press('Escape');

      // Modal should close
      await expect(modal).not.toBeVisible();
    });
  });

  test.describe('DevTools recording import', () => {
    test('imports Chrome DevTools recording via context menu', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);

      const contentExtractorLabel = flyout.getByText('Content extractor', { exact: true });
      await contentExtractorLabel.scrollIntoViewIfNeeded();

      const editor = getEditor(flyout);
      await expect(editor).toBeVisible({ timeout: 15000 });

      await verifyImportMenuItemExists(editor, page, 'Import: Chrome DevTools recording');

      const modal = await openImportModal(page, 'import-devtools-recording', 'Import: Chrome DevTools recording');

      // Paste the DevTools recording JSON
      const textarea = modal.getByPlaceholder('Paste recorded script or JSON here');
      await expect(textarea).toBeVisible();
      await textarea.fill(SAMPLE_DEVTOOLS_RECORDING);

      // Wait for preview to appear
      const previewBlock = modal.getByText('export async function execute(page)');
      await expect(previewBlock).toBeVisible({ timeout: 5000 });

      // Verify the preview shows the transformed script
      const previewCode = modal.locator('.euiCodeBlock');
      await expect(previewCode).toContainText('export async function execute(page)');
      await expect(previewCode).toContainText("await page.goto('https://example.com')");

      // Click Import
      const importButton = modal.getByRole('button', { name: 'Import' });
      await expect(importButton).toBeEnabled();
      await importButton.click();

      // Modal should close
      await expect(modal).not.toBeVisible();

      // The editor should now contain the transformed script
      const lines = editorLines(editor);
      await expect(lines).toContainText('export async function execute(page)', { timeout: 5000 });
      await expect(lines).toContainText('page.goto', { timeout: 5000 });
    });

    test('disables import button for invalid DevTools JSON', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);

      const contentExtractorLabel = flyout.getByText('Content extractor', { exact: true });
      await contentExtractorLabel.scrollIntoViewIfNeeded();

      const editor = getEditor(flyout);
      await expect(editor).toBeVisible({ timeout: 15000 });

      const modal = await openImportModal(page, 'import-devtools-recording', 'Import: Chrome DevTools recording');

      // Paste invalid JSON
      const textarea = modal.getByPlaceholder('Paste recorded script or JSON here');
      await textarea.fill('not valid json {{{');

      // Wait a moment for transformation to attempt
      await page.waitForTimeout(500);

      // The import button should be disabled (no valid preview generated)
      const importButton = modal.getByRole('button', { name: 'Import' });
      await expect(importButton).toBeDisabled();
    });

    test('disables import button for DevTools JSON without steps', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);

      const contentExtractorLabel = flyout.getByText('Content extractor', { exact: true });
      await contentExtractorLabel.scrollIntoViewIfNeeded();

      const editor = getEditor(flyout);
      await expect(editor).toBeVisible({ timeout: 15000 });

      const modal = await openImportModal(page, 'import-devtools-recording', 'Import: Chrome DevTools recording');

      // Paste JSON without steps array
      const textarea = modal.getByPlaceholder('Paste recorded script or JSON here');
      await textarea.fill(JSON.stringify({ title: 'Empty recording' }));

      // Wait a moment for transformation to attempt
      await page.waitForTimeout(500);

      // The import button should be disabled (no valid preview generated without steps)
      const importButton = modal.getByRole('button', { name: 'Import' });
      await expect(importButton).toBeDisabled();
    });
  });

  test.describe('import preview behavior', () => {
    test('shows live preview as user types', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);

      const contentExtractorLabel = flyout.getByText('Content extractor', { exact: true });
      await contentExtractorLabel.scrollIntoViewIfNeeded();

      const editor = getEditor(flyout);
      await expect(editor).toBeVisible({ timeout: 15000 });

      const modal = await openImportModal(page, 'import-playwright-recording', 'Import: Playwright recording');

      const textarea = modal.getByPlaceholder('Paste recorded script or JSON here');

      // Initially no preview (empty input)
      await expect(modal.locator('.euiCodeBlock')).not.toBeVisible();

      // Type a script with page interaction - preview should appear
      await textarea.fill(`const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.goto('https://example.com');
})();`);

      await expect(modal.locator('.euiCodeBlock')).toBeVisible({ timeout: 5000 });

      // Verify the preview contains the page.goto call
      const previewCode = modal.locator('.euiCodeBlock');
      await expect(previewCode).toContainText('export async function execute(page)');
      await expect(previewCode).toContainText("await page.goto('https://example.com')");
    });

    test('clears error when input becomes valid', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);

      const contentExtractorLabel = flyout.getByText('Content extractor', { exact: true });
      await contentExtractorLabel.scrollIntoViewIfNeeded();

      const editor = getEditor(flyout);
      await expect(editor).toBeVisible({ timeout: 15000 });

      const modal = await openImportModal(page, 'import-devtools-recording', 'Import: Chrome DevTools recording');

      const textarea = modal.getByPlaceholder('Paste recorded script or JSON here');

      // Start with invalid JSON
      await textarea.fill('invalid');
      // Import should be disabled for invalid input
      await expect(modal.getByRole('button', { name: 'Import' })).toBeDisabled();

      // Replace content with valid JSON
      await textarea.fill(SAMPLE_DEVTOOLS_RECORDING);

      // Preview should appear and import should be enabled
      await expect(modal.locator('.euiCodeBlock')).toBeVisible({ timeout: 5000 });
      await expect(modal.getByRole('button', { name: 'Import' })).toBeEnabled();
    });
  });
});
