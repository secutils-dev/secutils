import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { deflateRawSync } from 'node:zlib';

import { expect } from '@playwright/test';

import { test } from './_mars_test';

const baseURL = process.env.MARS_BASE_URL ?? 'https://mars.secutils.dev';
const markdownPreview = readFileSync(resolve(__dirname, '../../dev/tools/md/index.html'), 'utf8').replaceAll(
  '{{TOOLS_HOST}}',
  'tools.secutils.dev',
);
const body = '# Review markers\n\n```md\n<!-- su-comments\n[]\n-->\n```\n\nAfter the example.\n';
const text = `${body}\n<!-- su-comments\n[{"id":"actual-review","quote":"After the example","comments":[{"id":"actual-comment","author":"Reviewer","body":"Actual private review"}]}]\n-->\n`;
test.use({ baseURL });

test('review markers: Markdown Preview retains examples and parses only the final review block', async ({ page }) => {
  const plain = Buffer.from(text),
    bytes = Buffer.alloc(4);
  bytes.writeUInt32LE(plain.length);
  const encoded = Buffer.concat([bytes, deflateRawSync(plain)]).toString('base64url');
  const target = new URL('/markdown-preview-review-check', baseURL).href;
  await page.route(target, (route) => route.fulfill({ status: 200, contentType: 'text/html', body: markdownPreview }));
  await page.goto(`${target}#md=${encoded}`);
  await expect(page.locator('#previewArticle')).toContainText('After the example.');
  await expect(page.locator('#previewArticle pre')).toContainText('<!-- su-comments');
  await expect(page.locator('#commentsRail')).toContainText('Actual private review');
});

test('review markers: MARS retains examples in the reader without exporting private review metadata', async ({
  page,
}) => {
  await page.goto(baseURL);
  await expect(page.locator('#emptyState')).toBeVisible();
  await page
    .locator('#fileInput')
    .setInputFiles({ name: 'Review markers.md', mimeType: 'text/markdown', buffer: Buffer.from(text) });
  await expect(page.locator('#previewArticle')).toContainText('After the example.');
  await expect(page.locator('#previewArticle pre')).toContainText('<!-- su-comments');
  await expect(page.locator('#commentsRail')).toContainText('Actual private review');
  await page.getByRole('button', { name: 'Export document', exact: true }).click();
  await expect(
    page.getByRole('dialog', { name: /^Export / }).getByRole('button', { name: 'Download HTML', exact: true }),
  ).toBeEnabled({ timeout: 20000 });
  const capture = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Download HTML', exact: true }).click();
  const file = await capture,
    html = readFileSync((await file.path())!, 'utf8');
  expect(html).toContain('After the example.');
  expect(html).not.toContain('Actual private review');
  expect(html).toContain('su-comments');
});
