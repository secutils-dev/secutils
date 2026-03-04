import type { Locator, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

import type { PageDebugResult } from './page_tracker_debug_fixtures';
import { ensureUserAndLogin } from '../helpers';

// ---------------------------------------------------------------------------
// Fixture factories
// ---------------------------------------------------------------------------

function debugResultSimple(): PageDebugResult {
  return {
    durationMs: 2800,
    result: '## Secutils.dev',
    target: {
      type: 'page',
      engine: { type: 'chromium' },
      extractorSource:
        "export async function execute(page) {\n  await page.goto('https://secutils.dev');\n  return `## ${await page.title()}`;\n}",
      logs: [
        { level: 'info', message: 'Navigating to https://secutils.dev...' },
        { level: 'info', message: 'Page loaded successfully' },
      ],
      durationMs: 2700,
    },
  };
}

function debugResultWithParams(): PageDebugResult {
  return {
    durationMs: 3200,
    result: '## Dashboard',
    target: {
      type: 'page',
      params: { secrets: { apiKey: 'sk-test-123' } },
      engine: { type: 'chromium' },
      extractorSource: "export async function execute(page) { return 'ok'; }",
      logs: [{ level: 'info', message: 'Connecting to browser...' }],
      durationMs: 3100,
    },
  };
}

function debugResultWithError(): PageDebugResult {
  return {
    durationMs: 1500,
    error: 'Extractor script failed: TimeoutError: page.goto: Timeout 30000ms exceeded.',
    target: {
      type: 'page',
      engine: { type: 'chromium' },
      extractorSource: "export async function execute(page) { await page.goto('https://down.example.com'); }",
      logs: [
        { level: 'info', message: 'Navigating to https://down.example.com...' },
        { level: 'error', message: 'Navigation failed: net::ERR_NAME_NOT_RESOLVED' },
      ],
      durationMs: 1400,
      error: 'TimeoutError: page.goto: Timeout 30000ms exceeded.',
    },
  };
}

function debugResultNoLogs(): PageDebugResult {
  return {
    durationMs: 800,
    result: 'simple text',
    target: {
      type: 'page',
      extractorSource: "export async function execute(page) { return 'simple text'; }",
      logs: [],
      durationMs: 750,
    },
  };
}

// Tiny 1x1 red PNG as base64 for testing screenshot display.
const TINY_PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwADhQGAWjR9awAAAABJRU5ErkJggg==';

function debugResultWithScreenshots(): PageDebugResult {
  return {
    durationMs: 3500,
    result: 'ok',
    target: {
      type: 'page',
      engine: { type: 'chromium' },
      extractorSource: "export async function execute(page) { await page.goto('https://example.com'); return 'ok'; }",
      logs: [{ level: 'info', message: 'Connected.' }],
      screenshots: [
        { label: 'after goto: https://example.com', data: TINY_PNG, mimeType: 'image/png' },
        { label: 'page.screenshot()', data: TINY_PNG, mimeType: 'image/png' },
      ],
      durationMs: 3400,
    },
  };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function openTrackerFlyout(page: Page) {
  await page.goto('/ws/web_scraping__page');
  const createButton = page.getByRole('button', { name: 'Track page' });
  await expect(createButton).toBeVisible({ timeout: 15000 });
  await createButton.click();

  const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add tracker' }) });
  await expect(flyout).toBeVisible();
  return flyout;
}

async function clickDebug(flyout: Locator) {
  const debugButton = flyout.getByRole('button', { name: 'Debug' });
  await expect(debugButton).toBeEnabled();
  await debugButton.click();
}

function getDebugModal(page: Page) {
  return page.locator('[data-test-subj="debug-modal"]');
}

function mockDebugEndpoint(page: Page, result: PageDebugResult) {
  return page.route('**/api/utils/web_scraping/page/debug', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(result),
    });
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

test.describe('Page Tracker Debug Panel', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test.describe('basic controls', () => {
    test('shows debug button enabled with default extractor script', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);
      const debugButton = flyout.getByRole('button', { name: 'Debug' });
      await expect(debugButton).toBeVisible();
      await expect(debugButton).toBeEnabled();
    });

    test('shows error on network failure', async ({ page }) => {
      await page.route('**/api/utils/web_scraping/page/debug', async (route) => {
        await route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ message: 'Internal server error' }),
        });
      });

      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      await expect(modal).toBeVisible();
      await expect(modal.getByText('Internal server error', { exact: false })).toBeVisible({ timeout: 15000 });
    });
  });

  test.describe('simple extraction (mocked)', () => {
    test('shows Extractor and Result steps', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultSimple());
      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      await expect(modal).toBeVisible();
      await expect(modal.getByRole('button', { name: 'Extractor' })).toBeVisible({ timeout: 15000 });
      await expect(modal.getByRole('button', { name: 'Result' })).toBeVisible();

      await expect(modal.getByRole('button', { name: /Configurator/ })).not.toBeVisible();
      await expect(modal.getByRole('button', { name: /Request/ })).not.toBeVisible();
    });

    test('shows result detail by default with total duration', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultSimple());
      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      await expect(modal.getByRole('button', { name: 'Result' })).toBeVisible({ timeout: 15000 });
      await expect(modal.getByText('2800ms total')).toBeVisible();
    });

    test('shows extractor detail with duration and engine badge', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultSimple());
      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      const extractorStep = modal.getByRole('button', { name: 'Extractor' });
      await expect(extractorStep).toBeVisible({ timeout: 15000 });
      await extractorStep.click();

      await expect(modal.getByText('2700ms', { exact: true })).toBeVisible();
      await expect(modal.getByText('chromium', { exact: true })).toBeVisible();
    });
  });

  test.describe('with logs (mocked)', () => {
    test('shows Logs tab with log entries', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultSimple());
      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      const extractorStep = modal.getByRole('button', { name: 'Extractor' });
      await expect(extractorStep).toBeVisible({ timeout: 15000 });
      await extractorStep.click();

      const logsTab = modal.getByRole('tab', { name: 'Logs' });
      await expect(logsTab).toBeVisible();
      await logsTab.click();

      await expect(modal.getByText('Navigating to https://secutils.dev...')).toBeVisible();
      await expect(modal.getByText('Page loaded successfully')).toBeVisible();
    });

    test('hides Logs tab when no logs are present', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultNoLogs());
      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      const extractorStep = modal.getByRole('button', { name: 'Extractor' });
      await expect(extractorStep).toBeVisible({ timeout: 15000 });
      await extractorStep.click();

      await expect(modal.getByRole('tab', { name: 'Logs' })).not.toBeVisible();
    });
  });

  test.describe('with params (mocked)', () => {
    test('shows Params tab with secrets', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithParams());
      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      const extractorStep = modal.getByRole('button', { name: 'Extractor' });
      await expect(extractorStep).toBeVisible({ timeout: 15000 });
      await extractorStep.click();

      const paramsTab = modal.getByRole('tab', { name: 'Params' });
      await expect(paramsTab).toBeVisible();
      await paramsTab.click();

      await expect(modal.getByText('sk-test-123')).toBeVisible();
    });

    test('hides Params tab when no params are present', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultSimple());
      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      const extractorStep = modal.getByRole('button', { name: 'Extractor' });
      await expect(extractorStep).toBeVisible({ timeout: 15000 });
      await extractorStep.click();

      await expect(modal.getByRole('tab', { name: 'Params' })).not.toBeVisible();
    });
  });

  test.describe('error states (mocked)', () => {
    test('shows error logs in Extractor step when script fails', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithError());
      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      const extractorStep = modal.getByRole('button', { name: 'Extractor' });
      await expect(extractorStep).toBeVisible({ timeout: 15000 });
      await extractorStep.click();

      const logsTab = modal.getByRole('tab', { name: 'Logs' });
      await expect(logsTab).toBeVisible();
      await logsTab.click();

      await expect(modal.getByText('Navigation failed', { exact: false })).toBeVisible();
    });

    test('shows error callout in Result when pipeline fails', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithError());
      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      const resultStep = modal.getByRole('button', { name: 'Result' });
      await expect(resultStep).toBeVisible({ timeout: 15000 });

      await expect(modal.getByText('Pipeline failed')).toBeVisible();
      await expect(modal.getByText('TimeoutError', { exact: false })).toBeVisible();
    });
  });

  test.describe('secrets in request (mocked)', () => {
    test('sends secrets in the debug request body', async ({ page }) => {
      let capturedSecrets: unknown = null;
      await page.route('**/api/utils/web_scraping/page/debug', async (route) => {
        const postData = route.request().postDataJSON();
        capturedSecrets = postData?.secrets;
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(debugResultWithParams()),
        });
      });

      const flyout = await openTrackerFlyout(page);

      await flyout.getByLabel('Advanced mode').check();
      const secretsSelect = flyout.getByRole('combobox', { name: 'Access mode' });
      await secretsSelect.selectOption('all');

      await clickDebug(flyout);

      const modal = getDebugModal(page);
      await expect(modal.getByRole('button', { name: 'Result' })).toBeVisible({ timeout: 15000 });

      expect(capturedSecrets).toEqual({ type: 'all' });
    });

    test('sends secrets none when no secrets configured', async ({ page }) => {
      let capturedSecrets: unknown = null;
      await page.route('**/api/utils/web_scraping/page/debug', async (route) => {
        const postData = route.request().postDataJSON();
        capturedSecrets = postData?.secrets;
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(debugResultSimple()),
        });
      });

      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      await expect(modal.getByRole('button', { name: 'Result' })).toBeVisible({ timeout: 15000 });

      expect(capturedSecrets).toEqual({ type: 'none' });
    });
  });

  test.describe('with screenshots (mocked)', () => {
    test('shows Screenshots tab with images', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithScreenshots());
      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      const extractorStep = modal.getByRole('button', { name: 'Extractor' });
      await expect(extractorStep).toBeVisible({ timeout: 15000 });
      await extractorStep.click();

      const screenshotsTab = modal.getByRole('tab', { name: 'Screenshots' });
      await expect(screenshotsTab).toBeVisible();
      await screenshotsTab.click();

      await expect(modal.getByText('after goto: https://example.com')).toBeVisible();
      await expect(modal.getByText('page.screenshot()')).toBeVisible();
    });

    test('hides Screenshots tab when no screenshots are present', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultSimple());
      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      const extractorStep = modal.getByRole('button', { name: 'Extractor' });
      await expect(extractorStep).toBeVisible({ timeout: 15000 });
      await extractorStep.click();

      await expect(modal.getByRole('tab', { name: 'Screenshots' })).not.toBeVisible();
    });
  });

  test.describe('engine selection', () => {
    test('shows camoufox engine badge in debug results', async ({ page }) => {
      const camoufoxResult: PageDebugResult = {
        ...debugResultSimple(),
        target: { ...debugResultSimple().target, engine: { type: 'camoufox' } },
      };
      await mockDebugEndpoint(page, camoufoxResult);
      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      const extractorStep = modal.getByRole('button', { name: 'Extractor' });
      await expect(extractorStep).toBeVisible({ timeout: 15000 });
      await extractorStep.click();

      await expect(modal.getByText('camoufox', { exact: true })).toBeVisible();
    });

    test('engine selector is visible only in advanced mode', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);

      await expect(flyout.getByRole('combobox', { name: 'Browser engine' })).not.toBeVisible();

      await flyout.getByLabel('Advanced mode').check();
      await expect(flyout.getByRole('combobox', { name: 'Browser engine' })).toBeVisible();
    });

    test('sends engine in debug request when camoufox selected', async ({ page }) => {
      let capturedTarget: unknown = null;
      await page.route('**/api/utils/web_scraping/page/debug', async (route) => {
        const postData = route.request().postDataJSON();
        capturedTarget = postData?.target;
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            ...debugResultSimple(),
            target: { ...debugResultSimple().target, engine: { type: 'camoufox' } },
          }),
        });
      });

      const flyout = await openTrackerFlyout(page);
      await flyout.getByLabel('Advanced mode').check();
      await flyout.getByRole('combobox', { name: 'Browser engine' }).selectOption('camoufox');
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      await expect(modal.getByRole('button', { name: 'Result' })).toBeVisible({ timeout: 15000 });

      expect(capturedTarget).toHaveProperty('engine', { type: 'camoufox' });
    });

    test('does not send engine in debug request when chromium selected', async ({ page }) => {
      let capturedTarget: unknown = null;
      await page.route('**/api/utils/web_scraping/page/debug', async (route) => {
        const postData = route.request().postDataJSON();
        capturedTarget = postData?.target;
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(debugResultSimple()),
        });
      });

      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      await expect(modal.getByRole('button', { name: 'Result' })).toBeVisible({ timeout: 15000 });

      expect(capturedTarget).not.toHaveProperty('engine');
    });
  });

  test.describe('modal behavior', () => {
    test('closes modal when pressing Escape', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultSimple());
      const flyout = await openTrackerFlyout(page);
      await clickDebug(flyout);

      const modal = getDebugModal(page);
      await expect(modal).toBeVisible();
      await expect(modal.getByRole('button', { name: 'Result' })).toBeVisible({ timeout: 15000 });

      await page.keyboard.press('Escape');
      await expect(modal).not.toBeVisible();

      await expect(flyout).toBeVisible();
    });
  });
});
