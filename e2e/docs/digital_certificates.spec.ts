import { readFileSync } from 'fs';
import { join } from 'path';

import { expect, test } from '@playwright/test';

import {
  dismissAllToasts,
  DOCS_IMG_DIR,
  ensureUserAndLogin,
  fixCertificateTemplateValidityDates,
  goto,
  highlightOff,
  highlightOn,
} from './helpers';

const PRIVATE_KEYS_IMG_DIR = join(DOCS_IMG_DIR, 'digital_certificates/private_keys');
const CERT_TEMPLATES_IMG_DIR = join(DOCS_IMG_DIR, 'digital_certificates/certificate_templates');

test.describe('Private keys guide screenshots', () => {
  test.beforeEach(async ({ page, request }) => {
    await ensureUserAndLogin(request, page);
  });

  test('Generate an RSA private key', async ({ page }) => {
    // Step 1: Navigate to private keys and show the empty state.
    await goto(page, '/ws/certificates__private_keys');
    const createButton = page.getByRole('button', { name: 'Create private key' });
    await expect(createButton).toBeVisible({ timeout: 15000 });
    await highlightOn(createButton);
    await page.screenshot({ path: join(PRIVATE_KEYS_IMG_DIR, 'rsa_step1_empty.png') });

    // Step 2: Open the flyout and fill in the General section.
    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add private key' }) });
    await expect(flyout).toBeVisible();

    await flyout.getByLabel('Name', { exact: true }).fill('RSA');
    await flyout.getByLabel('Key algorithm').selectOption('RSA');
    await flyout.getByLabel('Key size').selectOption('2048');

    await page.screenshot({ path: join(PRIVATE_KEYS_IMG_DIR, 'rsa_step2_general.png') });

    // Step 3: Scroll down to Security and set encryption to None.
    await flyout.getByLabel('Encryption').selectOption('None');

    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveButton);
    await page.screenshot({ path: join(PRIVATE_KEYS_IMG_DIR, 'rsa_step3_security.png') });

    // Step 4: Save and verify the key appears in the grid.
    await saveButton.click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    const keyRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'RSA' }) });
    await expect(keyRow).toBeVisible();
    await highlightOn(keyRow);
    await page.screenshot({ path: join(PRIVATE_KEYS_IMG_DIR, 'rsa_step4_created.png') });

    await dismissAllToasts(page);

    // Step 5: Open the Export modal with PEM format and no encryption.
    const grid = page.getByRole('table');
    await grid.getByRole('button', { name: 'All actions, row' }).click();
    await page.getByRole('button', { name: 'Export', exact: true }).click();

    const exportModal = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Export' }) });
    await expect(exportModal).toBeVisible({ timeout: 10000 });

    await exportModal.getByLabel('Format').selectOption('PEM');

    const exportButton = exportModal.getByRole('button', { name: 'Export' });
    await highlightOn(exportButton);
    await page.screenshot({ path: join(PRIVATE_KEYS_IMG_DIR, 'rsa_step5_export.png') });
  });

  test('Generate an ECDSA elliptic curve private key', async ({ page }) => {
    // Step 1: Navigate to private keys and show the empty state.
    await goto(page, '/ws/certificates__private_keys');
    const createButton = page.getByRole('button', { name: 'Create private key' });
    await expect(createButton).toBeVisible({ timeout: 15000 });
    await highlightOn(createButton);
    await page.screenshot({ path: join(PRIVATE_KEYS_IMG_DIR, 'ecdsa_step1_empty.png') });

    // Step 2: Open the flyout and fill in the General section.
    await createButton.click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add private key' }) });
    await expect(flyout).toBeVisible();

    await flyout.getByLabel('Name', { exact: true }).fill('ECC');
    await flyout.getByLabel('Key algorithm').selectOption('ECDSA');
    await flyout.getByLabel('Curve name').selectOption('secp384r1');

    await page.screenshot({ path: join(PRIVATE_KEYS_IMG_DIR, 'ecdsa_step2_general.png') });

    // Step 3: Scroll down to Security and fill in passphrase.
    await flyout.getByLabel('Passphrase', { exact: true }).fill('pass');
    await flyout.getByLabel('Repeat passphrase').fill('pass');

    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveButton);
    await page.screenshot({ path: join(PRIVATE_KEYS_IMG_DIR, 'ecdsa_step3_security.png') });

    // Step 4: Save and verify the key appears in the grid.
    await saveButton.click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    const keyRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'ECC' }) });
    await expect(keyRow).toBeVisible();
    await highlightOn(keyRow);
    await page.screenshot({ path: join(PRIVATE_KEYS_IMG_DIR, 'ecdsa_step4_created.png') });

    await dismissAllToasts(page);

    // Step 5: Open the Export modal with PKCS#8 format and passphrase.
    const grid = page.getByRole('table');
    await grid.getByRole('button', { name: 'All actions, row' }).click();
    await page.getByRole('button', { name: 'Export', exact: true }).click();

    const exportModal = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Export' }) });
    await expect(exportModal).toBeVisible({ timeout: 10000 });

    await exportModal.getByLabel('Format').selectOption('PKCS#8');
    await exportModal.getByLabel('Current passphrase').fill('pass');
    await exportModal.getByLabel('Export passphrase', { exact: true }).fill('pass-export');
    await exportModal.getByLabel('Repeat export passphrase').fill('pass-export');

    const exportButton = exportModal.getByRole('button', { name: 'Export' });
    await highlightOn(exportButton);
    await page.screenshot({ path: join(PRIVATE_KEYS_IMG_DIR, 'ecdsa_step5_export.png') });
  });
});

