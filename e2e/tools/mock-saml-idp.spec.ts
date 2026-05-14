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
});
