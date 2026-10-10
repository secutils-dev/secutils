import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, test } from '@playwright/test';

test('bundle reports render imported names and commit messages as text', async ({ page }) => {
  const html = readFileSync(join(__dirname, '../../components/secutils-webui/scripts/bundle-report.html'), 'utf8');
  await page.route('https://report.test/', (route) => route.fulfill({ contentType: 'text/html', body: html }));
  // Charts do not participate in table rendering; keep this test offline.
  await page.route('https://cdn.jsdelivr.net/**', (route) =>
    route.fulfill({ contentType: 'text/javascript', body: 'window.Chart = class { destroy() {} };' }),
  );
  await page.goto('https://report.test/');
  const malicious = '"><img src=x onerror="document.body.dataset.injected=1">';
  const entry = {
    timestamp: '2026-09-25T12:00:00Z',
    commitSha: '<img src=x>',
    commitMessage: malicious,
    bundles: { [malicious]: { raw: 100, brotli: 50 } },
    categories: { [malicious]: { raw: 100, brotli: 50 }, total: { raw: 100, brotli: 50 } },
  };
  await page.locator('#fileInput').setInputFiles({
    name: 'history.jsonl',
    mimeType: 'application/json',
    buffer: Buffer.from([entry, entry].map((value) => JSON.stringify(value)).join('\n')),
  });
  await expect(page.locator('#historyTable tbody tr')).toHaveCount(2);
  await expect(page.locator('#historyTable tbody tr').first().locator('td').nth(1)).toHaveAttribute('title', malicious);
  await expect(page.locator('#comparisonTable')).toContainText(malicious);
  await expect(page.locator('table img')).toHaveCount(0);
  await expect(page.locator('body')).not.toHaveAttribute('data-injected');
});
