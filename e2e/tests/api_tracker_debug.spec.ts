import type { Locator, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

import type { DebugResult } from './api_tracker_debug_fixtures';
import { ensureUserAndLogin } from '../helpers';

// Reachable from Retrack inside Docker via the host-gateway alias.
const REAL_TARGET_URL = 'http://host.docker.internal:7171/api/ui/state';

// ---------------------------------------------------------------------------
// Fixture factories (for tests that need precise debug-response shapes)
// ---------------------------------------------------------------------------

function debugResultWithExtractor(): DebugResult {
  return {
    durationMs: 210,
    result: { extracted: true, count: 5 },
    target: {
      type: 'api',
      params: { secrets: { apiKey: 'test-key-123' } },
      requests: [
        {
          index: 0,
          source: 'original',
          url: 'https://api.example.com/data',
          method: 'GET',
          statusCode: 200,
          responseHeaders: { 'content-type': 'application/json' },
          responseBodyRaw: '{"items":[1,2,3,4,5]}',
          responseBodyRawSize: 21,
          durationMs: 140,
        },
      ],
      extractor: {
        durationMs: 18,
        result: { extracted: true, count: 5 },
      },
    },
  };
}

function debugResultWithConfigurator(): DebugResult {
  return {
    durationMs: 250,
    result: { data: 'from-configured-request' },
    target: {
      type: 'api',
      params: { secrets: { token: 'cfg-secret' } },
      configurator: {
        durationMs: 12,
        result: { type: 'requests', count: 1 },
      },
      requests: [
        {
          index: 0,
          source: 'configurator',
          url: 'https://api.example.com/configured',
          method: 'POST',
          statusCode: 201,
          requestHeaders: { 'content-type': 'application/json', authorization: 'Bearer token' },
          requestBody: { query: 'test' },
          responseHeaders: { 'content-type': 'application/json' },
          responseBodyRaw: '{"data":"from-configured-request"}',
          responseBodyRawSize: 34,
          durationMs: 180,
        },
      ],
    },
  };
}

function debugResultWithMultipleRequests(): DebugResult {
  return {
    durationMs: 340,
    result: [{ id: 1 }, { id: 2 }],
    target: {
      type: 'api',
      requests: [
        {
          index: 0,
          source: 'original',
          url: 'https://api.example.com/page/1',
          method: 'GET',
          statusCode: 200,
          responseHeaders: { 'content-type': 'application/json' },
          responseBodyRaw: '{"id":1}',
          responseBodyRawSize: 8,
          durationMs: 150,
        },
        {
          index: 1,
          source: 'original',
          url: 'https://api.example.com/page/2',
          method: 'GET',
          statusCode: 200,
          responseHeaders: { 'content-type': 'application/json' },
          responseBodyRaw: '{"id":2}',
          responseBodyRawSize: 8,
          durationMs: 160,
        },
      ],
    },
  };
}

function debugResultWithExtractorError(): DebugResult {
  return {
    durationMs: 180,
    error: 'Extractor script failed: TypeError: Cannot read property',
    target: {
      type: 'api',
      requests: [
        {
          index: 0,
          source: 'original',
          url: 'https://api.example.com/data',
          method: 'GET',
          statusCode: 200,
          responseHeaders: { 'content-type': 'application/json' },
          responseBodyRaw: '{"data":"ok"}',
          responseBodyRawSize: 13,
          durationMs: 120,
        },
      ],
      extractor: {
        durationMs: 5,
        error: "TypeError: Cannot read property 'items' of undefined",
      },
    },
  };
}

function debugResultWithAutoParse(): DebugResult {
  return {
    durationMs: 190,
    result: [
      ['Name', 'Age'],
      ['Alice', '30'],
    ],
    target: {
      type: 'api',
      requests: [
        {
          index: 0,
          source: 'original',
          url: 'https://api.example.com/report.csv',
          method: 'GET',
          statusCode: 200,
          responseHeaders: { 'content-type': 'text/csv' },
          responseBodyRaw: 'Name,Age\nAlice,30',
          responseBodyRawSize: 18,
          responseBodyParsed: [
            ['Name', 'Age'],
            ['Alice', '30'],
          ],
          autoParse: { mediaType: 'text/csv', success: true },
          durationMs: 130,
        },
      ],
    },
  };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function openTrackerFlyout(page: Page) {
  await page.goto('/ws/web_scraping__api');
  const createButton = page.getByRole('button', { name: 'Track API' });
  await expect(createButton).toBeVisible({ timeout: 15000 });
  await createButton.click();

  const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add API tracker' }) });
  await expect(flyout).toBeVisible();
  return flyout;
}

async function fillUrlAndDebug(page: Page, flyout: Locator, url = REAL_TARGET_URL) {
  const urlInput = flyout.getByLabel('URL');
  await urlInput.fill(url);

  const debugButton = flyout.getByRole('button', { name: 'Debug' });
  await expect(debugButton).toBeEnabled();
  await debugButton.click();
}

function getDebugModal(page: Page) {
  return page.locator('[data-test-subj="debug-modal"]');
}

function mockDebugEndpoint(page: Page, result: DebugResult) {
  return page.route('**/api/utils/web_scraping/api/debug', async (route) => {
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

test.describe('API Tracker Debug Panel', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test.describe('basic controls', () => {
    test('shows debug button disabled when URL is empty', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);
      const debugButton = flyout.getByRole('button', { name: 'Debug' });
      await expect(debugButton).toBeVisible();
      await expect(debugButton).toBeDisabled();
    });

    test('enables debug button after filling URL', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);
      const urlInput = flyout.getByLabel('URL');
      await urlInput.fill('https://api.example.com/data');

      const debugButton = flyout.getByRole('button', { name: 'Debug' });
      await expect(debugButton).toBeEnabled();
    });

    test('shows error on network failure', async ({ page }) => {
      await page.route('**/api/utils/web_scraping/api/debug', async (route) => {
        await route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ message: 'Internal server error' }),
        });
      });

      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      await expect(modal).toBeVisible();
      await expect(modal.getByText('Internal server error', { exact: false })).toBeVisible({ timeout: 15000 });
    });
  });

  test.describe('simple request pipeline (real backend)', () => {
    test('shows Request and Result steps for a real GET', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      await expect(modal).toBeVisible();

      const requestStep = modal.getByRole('button', { name: 'Request' });
      await expect(requestStep).toBeVisible({ timeout: 30000 });

      const resultStep = modal.getByRole('button', { name: 'Result' });
      await expect(resultStep).toBeVisible();

      await expect(modal.getByRole('button', { name: /Configurator/ })).not.toBeVisible();
      await expect(modal.getByRole('button', { name: /Extractor/ })).not.toBeVisible();
    });

    test('shows result detail by default with total duration', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      await expect(modal.getByRole('button', { name: 'Result' })).toBeVisible({ timeout: 30000 });

      await expect(modal.getByText(/\d+ms total/)).toBeVisible();
    });

    test('shows request detail when clicking Request step', async ({ page }) => {
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const requestStep = modal.getByRole('button', { name: 'Request' });
      await expect(requestStep).toBeVisible({ timeout: 30000 });
      await requestStep.click();

      await expect(modal.getByRole('strong').filter({ hasText: 'GET' })).toBeVisible();
      await expect(modal.getByText(REAL_TARGET_URL)).toBeVisible();

      await expect(modal.getByRole('tab', { name: 'Response Body' })).toBeVisible();
      await expect(modal.getByRole('tab', { name: 'Response Headers' })).toBeVisible();
    });
  });

  test.describe('with extractor (mocked)', () => {
    test('shows Extractor step in the pipeline', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithExtractor());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      await expect(modal).toBeVisible();
      await expect(modal.getByRole('button', { name: 'Request' })).toBeVisible({ timeout: 15000 });
      await expect(modal.getByRole('button', { name: 'Extractor' })).toBeVisible();
      await expect(modal.getByRole('button', { name: 'Result' })).toBeVisible();
    });

    test('shows extractor detail with Result and Params tabs', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithExtractor());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const extractorStep = modal.getByRole('button', { name: 'Extractor' });
      await expect(extractorStep).toBeVisible({ timeout: 15000 });
      await extractorStep.click();

      await expect(modal.getByText('18ms', { exact: true })).toBeVisible();

      const resultTab = modal.getByRole('tab', { name: 'Result' });
      await expect(resultTab).toBeVisible();
      await expect(resultTab).toHaveAttribute('aria-selected', 'true');
      await expect(modal.getByText('"extracted"')).toBeVisible();

      const paramsTab = modal.getByRole('tab', { name: 'Params' });
      await expect(paramsTab).toBeVisible();
      await paramsTab.click();
      await expect(modal.getByText('test-key-123')).toBeVisible();
    });

    test('shows extractor error in Result tab when script fails', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithExtractorError());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const extractorStep = modal.getByRole('button', { name: 'Extractor' });
      await expect(extractorStep).toBeVisible({ timeout: 15000 });
      await extractorStep.click();

      await expect(modal.getByText('Extractor script failed')).toBeVisible();
      await expect(modal.getByText("Cannot read property 'items'", { exact: false })).toBeVisible();

      // No params in this fixture, so Params tab should not appear.
      await expect(modal.getByRole('tab', { name: 'Params' })).not.toBeVisible();
    });
  });

  test.describe('with configurator (mocked)', () => {
    test('shows Configurator step in the pipeline', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithConfigurator());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      await expect(modal).toBeVisible();
      await expect(modal.getByRole('button', { name: 'Configurator' })).toBeVisible({ timeout: 15000 });
      await expect(modal.getByRole('button', { name: 'Request' })).toBeVisible();
      await expect(modal.getByRole('button', { name: 'Result' })).toBeVisible();
    });

    test('shows configurator detail with Result and Params tabs', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithConfigurator());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const configuratorStep = modal.getByRole('button', { name: 'Configurator' });
      await expect(configuratorStep).toBeVisible({ timeout: 15000 });
      await configuratorStep.click();

      await expect(modal.getByText('12ms', { exact: true })).toBeVisible();

      const resultTab = modal.getByRole('tab', { name: 'Result' });
      await expect(resultTab).toBeVisible();
      await expect(resultTab).toHaveAttribute('aria-selected', 'true');
      await expect(modal.getByText('"requests"')).toBeVisible();

      const paramsTab = modal.getByRole('tab', { name: 'Params' });
      await expect(paramsTab).toBeVisible();
      await paramsTab.click();
      await expect(modal.getByText('cfg-secret')).toBeVisible();
    });

    test('shows request source badge for configurator-modified request', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithConfigurator());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const requestStep = modal.getByRole('button', { name: 'Request' });
      await expect(requestStep).toBeVisible({ timeout: 15000 });
      await requestStep.click();

      await expect(modal.getByText('configurator', { exact: true })).toBeVisible();
      await expect(modal.getByRole('strong').filter({ hasText: 'POST' })).toBeVisible();
      await expect(modal.getByText('https://api.example.com/configured')).toBeVisible();
    });
  });

  test.describe('multiple requests (mocked)', () => {
    test('shows numbered Request steps when there are multiple', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithMultipleRequests());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      await expect(modal.getByRole('button', { name: 'Request #1' })).toBeVisible({ timeout: 15000 });
      await expect(modal.getByRole('button', { name: 'Request #2' })).toBeVisible();
      await expect(modal.getByRole('button', { name: 'Result' })).toBeVisible();
    });

    test('can navigate between request steps', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithMultipleRequests());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const req1 = modal.getByRole('button', { name: 'Request #1' });
      await expect(req1).toBeVisible({ timeout: 15000 });
      await req1.click();

      await expect(modal.getByText('https://api.example.com/page/1')).toBeVisible();

      const req2 = modal.getByRole('button', { name: 'Request #2' });
      await req2.click();

      await expect(modal.getByText('https://api.example.com/page/2')).toBeVisible();
    });
  });

  test.describe('error states (mocked)', () => {
    test('shows error callout in Result when pipeline fails', async ({ page }) => {
      const errorResult: DebugResult = {
        durationMs: 55,
        error: 'Failed to execute the API request (0): 500 Internal Server Error',
        target: {
          type: 'api',
          requests: [
            {
              index: 0,
              source: 'original',
              url: 'https://api.example.com/data',
              method: 'GET',
              statusCode: 500,
              responseHeaders: { 'content-type': 'text/plain' },
              responseBodyRaw: 'Internal Server Error',
              responseBodyRawSize: 21,
              durationMs: 50,
              error: '500 Internal Server Error',
            },
          ],
        },
      };
      await mockDebugEndpoint(page, errorResult);
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const resultStep = modal.getByRole('button', { name: 'Result' });
      await expect(resultStep).toBeVisible({ timeout: 15000 });

      await expect(modal.getByText('Pipeline failed')).toBeVisible();
      await expect(modal.getByText('500 Internal Server Error', { exact: false })).toBeVisible();
    });

    test('shows error callout in Request step detail', async ({ page }) => {
      const errorResult: DebugResult = {
        durationMs: 55,
        error: 'Failed to execute the API request (0): 500 Internal Server Error',
        target: {
          type: 'api',
          requests: [
            {
              index: 0,
              source: 'original',
              url: 'https://api.example.com/data',
              method: 'GET',
              statusCode: 500,
              responseHeaders: { 'content-type': 'text/plain' },
              responseBodyRaw: 'Internal Server Error',
              responseBodyRawSize: 21,
              durationMs: 50,
              error: '500 Internal Server Error',
            },
          ],
        },
      };
      await mockDebugEndpoint(page, errorResult);
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const requestStep = modal.getByRole('button', { name: 'Request' });
      await expect(requestStep).toBeVisible({ timeout: 15000 });
      await requestStep.click();

      await expect(modal.getByText('Request failed')).toBeVisible();
      await expect(modal.getByText('500 Internal Server Error')).toBeVisible();
    });
  });

  test.describe('auto-parse (mocked)', () => {
    test('shows auto-parse badge for CSV response', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithAutoParse());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const requestStep = modal.getByRole('button', { name: 'Request' });
      await expect(requestStep).toBeVisible({ timeout: 15000 });
      await requestStep.click();

      await expect(modal.getByText('Parsed as text/csv')).toBeVisible();
    });
  });

  test.describe('request detail tabs (mocked)', () => {
    test('shows Response Body tab by default', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithConfigurator());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const requestStep = modal.getByRole('button', { name: 'Request' });
      await expect(requestStep).toBeVisible({ timeout: 15000 });
      await requestStep.click();

      const responseBodyTab = modal.getByRole('tab', { name: 'Response Body' });
      await expect(responseBodyTab).toBeVisible();
      await expect(responseBodyTab).toHaveAttribute('aria-selected', 'true');
    });

    test('shows response headers when clicking Response Headers tab', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithConfigurator());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const requestStep = modal.getByRole('button', { name: 'Request' });
      await expect(requestStep).toBeVisible({ timeout: 15000 });
      await requestStep.click();

      const responseHeadersTab = modal.getByRole('tab', { name: 'Response Headers' });
      await expect(responseHeadersTab).toBeVisible();
      await responseHeadersTab.click();

      await expect(modal.getByText('content-type: application/json')).toBeVisible();
    });

    test('shows request headers when clicking Request Headers tab', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithConfigurator());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const requestStep = modal.getByRole('button', { name: 'Request' });
      await expect(requestStep).toBeVisible({ timeout: 15000 });
      await requestStep.click();

      const requestHeadersTab = modal.getByRole('tab', { name: 'Request Headers' });
      await expect(requestHeadersTab).toBeVisible();
      await requestHeadersTab.click();

      await expect(modal.getByText('authorization: Bearer token')).toBeVisible();
    });

    test('shows request body when clicking Request Body tab', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithConfigurator());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const requestStep = modal.getByRole('button', { name: 'Request' });
      await expect(requestStep).toBeVisible({ timeout: 15000 });
      await requestStep.click();

      const requestBodyTab = modal.getByRole('tab', { name: 'Request Body' });
      await expect(requestBodyTab).toBeVisible();
      await requestBodyTab.click();

      await expect(modal.getByText('"query"')).toBeVisible();
    });

    test('hides Request Headers and Request Body tabs when not available', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithExtractor());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const requestStep = modal.getByRole('button', { name: 'Request' });
      await expect(requestStep).toBeVisible({ timeout: 15000 });
      await requestStep.click();

      await expect(modal.getByRole('tab', { name: 'Response Body' })).toBeVisible();
      await expect(modal.getByRole('tab', { name: 'Response Headers' })).toBeVisible();
      await expect(modal.getByRole('tab', { name: 'Request Headers' })).not.toBeVisible();
      await expect(modal.getByRole('tab', { name: 'Request Body' })).not.toBeVisible();
    });
  });

  test.describe('secrets / params (mocked)', () => {
    test('shows Params tab on extractor step when params are present', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithExtractor());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const extractorStep = modal.getByRole('button', { name: 'Extractor' });
      await expect(extractorStep).toBeVisible({ timeout: 15000 });
      await extractorStep.click();

      const paramsTab = modal.getByRole('tab', { name: 'Params' });
      await expect(paramsTab).toBeVisible();
      await paramsTab.click();

      await expect(modal.getByText('test-key-123')).toBeVisible();
    });

    test('hides Params tab when params are absent', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithExtractorError());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      const extractorStep = modal.getByRole('button', { name: 'Extractor' });
      await expect(extractorStep).toBeVisible({ timeout: 15000 });
      await extractorStep.click();

      await expect(modal.getByRole('tab', { name: 'Params' })).not.toBeVisible();
    });

    test('sends secrets in the debug request body', async ({ page }) => {
      let capturedSecrets: unknown = null;
      await page.route('**/api/utils/web_scraping/api/debug', async (route) => {
        const postData = route.request().postDataJSON();
        capturedSecrets = postData?.secrets;
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(debugResultWithExtractor()),
        });
      });

      const flyout = await openTrackerFlyout(page);

      // Enable Advanced mode and select "All secrets".
      await flyout.getByLabel('Advanced mode').check();
      const secretsSelect = flyout.getByRole('combobox', { name: 'Access mode' });
      await secretsSelect.selectOption('all');

      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      await expect(modal.getByRole('button', { name: 'Result' })).toBeVisible({ timeout: 15000 });

      expect(capturedSecrets).toEqual({ type: 'all' });
    });

    test('sends secrets none when no secrets configured', async ({ page }) => {
      let capturedSecrets: unknown = null;
      await page.route('**/api/utils/web_scraping/api/debug', async (route) => {
        const postData = route.request().postDataJSON();
        capturedSecrets = postData?.secrets;
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(debugResultWithExtractor()),
        });
      });

      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      await expect(modal.getByRole('button', { name: 'Result' })).toBeVisible({ timeout: 15000 });

      expect(capturedSecrets).toEqual({ type: 'none' });
    });
  });

  test.describe('modal behavior', () => {
    test('closes modal when clicking close button', async ({ page }) => {
      await mockDebugEndpoint(page, debugResultWithExtractor());
      const flyout = await openTrackerFlyout(page);
      await fillUrlAndDebug(page, flyout);

      const modal = getDebugModal(page);
      await expect(modal).toBeVisible();
      await expect(modal.getByRole('button', { name: 'Result' })).toBeVisible({ timeout: 15000 });

      await page.keyboard.press('Escape');
      await expect(modal).not.toBeVisible();

      // Flyout should still be open.
      await expect(flyout).toBeVisible();
    });
  });
});
