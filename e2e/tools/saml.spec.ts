import { expect, test } from '@playwright/test';

import { assertSeoBasics, assertSkillMd, getTool } from './_helpers';

const tool = getTool('saml');

// Minimal but real SAML Response (b64-encoded). We don't validate the signature,
// only that the decoder surfaces the AttributeStatement when it's pasted in.
const SAMPLE_SAML_B64 =
  'PHNhbWxwOlJlc3BvbnNlIHhtbG5zOnNhbWxwPSJ1cm46b2FzaXM6bmFtZXM6dGM6U0FNTDoyLjA6cHJvdG9jb2wiPjxzYW1scDpTdGF0dXM+PHNhbWxwOlN0YXR1c0NvZGUgVmFsdWU9InVybjpvYXNpczpuYW1lczp0YzpTQU1MOjIuMDpzdGF0dXM6U3VjY2VzcyIvPjwvc2FtbHA6U3RhdHVzPjxBc3NlcnRpb24geG1sbnM9InVybjpvYXNpczpuYW1lczp0YzpTQU1MOjIuMDphc3NlcnRpb24iIElEPSJfYTEiIElzc3VlSW5zdGFudD0iMjAyNS0wMS0wMVQwMDowMDowMFoiIFZlcnNpb249IjIuMCI+PElzc3Vlcj5leGFtcGxlLm9yZzwvSXNzdWVyPjxTdWJqZWN0PjxOYW1lSUQgRm9ybWF0PSJ1cm46b2FzaXM6bmFtZXM6dGM6U0FNTDoxLjE6bmFtZWlkLWZvcm1hdDplbWFpbEFkZHJlc3MiPmpvaG5AZXhhbXBsZS5vcmc8L05hbWVJRD48L1N1YmplY3Q+PEF0dHJpYnV0ZVN0YXRlbWVudD48QXR0cmlidXRlIE5hbWU9ImVtYWlsIj48QXR0cmlidXRlVmFsdWU+am9obkBleGFtcGxlLm9yZzwvQXR0cmlidXRlVmFsdWU+PC9BdHRyaWJ1dGU+PC9BdHRyaWJ1dGVTdGF0ZW1lbnQ+PC9Bc3NlcnRpb24+PC9zYW1scDpSZXNwb25zZT4=';

test.describe(`${tool.name} (${tool.path})`, () => {
  test('SEO head block matches the AGENTS.md SEO budget', async ({ page }) => {
    const response = await page.goto(tool.path);
    expect(response?.ok()).toBeTruthy();
    await assertSeoBasics(page, tool);
  });

  test('skill .md is reachable with required frontmatter', async ({ request }) => {
    await assertSkillMd(request, tool);
  });

  test('decodes a base64-encoded SAML payload', async ({ page }) => {
    await page.goto(tool.path);
    const input = page.locator('#encoded-saml');
    await expect(input).toBeVisible();
    await input.fill(SAMPLE_SAML_B64);

    await expect(page.locator('#decoded-xml-container')).toContainText('AttributeStatement');
    await page.locator('#attributes-tab').click();
    await expect(page.locator('#attributes-table-body')).toContainText('john@example.org');
  });
});
