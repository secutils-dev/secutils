import { readFileSync } from 'fs';
import { join } from 'path';

import { expect, test } from '@playwright/test';

import {
  dismissAllToasts,
  DOCS_IMG_DIR,
  ensureUserAndLogin,
  fixCertificateTemplateValidityDates,
  goto,
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
    const createButton = page.getByRole('button', { name: 'Create certificate template' });
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
    const createButton = page.getByRole('button', { name: 'Create certificate template' });
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

  test('Share a certificate template', async ({ page }) => {
    // Create a template to share.
    await goto(page, '/ws/certificates__certificate_templates');
    const createButton = page.getByRole('button', { name: 'Create certificate template' });
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
