import { join } from 'path';

import { expect, test } from '@playwright/test';
import type { Locator } from '@playwright/test';

import { DOCS_IMG_DIR, EMAIL, ensureUserAndLogin, fixEntityTimestamps, goto, highlightOn, PASSWORD } from '../helpers';

const IMG_DIR = join(DOCS_IMG_DIR, 'user_scripts');

function getByRoleAndLabel(parent: Locator, role: 'combobox' | 'textbox', label: string) {
  return parent.locator(`:below(label:text("${label}"))`).getByRole(role).first();
}

test.describe('User Scripts guide screenshots', () => {
  test.beforeEach(async ({ page, request }) => {
    await ensureUserAndLogin(request, page, { email: EMAIL, password: PASSWORD });
    await fixEntityTimestamps(page, '**/api/user/scripts');
  });

  test('manage user scripts', async ({ page }) => {
    // Step 1: Navigate to the workspace Scripts page - empty state.
    await goto(page, '/ws/workspace__scripts');

    await expect(page.getByText('No scripts yet')).toBeVisible({ timeout: 15000 });
    const addButton = page.getByRole('button', { name: 'Add script' });
    await highlightOn(addButton);
    await page.screenshot({ path: join(IMG_DIR, 'scripts_step1_empty.png') });

    // Step 2: Open the Add Script modal and screenshot the form.
    await addButton.click();
    const modal = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add script' }) });
    await expect(modal).toBeVisible({ timeout: 15000 });
    await modal.getByPlaceholder('MY_SCRIPT').fill('EXTRACTOR_SCRIPT');
    await modal.getByLabel('Type').selectOption('api_extractor');
    // Wait for the Monaco editor to be ready
    await expect(modal.locator('.monaco-editor')).toBeVisible({ timeout: 15000 });
    await page.screenshot({ path: join(IMG_DIR, 'scripts_step2_form.png') });

    // Step 3: Create the first script via API (Monaco editor cannot be reliably filled via Playwright).
    // Close flyout first (has unsaved changes, so confirm discard).
    await modal.getByRole('button', { name: 'Close' }).click();
    await page.getByRole('button', { name: 'Discard' }).click();
    await expect(modal).not.toBeVisible();

    const apiResponse = await page.request.post('/api/user/scripts', {
      data: {
        name: 'EXTRACTOR_SCRIPT',
        scriptType: 'api_extractor',
        content: "(() => {\n  return { extracted: 'data' };\n})();",
      },
    });
    expect(apiResponse.ok()).toBeTruthy();

    // Refresh to see the created script.
    await goto(page, '/ws/workspace__scripts');
    await expect(page.getByText('EXTRACTOR_SCRIPT', { exact: true })).toBeVisible({ timeout: 15000 });
    await page.screenshot({ path: join(IMG_DIR, 'scripts_step3_created.png') });

    // Step 4: Create a second script via API.
    const apiResponse2 = await page.request.post('/api/user/scripts', {
      data: {
        name: 'RESPONDER_SCRIPT',
        scriptType: 'responder',
        content: "(() => {\n  return { statusCode: 200, body: 'Hello!' };\n})();",
      },
    });
    expect(apiResponse2.ok()).toBeTruthy();

    // Refresh to see both scripts.
    await goto(page, '/ws/workspace__scripts');
    await expect(page.getByText('RESPONDER_SCRIPT', { exact: true })).toBeVisible({ timeout: 15000 });
    await page.screenshot({ path: join(IMG_DIR, 'scripts_step4_list.png') });
  });

  test('import script from responder', async ({ page }) => {
    // First, create a script via API (using page.request to inherit auth cookies)
    const createResponse = await page.request.post('/api/user/scripts', {
      data: {
        name: 'TEST_RESPONDER',
        scriptType: 'responder',
        content: "(() => {\n  return { statusCode: 200, body: 'Hello!' };\n})();",
      },
    });
    expect(createResponse.ok()).toBeTruthy();

    // Navigate to responders
    await goto(page, '/ws/webhooks__responders');

    // Click create responder
    const createButton = page.getByRole('button', { name: 'Create responder' });
    await expect(createButton).toBeVisible({ timeout: 15000 });
    await createButton.click();

    // Wait for flyout to appear
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add responder' }) });
    await expect(flyout).toBeVisible({ timeout: 15000 });

    // Fill in basic details
    await flyout.getByLabel('Name').fill('Test Responder');
    await getByRoleAndLabel(flyout, 'textbox', 'Path').fill('/test-path');

    // Enable advanced mode to see the script field
    await flyout.getByLabel('Advanced mode').check();

    // Find the script editor
    const scriptEditor = flyout
      .locator('.euiFormRow')
      .filter({ has: page.getByText('Script', { exact: true }) })
      .locator('.monaco-editor');
    await expect(scriptEditor).toBeVisible({ timeout: 15000 });

    // Screenshot of the script editor before import
    await page.screenshot({ path: join(IMG_DIR, 'scripts_step5_before_import.png') });

    // Trigger the import action via Monaco API (action ID is prefixed with 'import-')
    await page.evaluate(() => {
      const m = (
        window as Window & {
          __test_monaco?: { editor: { getEditors(): Array<{ getAction(id: string): { run(): void } | null }> } };
        }
      ).__test_monaco;
      if (!m) throw new Error('Monaco not available on window');
      for (const ed of m.editor.getEditors()) {
        const action = ed.getAction('import-import-predefined-script');
        if (action) {
          action.run();
          return;
        }
      }
      throw new Error('Action import-import-predefined-script not found on any editor');
    });

    // Wait for import modal
    const importModal = page
      .getByRole('dialog')
      .filter({ has: page.getByRole('heading', { name: 'Import from predefined scripts' }) });
    await expect(importModal).toBeVisible({ timeout: 15000 });

    // Select the test script
    await importModal.getByText('TEST_RESPONDER (Responder)').click();
    await page.screenshot({ path: join(IMG_DIR, 'scripts_step6_import_modal.png') });

    // Import the script
    await importModal.getByRole('button', { name: 'Import' }).click();
    await importModal.waitFor({ state: 'hidden', timeout: 10000 });

    // Verify the script was imported into the editor (use partial match like other tests)
    const editorLines = scriptEditor.locator('.view-lines');
    await expect(editorLines).toContainText('statusCode', { timeout: 15000 });
    await expect(editorLines).toContainText('200', { timeout: 5000 });

    // Take the final screenshot showing the imported script
    await flyout.getByText('Script', { exact: true }).first().scrollIntoViewIfNeeded();
    await page.screenshot({ path: join(IMG_DIR, 'scripts_step7_imported.png') });

    // Test completed successfully - the flyout will be cleaned up by beforeEach
    // We skip closing the flyout as it's not essential for this screenshot test
  });

  test('import script from API tracker', async ({ page }) => {
    // Create API extractor and configurator scripts via API
    const extractorResponse = await page.request.post('/api/user/scripts', {
      data: {
        name: 'API_EXTRACTOR',
        scriptType: 'api_extractor',
        content: '(() => { return { body: Deno.core.encode(JSON.stringify({ data: "extracted" })) }; })();',
      },
    });
    expect(extractorResponse.ok()).toBeTruthy();

    const configuratorResponse = await page.request.post('/api/user/scripts', {
      data: {
        name: 'API_CONFIGURATOR',
        scriptType: 'api_configurator',
        content: '(() => { return { requests: context.requests }; })();',
      },
    });
    expect(configuratorResponse.ok()).toBeTruthy();

    // Navigate to API trackers
    await goto(page, '/ws/web_scraping__api');

    // Click create tracker
    const createButton = page.getByRole('button', { name: 'Track API' });
    await expect(createButton).toBeVisible({ timeout: 15000 });
    await createButton.click();

    // Wait for flyout to appear
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add API tracker' }) });
    await expect(flyout).toBeVisible({ timeout: 15000 });

    // Fill in basic details
    await flyout.getByLabel('Name').fill('Test API Tracker');
    await flyout.getByLabel('URL').fill('https://example.com/api');

    // Enable advanced mode to see script fields
    await flyout.getByLabel('Advanced mode').check();

    // Find the extractor script editor and trigger import
    const extractorEditor = flyout
      .locator('.euiFormRow')
      .filter({ has: page.getByText('Data extractor', { exact: false }) })
      .locator('.monaco-editor')
      .first();
    await expect(extractorEditor).toBeVisible({ timeout: 15000 });

    // Screenshot of the script editor before import
    await page.screenshot({ path: join(IMG_DIR, 'scripts_api_tracker_step1_before_import.png') });

    // Trigger import via Monaco API
    await page.evaluate(() => {
      const m = (
        window as Window & {
          __test_monaco?: { editor: { getEditors(): Array<{ getAction(id: string): { run(): void } | null }> } };
        }
      ).__test_monaco;
      if (!m) throw new Error('Monaco not available on window');
      for (const ed of m.editor.getEditors()) {
        const action = ed.getAction('import-import-predefined-script');
        if (action) {
          action.run();
          return;
        }
      }
      throw new Error('Action import-import-predefined-script not found on any editor');
    });

    // Wait for import modal and select API extractor script
    const importModal = page
      .getByRole('dialog')
      .filter({ has: page.getByRole('heading', { name: 'Import from predefined scripts' }) });
    await expect(importModal).toBeVisible({ timeout: 15000 });

    // Select the API extractor script
    await importModal.getByText('API_EXTRACTOR (API Extractor)').click();
    await page.screenshot({ path: join(IMG_DIR, 'scripts_api_tracker_step2_import_modal.png') });

    await importModal.getByRole('button', { name: 'Import' }).click();
    await importModal.waitFor({ state: 'hidden', timeout: 10000 });

    // Verify script was imported
    const editorLines = extractorEditor.locator('.view-lines');
    await expect(editorLines).toContainText('extracted', { timeout: 15000 });

    // Scroll the extractor editor into view and take the final screenshot
    await extractorEditor.scrollIntoViewIfNeeded();
    await page.screenshot({ path: join(IMG_DIR, 'scripts_api_tracker_step3_imported.png') });

    // Close flyout (confirm discard since the form has unsaved changes)
    await flyout.getByRole('button', { name: 'Close' }).click();
    await page.getByRole('button', { name: 'Discard' }).click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });
  });

  test('import script from page tracker', async ({ page }) => {
    // Create a page extractor script via API
    const extractorResponse = await page.request.post('/api/user/scripts', {
      data: {
        name: 'PAGE_EXTRACTOR',
        scriptType: 'page_extractor',
        content: 'export async function execute(page) { return await page.title(); }',
      },
    });
    expect(extractorResponse.ok()).toBeTruthy();

    // Navigate to page trackers
    await goto(page, '/ws/web_scraping__page');

    // Click create tracker
    const createButton = page.getByRole('button', { name: 'Track page' });
    await expect(createButton).toBeVisible({ timeout: 15000 });
    await createButton.click();

    // Wait for flyout to appear
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add tracker' }) });
    await expect(flyout).toBeVisible({ timeout: 15000 });

    // Fill in basic details
    await flyout.getByLabel('Name').fill('Test Page Tracker');

    // Find the extractor script editor and trigger import
    const extractorEditor = flyout
      .locator('.euiFormRow')
      .filter({ has: page.getByText('Content extractor', { exact: false }) })
      .locator('.monaco-editor')
      .first();
    await expect(extractorEditor).toBeVisible({ timeout: 15000 });

    // Screenshot of the script editor before import
    await page.screenshot({ path: join(IMG_DIR, 'scripts_page_tracker_step1_before_import.png') });

    // Trigger import via Monaco API
    await page.evaluate(() => {
      const m = (
        window as Window & {
          __test_monaco?: { editor: { getEditors(): Array<{ getAction(id: string): { run(): void } | null }> } };
        }
      ).__test_monaco;
      if (!m) throw new Error('Monaco not available on window');
      for (const ed of m.editor.getEditors()) {
        const action = ed.getAction('import-import-predefined-script');
        if (action) {
          action.run();
          return;
        }
      }
      throw new Error('Action import-import-predefined-script not found on any editor');
    });

    // Wait for import modal and select page extractor script
    const importModal = page
      .getByRole('dialog')
      .filter({ has: page.getByRole('heading', { name: 'Import from predefined scripts' }) });
    await expect(importModal).toBeVisible({ timeout: 15000 });

    // Select the page extractor script
    await importModal.getByText('PAGE_EXTRACTOR (Page Extractor)').click();
    await page.screenshot({ path: join(IMG_DIR, 'scripts_page_tracker_step2_import_modal.png') });

    await importModal.getByRole('button', { name: 'Import' }).click();
    await importModal.waitFor({ state: 'hidden', timeout: 10000 });

    // Verify script was imported
    const editorLines = extractorEditor.locator('.view-lines');
    await expect(editorLines).toContainText('page.title', { timeout: 15000 });

    // Scroll the extractor editor into view and take the final screenshot
    await extractorEditor.scrollIntoViewIfNeeded();
    await page.screenshot({ path: join(IMG_DIR, 'scripts_page_tracker_step3_imported.png') });

    // Close flyout (confirm discard since the form has unsaved changes)
    await flyout.getByRole('button', { name: 'Close' }).click();
    await page.getByRole('button', { name: 'Discard' }).click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });
  });
});
