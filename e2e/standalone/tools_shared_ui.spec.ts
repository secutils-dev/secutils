import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import * as vm from 'node:vm';
import { inflateRawSync } from 'node:zlib';

import { expect, test } from '@playwright/test';

const origin = 'https://tools.secutils.dev';
const root = join(__dirname, '../..');
const source = readFileSync(join(root, 'dev/tools/echo.html'), 'utf8');
const responder = source.match(/<!--\s*@su:responder-script\s*([\s\S]*?)-->/)![1];
let html: string;

test.beforeAll(async () => {
  // The tools pipeline is native ESM; the Playwright suite uses CommonJS.
  html = execFileSync(process.execPath, ['--input-type=module'], {
    cwd: root,
    encoding: 'utf8',
    input: `
      import { readFileSync } from 'node:fs';
      import { minify } from 'html-minifier-terser';
      import { MINIFY_OPTIONS } from './dev/tools/deploy.ts';
      import { inlineSharedAssets } from './dev/tools/shared-assets.ts';
      const source = readFileSync('dev/tools/echo.html', 'utf8').replace(/\\{\\{TOOLS_HOST\\}\\}/g, 'tools.secutils.dev');
      process.stdout.write(await minify(inlineSharedAssets(source), MINIFY_OPTIONS));
    `,
  });
});

test.beforeEach(async ({ page }) => {
  await page.route('**/*', async (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== origin || url.pathname !== '/echo') return route.abort();
    if (!url.searchParams.has('c')) return route.fulfill({ contentType: 'text/html', body: html });
    const response = vm.runInNewContext(responder, {
      context: { query: Object.fromEntries(url.searchParams) },
      Deno: { core: { decode: (bytes: Uint8Array) => new TextDecoder().decode(bytes) } },
    });
    return route.fulfill({ status: response.statusCode, headers: response.headers, body: response.body });
  });
});

