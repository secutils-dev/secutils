import { join } from 'path';

import { expect, type Locator, test } from '@playwright/test';

import {
  dismissAllToasts,
  DOCS_IMG_DIR,
  EMAIL,
  ensureUserAndLogin,
  fixEntityTimestamps,
  fixResponderRequestFields,
  goto,
  highlightOff,
  highlightOn,
  PASSWORD,
  pinEntityTimestamps,
} from '../helpers';

const IMG_DIR = join(DOCS_IMG_DIR, 'csp');

function getByRoleAndLabel(parent: Locator, role: 'combobox' | 'textbox', label: string) {
  return parent.locator(`:below(label:text("${label}"))`).getByRole(role).first();
}

test.describe('CSP guide screenshots', () => {
  test.beforeEach(async ({ page, request }) => {
    await ensureUserAndLogin(request, page, { email: EMAIL, password: PASSWORD });
    await fixEntityTimestamps(page, '**/api/utils/web_security/csp');
    await fixEntityTimestamps(page, '**/api/utils/webhooks/responders');
  });

  test('create a content security policy', async ({ page }) => {
    await goto(page, '/ws/web_security__csp__policies');

    // Empty policies list.
    const createButton = page.getByRole('button', { name: 'Create policy' });
    await expect(createButton).toBeVisible({ timeout: 15000 });
    await highlightOn(createButton);
    await page.screenshot({ path: join(IMG_DIR, 'create_step1_empty.png') });

    // Show the policy flyout.
    await page.getByRole('button', { name: 'Create policy' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add policy' }) });
    await expect(flyout).toBeVisible();

    // Fill in the form.
    await flyout.getByLabel('Name').fill('secutils.dev');

    const defaultSrcDirective = getByRoleAndLabel(flyout, 'combobox', 'Default source (default-src)');
    await defaultSrcDirective.fill("'self'");
    await page.keyboard.press('Enter');
    await defaultSrcDirective.fill('api.secutils.dev');
    await page.keyboard.press('Enter');

    const styleSrcDirective = getByRoleAndLabel(flyout, 'combobox', 'Style source (style-src)');
    await styleSrcDirective.fill("'self'");
    await page.keyboard.press('Enter');
    await styleSrcDirective.fill('fonts.googleapis.com');
    await page.keyboard.press('Enter');

    await page.keyboard.press('Escape');

    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveButton);
    await page.screenshot({ path: join(IMG_DIR, 'create_step2_form.png') });

    // Save the policy and verify it's created.
    await saveButton.click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'secutils.dev' }) });
    await expect(row).toBeVisible();
    await highlightOn(row);
    await page.screenshot({ path: join(IMG_DIR, 'create_step3_created.png') });

    await dismissAllToasts(page);

    // Open the actions menu and click "Copy".
    const grid = page.getByRole('table');
    await grid.getByRole('button', { name: 'All actions, row' }).click();
    await page.getByRole('button', { name: 'Copy', exact: true }).click();
    await grid
      .getByRole('dialog')
      .filter({ has: page.getByRole('heading', { name: 'Copy policy' }) })
      .isVisible({ timeout: 10000 });
    await page.screenshot({ path: join(IMG_DIR, 'create_step4_copy.png') });
  });

  test('import policy from URL', async ({ page }) => {
    // Replace nonce values and pin timestamps in API responses so screenshots are stable.
    await page.route('**/api/utils/web_security/csp', async (route) => {
      if (route.request().method() === 'GET') {
        const response = await route.fetch();
        let body = await response.text();
        body = body.replace(/'nonce-[^']+'/g, "'nonce-m0ck'");
        const json = JSON.parse(body);
        pinEntityTimestamps(json);
        await route.fulfill({ response, json });
      } else {
        await route.continue();
      }
    });

    await goto(page, '/ws/web_security__csp__policies');

    // Empty policies list.
    const importButton = page.getByRole('button', { name: 'Import policy' });
    await expect(importButton).toBeVisible({ timeout: 15000 });
    await highlightOn(importButton);
    await page.screenshot({ path: join(IMG_DIR, 'import_url_step1_empty.png') });

    // Open the import modal.
    await importButton.click();
    const modal = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Import policy' }) });
    await expect(modal).toBeVisible();

    // Enter the URL and fetch the policy.
    await modal.getByRole('tab', { name: 'URL', exact: true }).click();
    await getByRoleAndLabel(modal, 'textbox', 'Policy name').fill('Google CSP');
    await getByRoleAndLabel(modal, 'textbox', 'URL').fill('https://google.com');
    await getByRoleAndLabel(modal, 'combobox', 'Policy source').selectOption('HTTP header (report only)');

    const importDialogButton = modal.getByRole('button', { name: 'Import' });
    await highlightOn(importDialogButton);

    await page.screenshot({ path: join(IMG_DIR, 'import_url_step2_modal.png') });

    // Import the policy and verify it's created.
    await importDialogButton.click();
    await expect(modal).not.toBeVisible({ timeout: 10000 });

    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Google CSP' }) });
    await expect(row).toBeVisible();
    await highlightOn(row);
    await page.screenshot({ path: join(IMG_DIR, 'import_url_step3_created.png') });
  });

  test('import policy from string', async ({ page }) => {
    await goto(page, '/ws/web_security__csp__policies');

    // Empty policies list.
    const importButton = page.getByRole('button', { name: 'Import policy' });
    await expect(importButton).toBeVisible({ timeout: 15000 });
    await highlightOn(importButton);
    await page.screenshot({ path: join(IMG_DIR, 'import_string_step1_empty.png') });

    // Open the import modal.
    await importButton.click();
    const modal = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Import policy' }) });
    await expect(modal).toBeVisible();

    // Enter serialized policy policy.
    await getByRoleAndLabel(modal, 'textbox', 'Policy name').fill('Custom CSP');
    await getByRoleAndLabel(modal, 'textbox', 'Serialized policy').fill(
      "default-src 'self' api.secutils.dev; style-src 'self' fonts.googleapis.com",
    );

    const importDialogButton = modal.getByRole('button', { name: 'Import' });
    await highlightOn(importDialogButton);

    await page.screenshot({ path: join(IMG_DIR, 'import_string_step2_modal.png') });

    // Import the policy and verify it's created.
    await importDialogButton.click();
    await expect(modal).not.toBeVisible({ timeout: 10000 });

    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Custom CSP' }) });
    await expect(row).toBeVisible();
    await highlightOn(row);
    await page.screenshot({ path: join(IMG_DIR, 'import_string_step3_created.png') });
  });

  test('test a content security policy', async ({ page }) => {
    // Step 1: Create a responder via API (Monaco editor cannot be reliably filled via Playwright).
    const body = [
      '<!DOCTYPE html>',
      '<html lang="en">',
      '<head>',
      '  <title>Evaluate CSP</title>',
      '</head>',
      '<body>',
      '<label for="eval-input">Expression to evaluate:</label>',
      '<input id="eval-input" type="text" value="alert(\'xss\')"/>',
      '<button id="eval-test">Eval</button>',
      '<script type="text/javascript" defer>',
      '  (async function main() {',
      "    const evalTestBtn = document.getElementById('eval-test');",
      "    evalTestBtn.addEventListener('click', () => {",
      "      const evalExpression = document.getElementById('eval-input');",
      '      window.eval(evalExpression.value);',
      '    });',
      '  })();',
      '</script>',
      '</body>',
      '</html>',
    ].join('\n');
    const createResponse = await page.request.post('/api/utils/webhooks/responders', {
      data: {
        name: 'CSP Test',
        location: { pathType: '=', path: '/csp-test' },
        method: 'ANY',
        enabled: true,
        settings: {
          requestsToTrack: 10,
          statusCode: 200,
          headers: [['Content-Type', 'text/html; charset=utf-8']],
          body,
        },
      },
    });
    expect(createResponse.ok()).toBeTruthy();

    // Reload to see the responder, open Edit, and screenshot the form.
    await goto(page, '/ws/webhooks__responders');
    const responderRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'CSP Test' }) });
    await expect(responderRow).toBeVisible({ timeout: 15000 });

    await responderRow.getByRole('button', { name: 'Edit' }).click();
    const responderFlyout = page
      .getByRole('dialog')
      .filter({ has: page.getByRole('heading', { name: 'Edit responder' }) });
    await expect(responderFlyout).toBeVisible();

    await responderFlyout.getByText('Body', { exact: true }).scrollIntoViewIfNeeded();
    const saveResponderButton = responderFlyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveResponderButton);
    await page.screenshot({ path: join(IMG_DIR, 'test_step1_responder_form.png') });

    await responderFlyout.getByRole('button', { name: 'Close' }).click();
    await expect(responderFlyout).not.toBeVisible({ timeout: 10000 });

    await highlightOn(responderRow);
    await page.screenshot({ path: join(IMG_DIR, 'test_step2_responder_created.png') });

    await dismissAllToasts(page);

    // Step 2b: Open the responder URL and capture the eval page.
    const responderLink = responderRow.getByRole('link');
    const responderUrl = await responderLink.getAttribute('href');
    const evalPage = await page.context().newPage();
    await goto(evalPage, responderUrl!);
    const evalButton = evalPage.getByRole('button', { name: 'Eval' });
    await expect(evalButton).toBeVisible({ timeout: 15000 });
    await highlightOn(evalButton);
    evalPage.once('dialog', (dialog) => dialog.dismiss());
    await evalButton.click();
    await evalPage.screenshot({ path: join(IMG_DIR, 'test_step2b_eval_page.png') });
    await evalPage.close();

    // Step 2: Create a CSP policy that forbids eval().
    await goto(page, '/ws/web_security__csp__policies');
    const createPolicyButton = page.getByRole('button', { name: 'Create policy' });
    await expect(createPolicyButton).toBeVisible({ timeout: 15000 });
    await createPolicyButton.click();

    const policyFlyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add policy' }) });
    await expect(policyFlyout).toBeVisible();

    await policyFlyout.getByLabel('Name').fill('CSP Test');

    const scriptSrcDirective = getByRoleAndLabel(policyFlyout, 'combobox', 'Script source (script-src)');
    await scriptSrcDirective.fill("'self'");
    await page.keyboard.press('Enter');
    await scriptSrcDirective.fill("'unsafe-inline'");
    await page.keyboard.press('Enter');

    await page.keyboard.press('Escape');

    const savePolicyButton = policyFlyout.getByRole('button', { name: 'Save' });
    await highlightOn(savePolicyButton);
    await page.screenshot({ path: join(IMG_DIR, 'test_step3_policy_form.png') });

    await savePolicyButton.click();
    await expect(policyFlyout).not.toBeVisible({ timeout: 10000 });

    const policyRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'CSP Test' }) });
    await expect(policyRow).toBeVisible();
    await highlightOn(policyRow);
    await page.screenshot({ path: join(IMG_DIR, 'test_step4_policy_created.png') });

    await dismissAllToasts(page);

    // Step 3: Copy the policy as an HTML meta-tag.
    const grid = page.getByRole('table');
    await grid.getByRole('button', { name: 'All actions, row' }).click();
    await page.getByRole('button', { name: 'Copy', exact: true }).click();

    const copyModal = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Copy policy' }) });
    await expect(copyModal).toBeVisible({ timeout: 10000 });

    await getByRoleAndLabel(copyModal, 'combobox', 'Policy source').selectOption('HTML meta tag');
    await highlightOn(copyModal.locator('.euiCodeBlock'));

    await page.screenshot({ path: join(IMG_DIR, 'test_step5_copy_meta_tag.png') });

    // Step 4: Update the responder body via API to add the CSP meta-tag.
    const updatedBody = [
      '<!DOCTYPE html>',
      '<html lang="en">',
      '<head>',
      '  <meta http-equiv="Content-Security-Policy"',
      "        content=\"script-src 'self' 'unsafe-inline'\">",
      '  <title>Evaluate CSP</title>',
      '</head>',
      '<body>',
      '<label for="eval-input">Expression to evaluate:</label>',
      '<input id="eval-input" type="text" value="alert(\'xss\')"/>',
      '<button id="eval-test">Eval</button>',
      '<script type="text/javascript" defer>',
      '  (async function main() {',
      "    const evalTestBtn = document.getElementById('eval-test');",
      "    evalTestBtn.addEventListener('click', () => {",
      "      const evalExpression = document.getElementById('eval-input');",
      '      window.eval(evalExpression.value);',
      '    });',
      '  })();',
      '</script>',
      '</body>',
      '</html>',
    ].join('\n');

    const respondersResponse = await page.request.get('/api/utils/webhooks/responders');
    const responders = await respondersResponse.json();
    const cspTestResponder = responders.find((r: { name: string }) => r.name === 'CSP Test');
    const updateResponse = await page.request.put(`/api/utils/webhooks/responders/${cspTestResponder.id}`, {
      data: {
        settings: {
          requestsToTrack: 10,
          statusCode: 200,
          headers: [['Content-Type', 'text/html; charset=utf-8']],
          body: updatedBody,
        },
      },
    });
    expect(updateResponse.ok()).toBeTruthy();

    // Open the Edit flyout to screenshot the updated body with the meta-tag.
    await goto(page, '/ws/webhooks__responders');
    const editRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'CSP Test' }) });
    await expect(editRow).toBeVisible({ timeout: 15000 });

    await editRow.getByRole('button', { name: 'Edit' }).click();
    const editFlyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit responder' }) });
    await expect(editFlyout).toBeVisible();

    await editFlyout.getByText('Body', { exact: true }).scrollIntoViewIfNeeded();
    // Body is the first Monaco editor in the flyout; Script is the second.
    const editorContainer = editFlyout.locator('.monaco-editor').first();
    await editorContainer.evaluate((el) => {
      el.style.outline = '3px dashed red';
      el.style.outlineOffset = '3px';
      el.style.borderRadius = '5px';
    });

    await page.screenshot({ path: join(IMG_DIR, 'test_step6_responder_meta_tag.png') });
  });

  test('report content security policy violations', async ({ page }) => {
    // Step 1: Create a "CSP Reporting" responder to collect violation reports.
    await goto(page, '/ws/webhooks__responders');
    const createResponderButton = page.getByRole('button', { name: 'Create responder' });
    await expect(createResponderButton).toBeVisible({ timeout: 15000 });
    await createResponderButton.click();

    const reportingFlyout = page
      .getByRole('dialog')
      .filter({ has: page.getByRole('heading', { name: 'Add responder' }) });
    await expect(reportingFlyout).toBeVisible();

    await reportingFlyout.getByLabel('Name').fill('CSP Reporting');
    await getByRoleAndLabel(reportingFlyout, 'textbox', 'Path').fill('/csp-reporting');

    await reportingFlyout.getByLabel('Advanced mode').click();
    await reportingFlyout.getByLabel('Method').selectOption('POST');
    await reportingFlyout.getByRole('slider').fill('10');

    const saveReportingButton = reportingFlyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveReportingButton);
    await page.screenshot({ path: join(IMG_DIR, 'report_step1_reporting_form.png') });

    await saveReportingButton.click();
    await expect(reportingFlyout).not.toBeVisible({ timeout: 10000 });

    const reportingRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'CSP Reporting' }) });
    await expect(reportingRow).toBeVisible();

    const reportingUrl = await reportingRow.getByRole('link').getAttribute('href');

    await highlightOn(reportingRow);
    await page.screenshot({ path: join(IMG_DIR, 'report_step2_reporting_created.png') });
    await highlightOff(reportingRow);

    await dismissAllToasts(page);

    // Step 2: Create a CSP policy with report-uri pointing to the reporting responder.
    await goto(page, '/ws/web_security__csp__policies');
    const createPolicyButton = page.getByRole('button', { name: 'Create policy' });
    await expect(createPolicyButton).toBeVisible({ timeout: 15000 });
    await createPolicyButton.click();

    const policyFlyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add policy' }) });
    await expect(policyFlyout).toBeVisible();

    await policyFlyout.getByLabel('Name').fill('CSP Reporting');

    const scriptSrcDirective = getByRoleAndLabel(policyFlyout, 'combobox', 'Script source (script-src)');
    await scriptSrcDirective.fill("'self'");
    await page.keyboard.press('Enter');
    await scriptSrcDirective.fill("'unsafe-inline'");
    await page.keyboard.press('Enter');
    await scriptSrcDirective.fill("'report-sample'");
    await page.keyboard.press('Enter');

    await page.keyboard.press('Escape');

    const reportUriField = getByRoleAndLabel(policyFlyout, 'textbox', 'Report URI (report-uri)');
    await reportUriField.fill(reportingUrl!);

    await policyFlyout.getByText('DEPRECATED', { exact: false }).scrollIntoViewIfNeeded();

    const savePolicyButton = policyFlyout.getByRole('button', { name: 'Save' });
    await highlightOn(savePolicyButton);
    await page.screenshot({ path: join(IMG_DIR, 'report_step3_policy_form.png') });

    await savePolicyButton.click();
    await expect(policyFlyout).not.toBeVisible({ timeout: 10000 });

    const policyRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'CSP Reporting' }) });
    await expect(policyRow).toBeVisible();
    await highlightOn(policyRow);
    await page.screenshot({ path: join(IMG_DIR, 'report_step4_policy_created.png') });

    await dismissAllToasts(page);

    // Step 3: Copy the policy as an HTTP header (enforcing).
    const policyGrid = page.getByRole('table');
    await policyGrid.getByRole('button', { name: 'All actions, row' }).click();
    await page.getByRole('button', { name: 'Copy', exact: true }).click();

    const copyModal = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Copy policy' }) });
    await expect(copyModal).toBeVisible({ timeout: 10000 });

    await expect(copyModal.locator('.euiCodeBlock')).toContainText('Content-Security-Policy', { timeout: 10000 });
    await highlightOn(copyModal.locator('.euiCodeBlock'));
    await page.screenshot({ path: join(IMG_DIR, 'report_step5_copy_header.png') });

    // Step 4: Create a "CSP Eval Test" responder via API (Monaco editor cannot be reliably filled via Playwright).
    const evalBody = [
      '<!DOCTYPE html>',
      '<html lang="en">',
      '<head>',
      '  <title>Evaluate CSP</title>',
      '</head>',
      '<body>',
      '<label for="eval-input">Expression to evaluate:</label>',
      '<input id="eval-input" type="text" value="alert(\'xss\')"/>',
      '<button id="eval-test">Eval</button>',
      '<script type="text/javascript" defer>',
      '  (async function main() {',
      "    const evalTestBtn = document.getElementById('eval-test');",
      "    evalTestBtn.addEventListener('click', () => {",
      "      const evalExpression = document.getElementById('eval-input');",
      '      window.eval(evalExpression.value);',
      '    });',
      '  })();',
      '</script>',
      '</body>',
      '</html>',
    ].join('\n');
    const createEvalResponse = await page.request.post('/api/utils/webhooks/responders', {
      data: {
        name: 'CSP Eval Test',
        location: { pathType: '=', path: '/csp-eval-test' },
        method: 'ANY',
        enabled: true,
        settings: {
          requestsToTrack: 10,
          statusCode: 200,
          headers: [
            ['Content-Type', 'text/html; charset=utf-8'],
            [
              'Content-Security-Policy',
              `script-src 'self' 'unsafe-inline' 'report-sample'; report-uri ${reportingUrl}`,
            ],
          ],
          body: evalBody,
        },
      },
    });
    expect(createEvalResponse.ok()).toBeTruthy();

    // Reload to see the responder, open Edit, and screenshot the form.
    await goto(page, '/ws/webhooks__responders');
    const evalRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'CSP Eval Test' }) });
    await expect(evalRow).toBeVisible({ timeout: 15000 });

    await evalRow.getByRole('button', { name: 'Edit' }).click();
    const evalFlyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit responder' }) });
    await expect(evalFlyout).toBeVisible();

    await evalFlyout.getByText('Body', { exact: true }).scrollIntoViewIfNeeded();
    const saveEvalButton = evalFlyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveEvalButton);
    await page.screenshot({ path: join(IMG_DIR, 'report_step6_eval_form.png') });

    await evalFlyout.getByRole('button', { name: 'Close' }).click();
    await expect(evalFlyout).not.toBeVisible({ timeout: 10000 });

    await highlightOn(page.getByRole('table'));
    await page.screenshot({ path: join(IMG_DIR, 'report_step7_eval_created.png') });

    // Step 5: Open the eval test page and try eval() - CSP blocks it and sends a report via report-uri.
    const evalLink = evalRow.getByRole('link');
    const evalUrl = await evalLink.getAttribute('href');
    const evalPage = await page.context().newPage();
    await goto(evalPage, evalUrl!);
    const evalButton = evalPage.getByRole('button', { name: 'Eval' });
    await expect(evalButton).toBeVisible({ timeout: 15000 });
    await highlightOn(evalButton);
    await evalButton.click();
    await evalPage.screenshot({ path: join(IMG_DIR, 'report_step8_eval_blocked.png') });
    await evalPage.close();

    // Step 6: Go back to responders and expand the CSP Reporting responder to view the violation report.
    await fixResponderRequestFields(page);
    await goto(page, '/ws/webhooks__responders');
    await expect(reportingRow).toBeVisible({ timeout: 15000 });

    const showRequestsButton = reportingRow.getByRole('button', { name: 'Show requests' });
    await showRequestsButton.click();

    const requestsGrid = page.getByRole('grid', { name: 'Requests' });
    await expect(requestsGrid).toBeVisible({ timeout: 15000 });

    const bodyCell = requestsGrid.getByRole('gridcell').filter({ hasText: 'bytes' }).first();
    await bodyCell.click();
    await page.keyboard.press('Enter');

    const cellPopover = page.locator('[data-test-subj="euiDataGridExpansionPopover"]');
    await expect(cellPopover).toBeVisible({ timeout: 10000 });
    await highlightOn(cellPopover);
    await page.screenshot({ path: join(IMG_DIR, 'report_step9_violation_report.png') });
  });

  test('share a policy', async ({ page }) => {
    await goto(page, '/ws/web_security__csp__policies');

    // Empty policies list.
    const createButton = page.getByRole('button', { name: 'Create policy' });
    await expect(createButton).toBeVisible({ timeout: 15000 });

    // Show the policy flyout.
    await page.getByRole('button', { name: 'Create policy' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add policy' }) });
    await expect(flyout).toBeVisible();

    // Fill in the form.
    await flyout.getByLabel('Name').fill('Policy to share');

    const defaultSrcDirective = getByRoleAndLabel(flyout, 'combobox', 'Default source (default-src)');
    await defaultSrcDirective.fill("'self'");
    await page.keyboard.press('Enter');
    await defaultSrcDirective.fill('api.secutils.dev');
    await page.keyboard.press('Enter');

    await page.keyboard.press('Escape');

    // Save the policy and verify it's created.
    await flyout.getByRole('button', { name: 'Save' }).click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    await dismissAllToasts(page);

    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Policy to share' }) });
    await expect(row).toBeVisible();

    // Open the actions menu and click "Share".
    const grid = page.getByRole('table');
    await grid.getByRole('button', { name: 'All actions, row' }).click();

    const shareButton = page.getByRole('button', { name: 'Share', exact: true });
    await highlightOn(shareButton);
    await page.screenshot({ path: join(IMG_DIR, 'share_step1_share.png') });

    await shareButton.click();
    const shareModal = page
      .getByRole('dialog')
      .filter({ has: page.getByRole('heading', { name: 'Share "Policy to share" policy' }) });
    await expect(shareModal).toBeVisible({ timeout: 10000 });

    const shareToggle = shareModal.getByRole('switch', { name: 'Share policy' });
    await highlightOn(shareToggle);
    await shareToggle.click();

    const shareLinkCopyButton = shareModal.getByRole('button', { name: 'Copy  link' });
    await highlightOn(shareLinkCopyButton);
    await page.screenshot({ path: join(IMG_DIR, 'share_step2_copy_link.png') });

    // Stop sharing
    await shareToggle.click();

    await page.screenshot({ path: join(IMG_DIR, 'share_step3_unshare.png') });
  });
});