test.describe('Certificate templates guide screenshots', () => {
  test.beforeEach(async ({ page, request }) => {
    await ensureUserAndLogin(request, page);
    await fixCertificateTemplateValidityDates(page);
  });

  test('Generate a key pair for a HTTPS server', async ({ page }) => {
    // Step 1: Navigate to certificate templates and show the empty state.
    await goto(page, '/ws/certificates__certificate_templates');
    const createButton = page.getByRole('button', { name: 'Create template' });
    await expect(createButton).toBeVisible({ timeout: 15000 });
    await highlightOn(createButton);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'https_step1_empty.png') });

    // Step 2: Open the flyout and fill in the General section.
    await createButton.click();
    const flyout = page
      .getByRole('dialog')
      .filter({ has: page.getByRole('heading', { name: 'Add certificate template' }) });
    await expect(flyout).toBeVisible();

    await flyout.getByLabel('Name', { exact: true }).fill('https-server');
    await flyout.getByLabel('Key algorithm').selectOption('RSA');
    await flyout.getByLabel('Key size').selectOption('2048');
    await flyout.getByLabel('Signature algorithm').selectOption('SHA-256');

    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'https_step2_general.png') });

    // Step 3: Scroll down to Extensions and fill in certificate type, key usage, extended key usage.
    await flyout.getByLabel('Certificate type').selectOption('End Entity');

    const keyUsageCombo = flyout.getByRole('combobox', { name: 'Key usage', exact: true });
    await keyUsageCombo.click();
    await page.getByRole('option', { name: 'Key encipherment' }).click();
    await page.getByRole('option', { name: 'Digital signature' }).click();
    await page.keyboard.press('Escape');

    const extKeyUsageCombo = flyout.getByRole('combobox', { name: 'Extended key usage' });
    await extKeyUsageCombo.click();
    await page.getByRole('option', { name: 'TLS Web server authentication' }).click();
    await page.keyboard.press('Escape');

    await extKeyUsageCombo.scrollIntoViewIfNeeded();
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'https_step3_extensions.png') });

    // Step 4: Scroll down to Distinguished Name and fill in common name.
    const commonNameField = flyout.getByLabel('Common name (CN)');
    await commonNameField.fill('localhost');
    await commonNameField.scrollIntoViewIfNeeded();
    await highlightOn(saveButton);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'https_step4_dn.png') });

    // Step 5: Save the template and verify it appears in the grid.
    await saveButton.click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    const templateRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'https-server' }) });
    await expect(templateRow).toBeVisible();
    await highlightOn(templateRow);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'https_step5_created.png') });

    await dismissAllToasts(page);

    // Step 6: Open the Generate modal via the actions menu.
    const grid = page.getByRole('table');
    await grid.getByRole('button', { name: 'All actions, row' }).click();
    await page.getByRole('button', { name: 'Generate', exact: true }).click();

    const generateModal = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Generate' }) });
    await expect(generateModal).toBeVisible({ timeout: 10000 });

    await generateModal.getByLabel('Format').selectOption('PKCS#12');
    await generateModal.getByLabel('Passphrase (optional)').fill('pass');

    const generateButton = generateModal.getByRole('button', { name: 'Generate' });
    await highlightOn(generateButton);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'https_step6_generate.png') });
  });

  test('Export a private key as a JSON Web Key', async ({ page }) => {
    // Step 1: Navigate to certificate templates and show the empty state.
    await goto(page, '/ws/certificates__certificate_templates');
    const createButton = page.getByRole('button', { name: 'Create template' });
    await expect(createButton).toBeVisible({ timeout: 15000 });
    await highlightOn(createButton);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'jwk_step1_empty.png') });

    // Step 2: Open the flyout and fill in the General section (ECDSA).
    await createButton.click();
    const flyout = page
      .getByRole('dialog')
      .filter({ has: page.getByRole('heading', { name: 'Add certificate template' }) });
    await expect(flyout).toBeVisible();

    await flyout.getByLabel('Name', { exact: true }).fill('jwk');
    await flyout.getByLabel('Key algorithm').selectOption('ECDSA');
    await flyout.getByLabel('Curve name').selectOption('secp384r1');
    await flyout.getByLabel('Signature algorithm').selectOption('SHA-256');

    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveButton);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'jwk_step2_general.png') });

    // Step 3: Save the template and verify it appears in the grid.
    await saveButton.click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    const templateRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'jwk' }) });
    await expect(templateRow).toBeVisible();
    await highlightOn(templateRow);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'jwk_step3_created.png') });

    await dismissAllToasts(page);

    // Step 4: Open the Generate modal, choose PKCS#8 format, and generate.
    const grid = page.getByRole('table');
    await grid.getByRole('button', { name: 'All actions, row' }).click();
    await page.getByRole('button', { name: 'Generate', exact: true }).click();

    const generateModal = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Generate' }) });
    await expect(generateModal).toBeVisible({ timeout: 10000 });

    await generateModal.getByLabel('Format').selectOption('PKCS#8 (private key only)');

    const generateButton = generateModal.getByRole('button', { name: 'Generate' });
    await highlightOn(generateButton);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'jwk_step4_generate.png') });

    // Actually generate and capture the downloaded file.
    const downloadPromise = page.waitForEvent('download');
    await generateButton.click();
    const download = await downloadPromise;
    const p8Path = await download.path();

    await generateModal.waitFor({ state: 'hidden', timeout: 10000 });

    // Step 5: Create the "Subtle Crypto" responder with an HTML page.
    await goto(page, '/ws/webhooks__responders');
    const createResponderButton = page.getByRole('button', { name: 'Create responder' });
    await expect(createResponderButton).toBeVisible({ timeout: 15000 });
    await createResponderButton.click();

    const responderFlyout = page
      .getByRole('dialog')
      .filter({ has: page.getByRole('heading', { name: 'Add responder' }) });
    await expect(responderFlyout).toBeVisible();

    await responderFlyout.getByLabel('Name').fill('Subtle Crypto');
    await responderFlyout.getByLabel('Path', { exact: true }).fill('/subtle-crypto');

    const responderBody = [
      '<!DOCTYPE html>',
      '<html lang="en">',
      '<head>',
      '  <title>Subtle Crypto</title>',
      '  <style>',
      '    .center { text-align: center }',
      '    pre {',
      '      outline: 1px solid #ccc;',
      '      padding: 5px;',
      '      margin: 1em auto;',
      '      width: 30%;',
      '      overflow: hidden;',
      '      text-overflow: ellipsis;',
      '    }',
      '  </style>',
      '  <script type="text/javascript">',
      '    document.addEventListener("DOMContentLoaded", async function main() {',
      '      document.getElementById("p8_upload").addEventListener("change", (e) => {',
      '        if (e.target.files.length === 0) {',
      '          return;',
      '        }',
      '',
      '        const reader = new FileReader();',
      '        reader.onload = async () => {',
      '          const cryptoKey = await window.crypto.subtle.importKey(',
      '              "pkcs8",',
      '              new Uint8Array(reader.result),',
      '              { name: "ECDSA", namedCurve: "P-384" },',
      '              true,',
      '              ["sign"]',
      '          )',
      '',
      '          document.getElementById("jwk").textContent = JSON.stringify(',
      "              await window.crypto.subtle.exportKey('jwk', cryptoKey),",
      '              null,',
      '              2',
      '          );',
      '        };',
      '        reader.readAsArrayBuffer(e.target.files[0]);',
      '      });',
      '    });',
      '  </script>',
      '</head>',
      '<body>',
      '<h1 class="center">PKCS#8 ➡ JSON Web Key (JWK)</h1>',
      '<div class="center">',
      '  <label for="p8_upload">Choose PKCS#8 key (*.p8)</label>',
      '  <input',
      '      type="file"',
      '      id="p8_upload"',
      '      name="p8_upload"',
      '      accept=".p8" />',
      '  <br />',
      '</div>',
      '<pre id="jwk">No PKCS#8 key is loaded yet...</pre>',
      '</body>',
      '</html>',
    ].join('\n');
    const bodyTextarea = responderFlyout.getByLabel('Body');
    await bodyTextarea.fill(responderBody);
    await bodyTextarea.evaluate((el) => (el.scrollTop = 0));

    const saveResponderButton = responderFlyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveResponderButton);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'jwk_step5_responder_form.png') });

    await saveResponderButton.click();
    await expect(responderFlyout).not.toBeVisible({ timeout: 10000 });

    const responderRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Subtle Crypto' }) });
    await expect(responderRow).toBeVisible();
    await highlightOn(responderRow);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'jwk_step6_responder_created.png') });

    await dismissAllToasts(page);

    // Step 7: Open the responder URL, upload the .p8 file, and view the JWK.
    const responderLink = responderRow.getByRole('link');
    const responderUrl = await responderLink.getAttribute('href');
    const cryptoPage = await page.context().newPage();
    await goto(cryptoPage, responderUrl!);
    await expect(cryptoPage.getByText('No PKCS#8 key is loaded yet...')).toBeVisible({ timeout: 15000 });

    await cryptoPage
      .locator('#p8_upload')
      .setInputFiles({ name: 'jwk.p8', mimeType: 'application/octet-stream', buffer: readFileSync(p8Path!) });
    const jwkOutput = cryptoPage.locator('#jwk');
    await expect(jwkOutput).toContainText('"kty"', { timeout: 10000 });

    // Replace random key material with fixed values so the screenshot is stable.
    await jwkOutput.evaluate((el) => {
      const jwk = JSON.parse(el.textContent!);
      jwk.d = 'MxVt1dX7QjQ4tn6Ktv1Xk1Hlsc2bgnlOoQW5NnXXsoQf5DqVyJg8nR0Tai4WZGZ';
      jwk.x = 'Eln2G96tC0MUh1ld97yKwpLycBl1ps1e2uP1KqlYaz81-eT3ziPB60SOi38xnxwl';
      jwk.y = '6Po8O_JXKxMnMpGr9rQrUO-JtegBxrfIb80nhTWTb5q0V1gh9wMfKhCbg5sHNaeR';
      el.textContent = JSON.stringify(jwk, null, 2);
    });

    await highlightOn(jwkOutput);
    await cryptoPage.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'jwk_step7_result.png') });
    await cryptoPage.close();
  });

  test('Import a certificate template from string', async ({ page }) => {
    // Real self-signed test certificate (RSA 2048, SHA-256, C=US/ST=California/L=San Francisco/O=Test Org/CN=test.example.com).
    const TEST_PEM = [
      '-----BEGIN CERTIFICATE-----',
      'MIIDsTCCApmgAwIBAgIUYWNwS/Zjq9Dg3k7p1mMOgHN2HPwwDQYJKoZIhvcNAQEL',
      'BQAwaDELMAkGA1UEBhMCVVMxEzARBgNVBAgMCkNhbGlmb3JuaWExFjAUBgNVBAcM',
      'DVNhbiBGcmFuY2lzY28xETAPBgNVBAoMCFRlc3QgT3JnMRkwFwYDVQQDDBB0ZXN0',
      'LmV4YW1wbGUuY29tMB4XDTI2MDIyMjE2MDQyMloXDTI3MDIyMjE2MDQyMlowaDEL',
      'MAkGA1UEBhMCVVMxEzARBgNVBAgMCkNhbGlmb3JuaWExFjAUBgNVBAcMDVNhbiBG',
      'cmFuY2lzY28xETAPBgNVBAoMCFRlc3QgT3JnMRkwFwYDVQQDDBB0ZXN0LmV4YW1w',
      'bGUuY29tMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEArRRWC6JnpD14',
      'nqLaGC/GDbavICOLXJOvnsUmmSQneFyGKF/21oz/+ywnznM6BkmjXQJQH7lSfjf6',
      '2nyavZvN21v0uZ1JwCUl3gqEvqoBPwlo57ZC8lrEm/OfGs9R+AMBZHr3AelmoV1r',
      'giwFbSVhth9Thquby2RPF/jbgs2m/oSPSVRooOCkUfdCbp1DAC17+lyyhrByczMw',
      'TCfZZi/bi6Bl9mUyIOImfxw4VDUIjG2z+3htoRMlt7DGmAcf0nHOtl6Y/PgNKGOL',
      'lAuiDp31cRGU7u2+ptrHH2nSrQbWkcDO7QClAFFsUyMWudVoSWp2LB5faBDtLr/K',
      'Buu6H+hM9QIDAQABo1MwUTAdBgNVHQ4EFgQUfuLq1fvV3xoyMudVt1WXuqbKvaAw',
      'HwYDVR0jBBgwFoAUfuLq1fvV3xoyMudVt1WXuqbKvaAwDwYDVR0TAQH/BAUwAwEB',
      '/zANBgkqhkiG9w0BAQsFAAOCAQEAQNGez0mH+lSa2R43Ex+20R+OECUnYu9CuCCK',
      'tfX1rVUCejYbRKXr/w2UsQ2jQ5vzyOUOtlg9gEccnI7lqrXzi+tXYwQtsF0RSBvQ',
      'HDhxTr7N2ZPch3E6Pu1VjK7GaKM6J0iLal76AhFZI5lUPxftRP1wvb4xFeU0/HCR',
      'Lj1tTefuCCXM7dOrSUFau7I56ythgbppFW6052AVdXhypPrIqWaiKwnXBO+Y7znQ',
      'fPWakaZEY44H0JWR7v6g9qk9RtCTDsxEr9qDH40PPQTT5dR6Y2nUd4nqqSXnoOTf',
      'rL6NaXtNHWpD0yoc9+z0o1uBEI19++PrtMnl0j3fgtVyNIl5UQ==',
      '-----END CERTIFICATE-----',
    ].join('\n');

    // Step 1: Navigate to certificate templates and highlight Import button.
    await goto(page, '/ws/certificates__certificate_templates');
    const importButton = page.getByRole('button', { name: 'Import template' });
    await expect(importButton).toBeVisible({ timeout: 15000 });
    await highlightOn(importButton);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'import_string_step1_empty.png') });

    // Step 2: Open the modal and paste PEM content.
    await highlightOff(importButton);
    await importButton.click();
    const modal = page.getByRole('dialog').filter({ has: page.getByText('Import certificate template') });
    await expect(modal).toBeVisible({ timeout: 10000 });

    const textarea = modal.getByRole('textbox');
    await textarea.fill(TEST_PEM);

    const parseButton = modal.getByRole('button', { name: 'Parse certificates' });
    await highlightOn(parseButton);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'import_string_step2_pem.png') });

    // Step 3: Parse and preview.
    await parseButton.click();

    await expect(modal.getByText('1 certificate found')).toBeVisible({ timeout: 10000 });

    const accordion = modal.getByRole('button', { name: /test\.example\.com/ });
    await accordion.click();

    const importActionButton = modal.getByRole('button', { name: /Import/ }).last();
    await highlightOn(importActionButton);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'import_string_step3_preview.png') });

    // Step 4: Import and show the template in the grid.
    await importActionButton.click();

    await expect(modal).not.toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);

    const templateRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'test.example.com' }) });
    await expect(templateRow).toBeVisible({ timeout: 10000 });
    await highlightOn(templateRow);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'import_string_step4_imported.png') });
  });

  test('Import a certificate template from URL', async ({ page }) => {
    // Reuse the same test certificate but serve it via a mocked API response.
    const TEST_PEM = [
      '-----BEGIN CERTIFICATE-----',
      'MIIDsTCCApmgAwIBAgIUYWNwS/Zjq9Dg3k7p1mMOgHN2HPwwDQYJKoZIhvcNAQEL',
      'BQAwaDELMAkGA1UEBhMCVVMxEzARBgNVBAgMCkNhbGlmb3JuaWExFjAUBgNVBAcM',
      'DVNhbiBGcmFuY2lzY28xETAPBgNVBAoMCFRlc3QgT3JnMRkwFwYDVQQDDBB0ZXN0',
      'LmV4YW1wbGUuY29tMB4XDTI2MDIyMjE2MDQyMloXDTI3MDIyMjE2MDQyMlowaDEL',
      'MAkGA1UEBhMCVVMxEzARBgNVBAgMCkNhbGlmb3JuaWExFjAUBgNVBAcMDVNhbiBG',
      'cmFuY2lzY28xETAPBgNVBAoMCFRlc3QgT3JnMRkwFwYDVQQDDBB0ZXN0LmV4YW1w',
      'bGUuY29tMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEArRRWC6JnpD14',
      'nqLaGC/GDbavICOLXJOvnsUmmSQneFyGKF/21oz/+ywnznM6BkmjXQJQH7lSfjf6',
      '2nyavZvN21v0uZ1JwCUl3gqEvqoBPwlo57ZC8lrEm/OfGs9R+AMBZHr3AelmoV1r',
      'giwFbSVhth9Thquby2RPF/jbgs2m/oSPSVRooOCkUfdCbp1DAC17+lyyhrByczMw',
      'TCfZZi/bi6Bl9mUyIOImfxw4VDUIjG2z+3htoRMlt7DGmAcf0nHOtl6Y/PgNKGOL',
      'lAuiDp31cRGU7u2+ptrHH2nSrQbWkcDO7QClAFFsUyMWudVoSWp2LB5faBDtLr/K',
      'Buu6H+hM9QIDAQABo1MwUTAdBgNVHQ4EFgQUfuLq1fvV3xoyMudVt1WXuqbKvaAw',
      'HwYDVR0jBBgwFoAUfuLq1fvV3xoyMudVt1WXuqbKvaAwDwYDVR0TAQH/BAUwAwEB',
      '/zANBgkqhkiG9w0BAQsFAAOCAQEAQNGez0mH+lSa2R43Ex+20R+OECUnYu9CuCCK',
      'tfX1rVUCejYbRKXr/w2UsQ2jQ5vzyOUOtlg9gEccnI7lqrXzi+tXYwQtsF0RSBvQ',
      'HDhxTr7N2ZPch3E6Pu1VjK7GaKM6J0iLal76AhFZI5lUPxftRP1wvb4xFeU0/HCR',
      'Lj1tTefuCCXM7dOrSUFau7I56ythgbppFW6052AVdXhypPrIqWaiKwnXBO+Y7znQ',
      'fPWakaZEY44H0JWR7v6g9qk9RtCTDsxEr9qDH40PPQTT5dR6Y2nUd4nqqSXnoOTf',
      'rL6NaXtNHWpD0yoc9+z0o1uBEI19++PrtMnl0j3fgtVyNIl5UQ==',
      '-----END CERTIFICATE-----',
    ].join('\n');

    // Intercept the peer_certificates API call to return our fixed PEM.
    await page.route('**/api/utils/certificates/templates/peer_certificates', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([TEST_PEM]),
      });
    });

    // Step 1: Navigate to certificate templates and highlight Import button.
    await goto(page, '/ws/certificates__certificate_templates');
    const importButton = page.getByRole('button', { name: 'Import template' });
    await expect(importButton).toBeVisible({ timeout: 15000 });
    await highlightOn(importButton);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'import_url_step1_empty.png') });

    // Step 2: Open the modal, switch to the URL tab, enter a URL and click Fetch.
    await highlightOff(importButton);
    await importButton.click();
    const modal = page.getByRole('dialog').filter({ has: page.getByText('Import certificate template') });
    await expect(modal).toBeVisible({ timeout: 10000 });

    await modal.getByRole('tab', { name: 'URL' }).click();
    const urlInput = modal.getByRole('textbox');
    await urlInput.fill('https://test.example.com');

    const fetchButton = modal.getByRole('button', { name: 'Fetch certificates' });
    await highlightOn(fetchButton);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'import_url_step2_url.png') });

    // Step 3: Fetch and preview.
    await fetchButton.click();

    await expect(modal.getByText('1 certificate found')).toBeVisible({ timeout: 10000 });

    const accordion = modal.getByRole('button', { name: /test\.example\.com/ });
    await accordion.click();

    const importActionButton = modal.getByRole('button', { name: /Import/ }).last();
    await highlightOn(importActionButton);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'import_url_step3_preview.png') });

    // Step 4: Import and show the template in the grid.
    await importActionButton.click();

    await expect(modal).not.toBeVisible({ timeout: 15000 });
    await dismissAllToasts(page);

    const templateRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'test.example.com' }) });
    await expect(templateRow).toBeVisible({ timeout: 10000 });
    await highlightOn(templateRow);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'import_url_step4_imported.png') });
  });

  test('Share a certificate template', async ({ page }) => {
    // Create a template to share.
    await goto(page, '/ws/certificates__certificate_templates');
    const createButton = page.getByRole('button', { name: 'Create template' });
    await expect(createButton).toBeVisible({ timeout: 15000 });
    await createButton.click();

    const flyout = page
      .getByRole('dialog')
      .filter({ has: page.getByRole('heading', { name: 'Add certificate template' }) });
    await expect(flyout).toBeVisible();
    await flyout.getByLabel('Name', { exact: true }).fill('Template to share');
    await flyout.getByRole('button', { name: 'Save' }).click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    await dismissAllToasts(page);

    const row = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Template to share' }) });
    await expect(row).toBeVisible();

    // Step 1: Open the actions menu and click "Share".
    const grid = page.getByRole('table');
    await grid.getByRole('button', { name: 'All actions, row' }).click();

    const shareButton = page.getByRole('button', { name: 'Share', exact: true });
    await highlightOn(shareButton);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'share_step1_share.png') });

    // Step 2: Toggle sharing on and show the Copy link button.
    await shareButton.click();
    const shareModal = page
      .getByRole('dialog')
      .filter({ has: page.getByRole('heading', { name: 'Share "Template to share" template' }) });
    await expect(shareModal).toBeVisible({ timeout: 10000 });

    const shareToggle = shareModal.getByRole('switch', { name: 'Share template' });
    await highlightOn(shareToggle);
    await shareToggle.click();

    const copyLinkButton = shareModal.getByRole('button', { name: 'Copy  link' });
    await expect(copyLinkButton).toBeVisible({ timeout: 10000 });
    await highlightOn(copyLinkButton);
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'share_step2_copy_link.png') });

    // Step 3: Stop sharing.
    await shareToggle.click();
    await page.screenshot({ path: join(CERT_TEMPLATES_IMG_DIR, 'share_step3_unshare.png') });
  });
});