test('deployed ECHO is self-contained and keeps its state and served mock contract', async ({ page }) => {
  const errors: string[] = [];
  const sharedRequests: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('request', (request) => {
    if (/\/(shared|echo\/assets)\//.test(request.url())) sharedRequests.push(request.url());
  });
  await page.goto(`${origin}/echo`);
  await page.getByLabel('Status code', { exact: true }).fill('418');
  await page.getByLabel('Body', { exact: true }).fill('Unicode mock: café 🌍\n{"ok":false}');
  await page.getByLabel('Description', { exact: false }).fill('Notes stay in the configurator');
  // Wait for the final edit's encoded state, not an earlier preview value.
  await expect
    .poll(async () => {
      const value = await page.getByLabel('Mock URL', { exact: true }).inputValue();
      if (!value) return null;
      const encoded = new URL(value).searchParams.get('c')!;
      return JSON.parse(inflateRawSync(Buffer.from(encoded, 'base64url').subarray(4)).toString()).d;
    })
    .toBe('Notes stay in the configurator');
  const url = await page.getByLabel('Mock URL', { exact: true }).inputValue();
  expect(url).toBe(`${origin}/echo?c=${new URL(page.url()).hash.slice(1)}`);
  const response = await page.evaluate(async (mockUrl) => {
    const res = await fetch(mockUrl);
    return { status: res.status, body: await res.text(), type: res.headers.get('content-type') };
  }, url);
  expect(response).toEqual({ status: 418, body: 'Unicode mock: café 🌍\n{"ok":false}', type: 'application/json' });
  await page.reload();
  await expect(page.getByLabel('Status code', { exact: true })).toHaveValue('418');
  await expect(page.getByLabel('Body', { exact: true })).toHaveValue(response.body);
  await expect(page.getByLabel('Description', { exact: false })).toHaveValue('Notes stay in the configurator');
  await expect(page.locator('[data-icon="copy"] svg')).toHaveAttribute('viewBox', '0 0 16 16');
  await expect(page.getByRole('link', { name: /View AI agent skill/ })).toHaveAttribute('href', '/echo.md');
  expect(sharedRequests).toEqual([]);
  expect(errors).toEqual([]);
});

for (const theme of ['dark', 'light']) {
  for (const width of [1280, 390, 320]) {
    test(`shared chrome, dynamic headers, and native dialog work in ${theme} at ${width}px`, async ({ page }) => {
      await page.setViewportSize({ width, height: 800 });
      await page.addInitScript((value) => localStorage.setItem('su-tool-theme', value), theme);
      await page.goto(`${origin}/echo`);
      await expect(page.getByLabel('Header name', { exact: true })).toBeVisible();
      const brand = page.getByRole('link', { name: 'ECHO by Secutils.dev (opens in new tab)' });
      await expect(brand.locator('.su-app-name')).toHaveText('ECHO');
      await expect(brand.locator('.su-app-publisher')).toHaveText('Secutils.dev');
      await expect(brand.locator('.su-app-publisher')).toBeVisible();
      await expect(brand.locator('img')).toHaveAttribute('src', /^data:image\/svg\+xml;base64,/);
      const iconBox = (await brand.locator('img').boundingBox())!;
      const nameBox = (await brand.locator('.su-app-name').boundingBox())!;
      const publisherBox = (await brand.locator('.su-app-publisher').boundingBox())!;
      expect(iconBox.width).toBe(32);
      expect(iconBox.height).toBe(32);
      expect(nameBox.x).toBeGreaterThan(iconBox.x + iconBox.width);
      expect(publisherBox.x).toBe(nameBox.x);
      expect(publisherBox.y).toBeGreaterThanOrEqual(nameBox.y + nameBox.height);
      const geometry = await page.evaluate(() => {
        const root = getComputedStyle(document.documentElement);
        const button = getComputedStyle(document.getElementById('open')!);
        const field = getComputedStyle(document.getElementById('status')!);
        return {
          background: root.getPropertyValue('--eui-plain').trim(),
          scheme: root.colorScheme,
          buttonHeight: button.height,
          fieldHeight: field.height,
          radius: button.borderRadius,
          overflow: document.documentElement.scrollWidth > innerWidth,
        };
      });
      expect(geometry).toEqual({
        background: theme === 'dark' ? '#0b1628' : '#ffffff',
        scheme: theme,
        buttonHeight: '32px',
        fieldHeight: '32px',
        radius: '6px',
        overflow: false,
      });
      await page.getByRole('button', { name: 'Add header', exact: true }).click();
      await expect(page.getByLabel('Header name', { exact: true })).toHaveCount(2);
      await expect(page.getByRole('button', { name: 'Remove header' }).last().locator('svg')).toHaveAttribute(
        'aria-hidden',
        'true',
      );
      await page.getByRole('button', { name: 'Remove header' }).last().click();
      await page.getByRole('button', { name: 'Remove header' }).click();
      await expect(page.locator('#empty')).toBeVisible();
      await expect(page.locator('#empty')).toContainText('Content-Type: text/plain');
      // Safari does not focus buttons on a mouse click. Use a keyboard opener
      // so the native dialog has a focused invoker to restore.
      await page.getByRole('button', { name: 'Privacy', exact: true }).focus();
      await page.keyboard.press('Enter');
      await expect(page.getByRole('dialog', { name: 'Privacy', exact: true })).toBeVisible();
      await page.keyboard.press('Escape');
      await expect(page.getByRole('dialog', { name: 'Privacy', exact: true })).not.toBeVisible();
      await expect(page.getByRole('button', { name: 'Privacy', exact: true })).toBeFocused();
      await page.getByRole('button', { name: 'Toggle theme' }).focus();
      await expect(page.getByRole('button', { name: 'Toggle theme' })).toHaveCSS('outline-style', 'solid');
      await page.keyboard.press('Enter');
      await expect(page.locator('html')).toHaveAttribute('data-theme', theme === 'dark' ? 'light' : 'dark');
      await expect(page.locator('body')).toHaveCSS(
        'background-color',
        theme === 'dark' ? 'rgb(246, 249, 252)' : 'rgb(7, 16, 31)',
      );
      await page.getByRole('button', { name: 'Reset', exact: true }).click();
      await expect(page.getByLabel('Header name', { exact: true })).toHaveValue('Content-Type');
    });
  }
}

test('copy feedback keeps the label stable, and validation waits for interaction', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('su-tool-theme', 'dark'));
  await page.goto(`${origin}/echo`);
  const status = page.getByLabel('Status code', { exact: true });
  await expect(status).not.toHaveAttribute('aria-invalid', 'true');
  await status.fill('99');
  await page.getByLabel('Body', { exact: true }).focus();
  await expect(status).toHaveAttribute('aria-invalid', 'true');
  await expect(status).toHaveCSS('border-color', 'rgb(246, 114, 106)');
  await page.evaluate(() => {
    navigator.clipboard.writeText = async () => {
      throw new Error('Denied');
    };
  });
  await page.getByRole('button', { name: 'Copy URL', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('Failed to copy');
  await expect(page.getByRole('button', { name: 'Copy URL', exact: true })).toBeVisible();
});
