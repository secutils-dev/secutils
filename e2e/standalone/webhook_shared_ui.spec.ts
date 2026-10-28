import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { deflateRawSync, inflateRawSync } from 'node:zlib';

import { expect, test } from '@playwright/test';

import { webhookBackend } from './_webhook_backend';

const root = join(__dirname, '../..');
const origin = 'https://tools.secutils.dev';
const source = readFileSync(join(root, 'dev/tools/webhook.html'), 'utf8');
const responder = source.match(/<!--\s*@su:responder-script\s*([\s\S]*?)-->/)![1];
let html: string;
let backend: ReturnType<typeof webhookBackend>;
let managementRequests: string[];

test.beforeAll(() => {
  html = execFileSync(process.execPath, ['--input-type=module'], {
    cwd: root,
    encoding: 'utf8',
    input: `
      import { readFileSync } from 'node:fs';
      import { minify } from 'html-minifier-terser';
      import { MINIFY_OPTIONS } from './dev/tools/deploy.ts';
      import { inlineSharedAssets } from './dev/tools/shared-assets.ts';
      const source = readFileSync('dev/tools/webhook.html', 'utf8').replace(/\\{\\{TOOLS_HOST\\}\\}/g, 'tools.secutils.dev');
      process.stdout.write(await minify(inlineSharedAssets(source), MINIFY_OPTIONS));
    `,
  });
});

test.beforeEach(async ({ page }) => {
  backend = webhookBackend(responder);
  managementRequests = [];
  await page.route('**/*', async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    if (url.origin !== origin || !/^\/webhook(?:\/|$)/.test(url.pathname)) return route.abort();
    if (url.searchParams.has('t')) managementRequests.push(request.method());
    const response = await backend.run(
      url,
      request.method(),
      new Uint8Array(request.postDataBuffer() ?? []),
      request.headers(),
    );
    return route.fulfill(
      response
        ? { status: response.statusCode, headers: response.headers, body: response.body }
        : { contentType: 'text/html', body: html },
    );
  });
});

for (const theme of ['dark', 'light']) {
  for (const width of [1280, 390, 320]) {
    test(`WEBHOOKS uses shared chrome in ${theme} at ${width}px`, async ({ page }) => {
      const errors: string[] = [];
      const assetRequests: string[] = [];
      page.on('pageerror', (error) => errors.push(error.message));
      page.on('request', (request) => {
        if (/\/(shared|webhook\/assets)\//.test(request.url())) assetRequests.push(request.url());
      });
      await page.addInitScript((value) => localStorage.setItem('su-tool-theme', value), theme);
      await page.setViewportSize({ width, height: 800 });
      await page.goto(`${origin}/webhook`);
      await expect(page.locator('#fingerprint')).toHaveClass('fp ok');
      const brand = page.getByRole('link', { name: 'WEBHOOKS by Secutils.dev (opens in new tab)' });
      await expect(brand.locator('.su-app-name')).toHaveText('WEBHOOKS');
      await expect(brand.locator('.su-app-publisher')).toHaveText('Secutils.dev');
      await expect(brand.locator('.su-app-publisher')).toBeVisible();
      await expect(brand.locator('img')).toHaveAttribute('src', /^data:image\/svg\+xml;base64,/);
      const iconSource = await brand.locator('img').getAttribute('src');
      const svg = Buffer.from(iconSource!.split(',')[1], 'base64').toString();
      expect(svg).toContain('fill="#fed047"');
      expect(svg).toContain('fill="#642340"');
      expect(svg).toContain('vector_triangle.svg');
      await expect(page.locator('.su-header')).toHaveCSS('height', '48px');
      await expect(page.getByRole('combobox')).toHaveCSS('height', '32px');
      await expect(page.getByRole('button', { name: 'New', exact: true })).toHaveCSS('height', '32px');
      await expect(page.locator('body')).toHaveCSS(
        'background-color',
        theme === 'dark' ? 'rgb(7, 16, 31)' : 'rgb(246, 249, 252)',
      );
      expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)).toBe(false);
      await page.locator('#respCard summary').click();
      await page.getByRole('button', { name: 'Add header', exact: true }).click();
      await expect(page.getByLabel('Header name', { exact: true })).toHaveCount(2);
      await expect(page.getByRole('button', { name: 'Remove header' }).last().locator('svg')).toHaveAttribute(
        'aria-hidden',
        'true',
      );
      await page.getByRole('button', { name: 'Remove header' }).last().click();
      expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)).toBe(false);
      await page.getByRole('button', { name: 'Privacy', exact: true }).focus();
      await page.keyboard.press('Enter');
      await expect(page.getByRole('dialog', { name: 'Privacy', exact: true })).toBeVisible();
      await page.keyboard.press('Escape');
      await expect(page.getByRole('dialog', { name: 'Privacy', exact: true })).toBeHidden();
      await expect(page.getByRole('button', { name: 'Privacy', exact: true })).toBeFocused();
      await page.getByRole('button', { name: 'Toggle theme' }).focus();
      await expect(page.getByRole('button', { name: 'Toggle theme' })).toHaveCSS('outline-style', 'solid');
      await page.keyboard.press('Enter');
      await expect(page.locator('html')).toHaveAttribute('data-theme', theme === 'dark' ? 'light' : 'dark');
      expect(assetRequests).toEqual([]);
      expect(errors).toEqual([]);
    });
  }
}

