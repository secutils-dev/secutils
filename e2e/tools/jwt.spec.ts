import { expect, test } from '@playwright/test';

import { assertSeoBasics, assertSkillMd, getTool } from './_helpers';

const tool = getTool('jwt');

// A known-good HS256 token for `{ "sub": "1234567890", "name": "John Doe", "iat": 1516239022 }`
// signed with `your-256-bit-secret`. We round-trip this through the URL fragment
// to confirm both the deep-link decoder and the verification flow work end-to-end.
const SAMPLE_JWT =
  'eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c';
const SAMPLE_SECRET = 'your-256-bit-secret';

test.describe(`${tool.name} (${tool.path})`, () => {
  test('SEO head block matches the AGENTS.md SEO budget', async ({ page }) => {
    const response = await page.goto(tool.path);
    expect(response?.ok()).toBeTruthy();
    await assertSeoBasics(page, tool);
  });

  test('skill .md is reachable with required frontmatter', async ({ request }) => {
    await assertSkillMd(request, tool);
  });

  test('decodes a JWT pasted into the encoded input', async ({ page }) => {
    await page.goto(tool.path);
    const encoded = page.locator('#encoded-output');
    await expect(encoded).toBeVisible();
    await encoded.fill(SAMPLE_JWT);
    await expect(page.locator('#header-output')).toContainText('HS256');
    await expect(page.locator('#payload-output')).toContainText('John Doe');
  });

  test('verifies the signature once the secret is provided', async ({ page }) => {
    await page.goto(tool.path);
    await page.locator('#encoded-output').fill(SAMPLE_JWT);
    await page.locator('#secret-input').fill(SAMPLE_SECRET);
    await expect(page.locator('#signature-status')).toContainText(/Signature Verified/i);
  });

  test('Share button produces a shareable URL with state in the fragment', async ({ page }) => {
    await page.goto(tool.path);
    await page.locator('#encoded-output').fill(SAMPLE_JWT);
    await page.locator('#secret-input').fill(SAMPLE_SECRET);

    // Capture the URL the page wants to share by stubbing the clipboard.
    let copied = '';
    await page.exposeFunction('__captureCopy', (value: string) => {
      copied = value;
    });
    await page.evaluate(() => {
      navigator.clipboard.writeText = async (value: string) => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        await (window as any).__captureCopy(value);
      };
    });

    await page.locator('#share-button').click();
    await expect.poll(() => copied).toMatch(/#\S+$/);
    expect(copied.startsWith('https://') || copied.startsWith('http://')).toBeTruthy();

    const fragment = new URL(copied).hash;
    await page.goto(`${tool.path}${fragment}`);
    await expect(page.locator('#encoded-output')).toHaveValue(SAMPLE_JWT);
    await expect(page.locator('#secret-input')).toHaveValue(SAMPLE_SECRET);
  });
});
