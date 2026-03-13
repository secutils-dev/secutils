import type { Locator, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

async function verifyContextMenuItemExists(editor: Locator, page: Page, label: string) {
  await editor.click();
  await editor.click({ button: 'right' });

  const menu = page.locator('.monaco-menu-container');
  await expect(menu).toBeVisible({ timeout: 5000 });
  await expect(menu.getByRole('menuitem', { name: label })).toBeVisible({ timeout: 5000 });

  await page.keyboard.press('Escape');
}

async function triggerSnippetAction(page: Page, actionId: string) {
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

function editorLines(editor: Locator): Locator {
  return editor.locator('.view-lines');
}

test.describe('Script editor context menu snippets', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('page tracker - insert content extractor snippet', async ({ page }) => {
    await page.goto('/ws/web_scraping__page');
    const createButton = page.getByRole('button', { name: 'Track page' });
    await expect(createButton).toBeVisible({ timeout: 15000 });

    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add tracker' }) });
    await expect(flyout).toBeVisible();

    const contentExtractorLabel = flyout.getByText('Content extractor', { exact: true });
    await contentExtractorLabel.scrollIntoViewIfNeeded();

    const editor = flyout.locator('.euiFormRow').filter({ hasText: 'Content extractor' }).locator('.monaco-editor');
    await expect(editor).toBeVisible({ timeout: 15000 });

    await verifyContextMenuItemExists(editor, page, 'Insert Example: Page Content Extractor');
    await triggerSnippetAction(page, 'insert-snippet-page-extractor-basic');

    const lines = editorLines(editor);
    await expect(lines).toContainText('export async function execute(page)', { timeout: 5000 });
    await expect(lines).toContainText('page.goto', { timeout: 5000 });
    await expect(lines).toContainText('return result;', { timeout: 5000 });
  });

  test('API tracker - insert data extractor snippet', async ({ page }) => {
    await page.goto('/ws/web_scraping__api');
    const createButton = page.getByRole('button', { name: 'Track API' });
    await expect(createButton).toBeVisible({ timeout: 15000 });

    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add API tracker' }) });
    await expect(flyout).toBeVisible();

    const dataExtractorLabel = flyout.getByText('Data extractor', { exact: true });
    await dataExtractorLabel.scrollIntoViewIfNeeded();

    const editor = flyout.locator('.euiFormRow').filter({ hasText: 'Data extractor' }).locator('.monaco-editor');
    await expect(editor).toBeVisible({ timeout: 15000 });

    await verifyContextMenuItemExists(editor, page, 'Insert Example: Data Extractor');
    await triggerSnippetAction(page, 'insert-snippet-api-extractor-basic');

    const lines = editorLines(editor);
    await expect(lines).toContainText('context.responses', { timeout: 5000 });
    await expect(lines).toContainText('Deno.core.decode', { timeout: 5000 });
  });

  test('API tracker - insert request configurator snippet', async ({ page }) => {
    await page.goto('/ws/web_scraping__api');
    const createButton = page.getByRole('button', { name: 'Track API' });
    await expect(createButton).toBeVisible({ timeout: 15000 });

    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add API tracker' }) });
    await expect(flyout).toBeVisible();

    const advancedToggle = flyout.getByText('Advanced mode', { exact: true });
    await advancedToggle.click();

    const configuratorLabel = flyout.getByText('Request configurator', { exact: true });
    await configuratorLabel.scrollIntoViewIfNeeded();

    const editor = flyout.locator('.euiFormRow').filter({ hasText: 'Request configurator' }).locator('.monaco-editor');
    await expect(editor).toBeVisible({ timeout: 15000 });

    await verifyContextMenuItemExists(editor, page, 'Insert Example: Request Configurator');
    await triggerSnippetAction(page, 'insert-snippet-api-configurator-basic');

    const lines = editorLines(editor);
    await expect(lines).toContainText('context.requests', { timeout: 5000 });
    await expect(lines).toContainText('Authorization', { timeout: 5000 });
  });

  test('responder - insert script snippet', async ({ page }) => {
    await page.goto('/ws/webhooks__responders');
    const createButton = page.getByRole('button', { name: 'Create responder' });
    await expect(createButton).toBeVisible({ timeout: 15000 });

    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add responder' }) });
    await expect(flyout).toBeVisible();

    const advancedToggle = flyout.getByText('Advanced mode', { exact: true });
    await advancedToggle.click();

    const scriptLabel = flyout.getByText('Script', { exact: true });
    await scriptLabel.scrollIntoViewIfNeeded();

    const editor = flyout
      .locator('.euiFormRow')
      .filter({ has: page.getByText('Script', { exact: true }) })
      .locator('.monaco-editor');
    await expect(editor).toBeVisible({ timeout: 15000 });

    await verifyContextMenuItemExists(editor, page, 'Insert Example: Responder Script');
    await triggerSnippetAction(page, 'insert-snippet-responder-script-basic');

    const lines = editorLines(editor);
    await expect(lines).toContainText('statusCode', { timeout: 5000 });
    await expect(lines).toContainText('context', { timeout: 5000 });
  });

  test('responder - insert request forwarder snippet', async ({ page }) => {
    await page.goto('/ws/webhooks__responders');
    const createButton = page.getByRole('button', { name: 'Create responder' });
    await expect(createButton).toBeVisible({ timeout: 15000 });

    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add responder' }) });
    await expect(flyout).toBeVisible();

    const advancedToggle = flyout.getByText('Advanced mode', { exact: true });
    await advancedToggle.click();

    const scriptLabel = flyout.getByText('Script', { exact: true });
    await scriptLabel.scrollIntoViewIfNeeded();

    const editor = flyout
      .locator('.euiFormRow')
      .filter({ has: page.getByText('Script', { exact: true }) })
      .locator('.monaco-editor');
    await expect(editor).toBeVisible({ timeout: 15000 });

    await verifyContextMenuItemExists(editor, page, 'Insert Example: Request Forwarder');
    await triggerSnippetAction(page, 'insert-snippet-responder-script-forwarder');

    const lines = editorLines(editor);
    await expect(lines).toContainText('op_proxy_request', { timeout: 5000 });
    await expect(lines).toContainText('context.path', { timeout: 5000 });
  });

  test('responder - insert advanced request forwarder snippet', async ({ page }) => {
    await page.goto('/ws/webhooks__responders');
    const createButton = page.getByRole('button', { name: 'Create responder' });
    await expect(createButton).toBeVisible({ timeout: 15000 });

    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add responder' }) });
    await expect(flyout).toBeVisible();

    const advancedToggle = flyout.getByText('Advanced mode', { exact: true });
    await advancedToggle.click();

    const scriptLabel = flyout.getByText('Script', { exact: true });
    await scriptLabel.scrollIntoViewIfNeeded();

    const editor = flyout
      .locator('.euiFormRow')
      .filter({ has: page.getByText('Script', { exact: true }) })
      .locator('.monaco-editor');
    await expect(editor).toBeVisible({ timeout: 15000 });

    await verifyContextMenuItemExists(editor, page, 'Insert Example: Advanced Request Forwarder');
    await triggerSnippetAction(page, 'insert-snippet-responder-script-forwarder-advanced');

    const lines = editorLines(editor);
    await expect(lines).toContainText('op_proxy_request', { timeout: 5000 });

    // The advanced snippet is taller than the editor viewport, so scroll to reveal the bottom.
    await page.evaluate(() => {
      const m = (
        window as Window & {
          __test_monaco?: {
            editor: {
              getEditors(): Array<{ getModel(): { getLineCount(): number } | null; revealLine(line: number): void }>;
            };
          };
        }
      ).__test_monaco;
      if (!m) return;
      for (const ed of m.editor.getEditors()) {
        const model = ed.getModel();
        if (model) ed.revealLine(model.getLineCount());
      }
    });

    await expect(lines).toContainText('trackResponse', { timeout: 5000 });
    await expect(lines).toContainText('skipRequest', { timeout: 5000 });
  });
});