test('request capture, browser decryption, mock response, and cloning survive the migration', async ({ page }) => {
  await page.goto(`${origin}/webhook`);
  await expect(page.locator('#fingerprint')).toHaveClass('fp ok');
  const webhook = await page.locator('#webhookUrl').inputValue();
  const payload = 'Secret test payload: café 🌍';
  const captured = await page.evaluate(
    async ({ url, payload }) => {
      const response = await fetch(`${url}?source=test`, { method: 'POST', body: payload });
      return { status: response.status, body: await response.json() };
    },
    { url: webhook, payload },
  );
  expect(captured).toEqual({ status: 200, body: { ok: true } });
  expect([...backend.values.entries()].filter(([key]) => key.startsWith('req/'))).toHaveLength(1);
  expect([...backend.values.values()].some((value) => value.includes(payload))).toBe(false);
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  const request = page.locator('#reqList .req');
  await expect(request).toHaveCount(1);
  await request.locator('summary').click();
  await expect(request.locator('pre')).toHaveText(payload);
  await expect(request).toContainText('source=test');
  await page.locator('#respCard summary').click();
  await page.getByLabel('Status code', { exact: true }).fill('418');
  await page.getByLabel('Body', { exact: true }).fill('Teapot response');
  await page.getByRole('button', { name: 'Save response', exact: true }).click();
  await expect(page.locator('#toast')).toHaveText('Response saved');
  const mock = await page.evaluate(async (url) => {
    const response = await fetch(url, { method: 'POST', body: 'Second request' });
    return { status: response.status, body: await response.text() };
  }, webhook);
  expect(mock).toEqual({ status: 418, body: 'Teapot response' });
  await page.getByRole('button', { name: 'Clone', exact: true }).click();
  await expect(page.locator('#webhookUrl')).not.toHaveValue(webhook);
  await expect(page.locator('#fingerprint')).toHaveClass('fp ok');
  await expect(page.getByLabel('Status code', { exact: true })).toHaveValue('418');
  await expect(page.locator('#reqList .req')).toHaveCount(0);
  await page.getByRole('button', { name: 'Reset to default', exact: true }).click();
  await expect(page.locator('#toast')).toHaveText('Response reset to default');
  await expect(page.getByLabel('Status code', { exact: true })).toHaveValue('200');
});

test('rename and destructive dialogs retain native behavior and saved state', async ({ page }) => {
  await page.goto(`${origin}/webhook`);
  await expect(page.locator('#fingerprint')).toHaveClass('fp ok');
  const original = await page.locator('#webhookUrl').inputValue();
  await page.getByRole('button', { name: 'Rename', exact: true }).click();
  const rename = page.getByRole('dialog', { name: 'Rename webhook', exact: true });
  await rename.getByLabel('Label', { exact: true }).fill('Release hook');
  await rename.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(rename).toBeHidden();
  await expect(page.getByRole('combobox')).toContainText('Release hook');
  // Reload only after the asynchronous encoder writes the renamed session.
  await expect
    .poll(() => {
      const encoded = new URL(page.url()).hash.slice(1);
      return JSON.parse(inflateRawSync(Buffer.from(encoded, 'base64url').subarray(4)).toString()).l;
    })
    .toBe('Release hook');
  await page.reload();
  await expect(page.locator('#fingerprint')).toHaveClass('fp ok');
  await expect(page.getByRole('combobox')).toContainText('Release hook');
  await expect(page.locator('#webhookUrl')).toHaveValue(original);
  await page.getByRole('button', { name: 'Clear history', exact: true }).click();
  const clear = page.getByRole('dialog', { name: 'Clear history', exact: true });
  await expect(clear).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(clear).toBeHidden();
  expect(managementRequests.filter((method) => method === 'DELETE')).toEqual([]);
  await page.getByRole('button', { name: 'Clear history', exact: true }).click();
  await clear.getByRole('button', { name: 'Clear history', exact: true }).click();
  await expect(page.locator('#toast')).toHaveText('History cleared');
  await page.getByRole('button', { name: 'Delete webhook', exact: true }).click();
  const deletion = page.getByRole('dialog', { name: 'Delete webhook', exact: true });
  await deletion.getByRole('button', { name: 'Delete webhook', exact: true }).click();
  await expect(page.locator('#webhookUrl')).not.toHaveValue(original);
  await expect(page.locator('#fingerprint')).toHaveClass('fp ok');
});

