import { join } from 'path';

import { expect, test } from '@playwright/test';

import {
  dismissAllToasts,
  DOCS_IMG_DIR,
  ensureUserAndLogin,
  fixResponderRequestFields,
  goto,
  highlightOn,
} from './helpers';

const IMG_DIR = join(DOCS_IMG_DIR, 'webhooks');

test.describe('Webhooks guide screenshots', () => {
  test.beforeEach(async ({ page, request }) => {
    await ensureUserAndLogin(request, page);
  });

  test('Return a static HTML page', async ({ page }) => {
    // Step 1: Navigate to responders and show the empty state.
    await goto(page, '/ws/webhooks__responders');
    const createButton = page.getByRole('button', { name: 'Create responder' });
    await expect(createButton).toBeVisible({ timeout: 15000 });
    await highlightOn(createButton);
    await page.screenshot({ path: join(IMG_DIR, 'html_step1_empty.png') });

    // Step 2: Open the flyout and fill in the form.
    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add responder' }) });
    await expect(flyout).toBeVisible();

    await flyout.getByLabel('Name').fill('HTML Responder');
    await flyout.getByLabel('Path', { exact: true }).fill('/html-responder');

    const htmlBody = [
      '<!DOCTYPE html>',
      '<html lang="en">',
      '<head>',
      '    <title>My HTML responder</title>',
      '</head>',
      '<body>Hello World</body>',
      '</html>',
    ].join('\n');
    const bodyTextarea = flyout.getByLabel('Body');
    await bodyTextarea.fill(htmlBody);
    await bodyTextarea.evaluate((el) => (el.scrollTop = 0));

    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveButton);
    await page.screenshot({ path: join(IMG_DIR, 'html_step2_form.png') });

    // Step 3: Save and verify the responder appears in the grid.
    await saveButton.click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    const responderRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'HTML Responder' }) });
    await expect(responderRow).toBeVisible();
    await highlightOn(responderRow);
    await page.screenshot({ path: join(IMG_DIR, 'html_step3_created.png') });

    await dismissAllToasts(page);

    // Step 4: Open the responder URL and verify it renders the HTML page.
    const responderLink = responderRow.getByRole('link');
    const responderUrl = await responderLink.getAttribute('href');
    const htmlPage = await page.context().newPage();
    await goto(htmlPage, responderUrl!);
    await expect(htmlPage.getByText('Hello World')).toBeVisible({ timeout: 15000 });
    await htmlPage.screenshot({ path: join(IMG_DIR, 'html_step4_result.png') });
    await htmlPage.close();
  });

  test('Emulate a JSON API endpoint', async ({ page }) => {
    // Step 1: Navigate to responders and show the empty state.
    await goto(page, '/ws/webhooks__responders');
    const createButton = page.getByRole('button', { name: 'Create responder' });
    await expect(createButton).toBeVisible({ timeout: 15000 });
    await highlightOn(createButton);
    await page.screenshot({ path: join(IMG_DIR, 'json_step1_empty.png') });

    // Step 2: Open the flyout and fill in the form.
    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add responder' }) });
    await expect(flyout).toBeVisible();

    await flyout.getByLabel('Name').fill('JSON Responder');
    await flyout.getByLabel('Path', { exact: true }).fill('/json-responder');

    // Remove the default Content-Type header and add the JSON one.
    await flyout.getByRole('button', { name: /Remove Content-Type/ }).click();
    const headersCombo = flyout.getByRole('combobox', { name: 'Headers' });
    await headersCombo.fill('Content-Type: application/json');
    await headersCombo.press('Enter');

    const jsonBody = ['{\n', '  "message": "Hello World"\n', '}'].join('');
    const bodyTextarea = flyout.getByLabel('Body');
    await bodyTextarea.fill(jsonBody);
    await bodyTextarea.evaluate((el) => (el.scrollTop = 0));

    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveButton);
    await page.screenshot({ path: join(IMG_DIR, 'json_step2_form.png') });

    // Step 3: Save and verify the responder appears in the grid.
    await saveButton.click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    const responderRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'JSON Responder' }) });
    await expect(responderRow).toBeVisible();
    await highlightOn(responderRow);
    await page.screenshot({ path: join(IMG_DIR, 'json_step3_created.png') });
  });

  test('Use the honeypot endpoint to inspect incoming requests', async ({ page }) => {
    // Step 1: Navigate to responders and show the empty state.
    await goto(page, '/ws/webhooks__responders');
    const createButton = page.getByRole('button', { name: 'Create responder' });
    await expect(createButton).toBeVisible({ timeout: 15000 });
    await highlightOn(createButton);
    await page.screenshot({ path: join(IMG_DIR, 'tracking_step1_empty.png') });

    // Step 2: Open the flyout and fill in the form with tracking enabled.
    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add responder' }) });
    await expect(flyout).toBeVisible();

    await flyout.getByLabel('Name').fill('Notion Honeypot');
    await flyout.getByLabel('Path', { exact: true }).fill('/notion-honeypot');

    await flyout.getByLabel('Advanced mode').click();
    await flyout.getByRole('slider').fill('5');

    const honeypotBody = [
      '<!DOCTYPE html>',
      '<html lang="en">',
      '<head>',
      '  <meta property="iframely:image"',
      '        content="https://raw.githubusercontent.com/secutils-dev/secutils/main/assets/logo/secutils-logo-initials.png" />',
      '  <meta property="iframely:description"',
      '        content="Inspect incoming HTTP request headers and body with the honeypot endpoint" />',
      '  <title>My HTML responder</title>',
      '</head>',
      '<body>Hello World</body>',
      '</html>',
    ].join('\n');
    const bodyTextarea = flyout.getByLabel('Body');
    await bodyTextarea.fill(honeypotBody);
    await bodyTextarea.evaluate((el) => (el.scrollTop = 0));

    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveButton);
    await page.screenshot({ path: join(IMG_DIR, 'tracking_step2_form.png') });

    // Step 3: Save and verify the responder appears in the grid.
    await saveButton.click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    const responderRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Notion Honeypot' }) });
    await expect(responderRow).toBeVisible();
    await highlightOn(responderRow);
    await page.screenshot({ path: join(IMG_DIR, 'tracking_step3_created.png') });

    await dismissAllToasts(page);

    // Step 4: Call the endpoint and expand the row to show tracked requests.
    await fixResponderRequestFields(page);
    const responderLink = responderRow.getByRole('link');
    const responderUrl = await responderLink.getAttribute('href');
    const honeypotPage = await page.context().newPage();
    await goto(honeypotPage, responderUrl!);
    await expect(honeypotPage.getByText('Hello World')).toBeVisible({ timeout: 15000 });
    await honeypotPage.close();

    // Wait for auto-refresh to pick up the request, then expand the row.
    await page.waitForTimeout(4000);
    await responderRow.getByRole('button', { name: 'Show requests' }).click();
    const requestsGrid = page.getByRole('grid', { name: 'Requests' });
    await expect(requestsGrid).toBeVisible({ timeout: 10000 });
    await page.screenshot({ path: join(IMG_DIR, 'tracking_step4_request.png') });
  });

  test('Generate a dynamic response', async ({ page }) => {
    const scriptBody = [
      '(async () => {',
      '  return {',
      '    body: Deno.core.encode(',
      "      context.query.arg ?? 'Query string does not include `arg` parameter'",
      '    )',
      '  };',
      '})();',
    ].join('\n');

    // Step 1: Navigate to responders and show the empty state.
    await goto(page, '/ws/webhooks__responders');
    const createButton = page.getByRole('button', { name: 'Create responder' });
    await expect(createButton).toBeVisible({ timeout: 15000 });
    await highlightOn(createButton);
    await page.screenshot({ path: join(IMG_DIR, 'dynamic_step1_empty.png') });

    // Create the responder via API (Monaco editor cannot be reliably filled via Playwright).
    const createResponse = await page.request.post('/api/utils/webhooks/responders', {
      data: {
        name: 'Dynamic',
        location: { pathType: '=', path: '/dynamic' },
        method: 'ANY',
        enabled: true,
        settings: {
          requestsToTrack: 5,
          statusCode: 200,
          headers: [['Content-Type', 'text/html; charset=utf-8']],
          script: scriptBody,
        },
      },
    });
    expect(createResponse.ok()).toBeTruthy();

    // Step 2: Reload to see the responder, open Edit, and screenshot the form with the script.
    await goto(page, '/ws/webhooks__responders');
    const responderRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Dynamic' }) });
    await expect(responderRow).toBeVisible({ timeout: 15000 });

    await responderRow.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit responder' }) });
    await expect(flyout).toBeVisible();

    // Scroll the flyout so the Script editor is visible.
    await flyout.getByText('The script is executed').scrollIntoViewIfNeeded();
    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveButton);
    await page.screenshot({ path: join(IMG_DIR, 'dynamic_step2_form.png') });

    await flyout.getByRole('button', { name: 'Close' }).click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    // Step 3: Show the responder in the grid.
    await highlightOn(responderRow);
    await page.screenshot({ path: join(IMG_DIR, 'dynamic_step3_created.png') });

    // Step 4: Open the responder URL without args - shows the default message.
    const responderLink = responderRow.getByRole('link');
    const responderUrl = await responderLink.getAttribute('href');
    const noArgPage = await page.context().newPage();
    await goto(noArgPage, responderUrl!);
    await expect(noArgPage.getByText('Query string does not include')).toBeVisible({ timeout: 15000 });
    await noArgPage.screenshot({ path: join(IMG_DIR, 'dynamic_step4_no_arg.png') });
    await noArgPage.close();

    // Step 5: Open the responder URL with ?arg=hello - shows the dynamic reply.
    const argPage = await page.context().newPage();
    await goto(argPage, `${responderUrl!}?arg=hello`);
    await expect(argPage.getByText('hello')).toBeVisible({ timeout: 15000 });
    await argPage.screenshot({ path: join(IMG_DIR, 'dynamic_step5_with_arg.png') });
    await argPage.close();
  });
});
