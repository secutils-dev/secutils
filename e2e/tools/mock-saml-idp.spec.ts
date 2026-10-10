import { deflateRawSync } from 'node:zlib';

import { expect, test } from '@playwright/test';

import { assertSeoBasics, assertSkillMd, getTool } from './_helpers';

const tool = getTool('mock-saml-idp');

// The Mock SAML IdP is intentionally NOT promoted on the home page (it's only
// useful for Elastic Stack SSO testing), but it still has to ship the same SEO
// head block, the same `<noscript>` paragraph, and a reachable skill .md so
// agents that *do* know about it can drive it.
test.describe(`${tool.name} (${tool.path})`, () => {
  test('SEO head block matches the AGENTS.md SEO budget (promote=false)', async ({ page }) => {
    const response = await page.goto(tool.path);
    expect(response?.ok()).toBeTruthy();
    await assertSeoBasics(page, tool);
  });

  test('skill .md is reachable with required frontmatter (promote=false lives in HTML meta)', async ({ request }) => {
    // The skill .md itself is a plain Claude Code SKILL.md (no `promote`
    // field in frontmatter). Promotion status is asserted on the HTML page
    // via `<meta name="su-tool-promote">` in `assertSeoBasics` above.
    await assertSkillMd(request, tool);
  });

  test('the configurator form is rendered and the user can fill the basic fields', async ({ page }) => {
    await page.goto(tool.path);
    await expect(page.locator('#samlForm')).toBeVisible();
    await page.locator('#username').fill('john.doe');
    await page.locator('#email').fill('john@example.org');
    await page.locator('#fullname').fill('John Doe');
    await expect(page.locator('#submitBtn')).toBeEnabled();
  });

  test('keeps request IDs and user attributes as text in logs and signed XML', async ({ page }) => {
    const requestId = '<img src=x onerror="document.body.dataset.injected=1">';
    const escapedId = requestId.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('"', '&quot;');
    const request = `<samlp:AuthnRequest xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol" ID="${escapedId}"/>`;
    const encoded = deflateRawSync(request).toString('base64');
    await page.goto(`${tool.path}?SAMLRequest=${encodeURIComponent(encoded)}`);
    await page.locator('#advancedModeBtn').click();
    const name = 'Alice & <admin> "Doe"';
    await page.locator('#username').fill(name);
    await page.locator('#fullname').fill(name);
    await page.locator('#email').fill('alice@example.org');
    // Capture the generated response without posting it to an external SP.
    await page.evaluate(() => {
      HTMLFormElement.prototype.submit = function () {
        this.dataset.submitted = 'true';
      };
    });
    await page.locator('#submitBtn').click();
    await expect(page.locator('#statusBox')).toContainText('Signature verified successfully!');
    await expect(page.locator('#statusBox')).toContainText(requestId);
    await expect(page.locator('#statusBox img')).toHaveCount(0);
    await expect(page.locator('#realPostForm')).toHaveAttribute('data-submitted', 'true');
    const response = await page.locator('#hiddenSAMLResponse').inputValue();
    const values = await page.evaluate((base64) => {
      const xml = new TextDecoder().decode(Uint8Array.from(atob(base64), (c) => c.charCodeAt(0)));
      const doc = new DOMParser().parseFromString(xml, 'application/xml');
      return {
        parseErrors: doc.querySelectorAll('parsererror').length,
        requestId: doc.documentElement.getAttribute('InResponseTo'),
        attributes: [...doc.getElementsByTagNameNS('*', 'AttributeValue')].map((node) => node.textContent),
      };
    }, response);
    expect(values.parseErrors).toBe(0);
    expect(values.requestId).toBe(requestId);
    expect(values.attributes.filter((value) => value === name)).toHaveLength(2);
  });
});