test('expired sessions show the shared take-over card without re-registering', async ({ page }) => {
  const state = Buffer.from(
    JSON.stringify({
      t: 'expired-demo',
      k: { demo: true },
      p: 'placeholder',
      l: 'Expired hook',
      exp: Math.floor(Date.now() / 1000) - 60,
    }),
  );
  const encoded = Buffer.concat([Buffer.alloc(4), deflateRawSync(state)]);
  encoded.writeUInt32LE(state.length, 0);
  await page.goto(`${origin}/webhook#${encoded.toString('base64url')}`);
  await expect(page.locator('#expiredState')).toBeVisible();
  await expect(page.locator('#requestsCard')).toBeHidden();
  await expect(page.locator('#respCard')).toBeHidden();
  await expect(page.locator('#expiredState [data-icon="clock"] svg')).toBeVisible();
  expect(managementRequests).toEqual([]);
});

test('status validation uses the shared error style after interaction', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('su-tool-theme', 'dark'));
  await page.goto(`${origin}/webhook`);
  await expect(page.locator('#fingerprint')).toHaveClass('fp ok');
  await page.locator('#respCard summary').click();
  const status = page.getByLabel('Status code', { exact: true });
  await expect(status).not.toHaveAttribute('aria-invalid', 'true');
  await status.fill('99');
  await page.getByLabel('Body', { exact: true }).focus();
  await expect(status).toHaveAttribute('aria-invalid', 'true');
  await expect(status).toHaveCSS('border-color', 'rgb(246, 114, 106)');
});

test('the live switch keeps its state and shared status colors', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('su-tool-theme', 'dark'));
  await page.goto(`${origin}/webhook`);
  await expect(page.locator('#fingerprint')).toHaveClass('fp ok');
  const live = page.getByRole('switch', { name: 'Live', exact: true });
  await expect(live).toHaveAttribute('aria-checked', 'false');
  await live.click();
  await expect(live).toHaveAttribute('aria-checked', 'true');
  await expect(live).toHaveCSS('color', 'rgb(36, 194, 146)');
  await live.click();
  await expect(live).toHaveAttribute('aria-checked', 'false');
  await page.mouse.move(0, 0);
  await expect(live).toHaveCSS('color', 'rgb(152, 168, 195)');
});

test('the live indicator pulses without changing dot or button geometry', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.goto(`${origin}/webhook`);
  await expect(page.locator('#fingerprint')).toHaveClass('fp ok');
  const live = page.getByRole('switch', { name: 'Live', exact: true });
  // Exercise the visual state without starting network polling in this animation check.
  await live.evaluate((button) => button.setAttribute('aria-checked', 'true'));
  const frames = await live.locator('.ring').evaluate((dot) => {
    const animation = dot
      .getAnimations()
      .find((item) => item instanceof CSSAnimation && item.animationName === 'su-live-pulse');
    if (!animation?.effect) throw new Error('Live pulse animation is missing');
    animation.pause();
    const duration = Number(animation.effect.getTiming().duration);
    return [0, 0.5, 1].map((progress) => {
      animation.currentTime = duration * progress;
      const style = getComputedStyle(dot);
      const rect = dot.getBoundingClientRect();
      const button = dot.parentElement!.getBoundingClientRect();
      return {
        dot: [rect.x, rect.y, rect.width, rect.height],
        button: [button.x, button.y, button.width, button.height],
        radius: style.borderRadius,
        transform: style.transform,
        opacity: Number(style.opacity),
      };
    });
  });
  for (const frame of frames) {
    expect(frame.dot).toEqual(frames[0].dot);
    expect(frame.button).toEqual(frames[0].button);
    expect(frame.dot.slice(2)).toEqual([8, 8]);
    expect(frame.radius).toBe('50%');
    expect(frame.transform).toBe('none');
  }
  expect(frames[0].opacity).toBeCloseTo(1);
  expect(frames[1].opacity).toBeCloseTo(0.65);
  expect(frames[2].opacity).toBeCloseTo(1);
});

test('reduced motion keeps the live indicator steady', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto(`${origin}/webhook`);
  await expect(page.locator('#fingerprint')).toHaveClass('fp ok');
  const live = page.getByRole('switch', { name: 'Live', exact: true });
  await live.evaluate((button) => button.setAttribute('aria-checked', 'true'));
  const dot = live.locator('.ring');
  await expect(dot).toHaveCSS('animation-name', 'none');
  await expect(dot).toHaveCSS('opacity', '1');
  await expect(dot).toHaveCSS('transform', 'none');
  await expect(dot).toHaveCSS('width', '8px');
  await expect(dot).toHaveCSS('height', '8px');
});
