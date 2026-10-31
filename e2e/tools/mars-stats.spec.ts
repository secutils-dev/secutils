import { expect, test } from '@playwright/test';

import { assertSeoBasics, assertSkillMd, getTool } from './_helpers';

const tool = getTool('mars-stats');

test('the deployed observatory is unlisted, locked, and exposes its companion skill', async ({ page, request }) => {
  await page.goto(tool.path);
  await assertSeoBasics(page, tool);
  await expect(page.locator('meta[name="robots"]')).toHaveAttribute('content', 'noindex, nofollow');
  await expect(page.getByRole('heading', { name: 'Your private observatory' })).toBeVisible();
  await expect(page.locator('.side-info, .side-footer, .su-more-tools')).toHaveCount(0);
  await assertSkillMd(request, tool);
  for (const action of ['stats', 'history']) {
    const response = await request.get(`${tool.path}?action=${action}`);
    expect(response.status()).toBe(401);
    expect(response.headers()['cache-control']).toBe('no-store');
  }
  expect((await request.post(`${tool.path}?action=sample`)).status()).toBe(401);
  const html = await (await request.get(tool.path)).text();
  expect(html).not.toContain('@su:responder-script');
  expect(html).not.toContain('su_ak_');
  expect(html).not.toContain('{{MARS_STATS_VIEW_TOKEN_HASH}}');
  expect(html).not.toContain('./shared/');
  expect(await (await request.get('/llms.txt')).text()).not.toContain('/mars-stats');
});

test('the owner can read real statistics and history', async ({ page, request }) => {
  const token = process.env.SECUTILS_MARS_STATS_VIEW_TOKEN;
  test.skip(!token, 'Set SECUTILS_MARS_STATS_VIEW_TOKEN to verify authenticated telemetry.');
  const response = await request.get(`${tool.path}?action=stats`, { headers: { Authorization: `Bearer ${token}` } });
  expect(response.ok()).toBe(true);
  const current = await response.json();
  expect(current.intervalSec).toBe(1800);
  expect(current.storage.entries).toBeGreaterThanOrEqual(0);
  expect(current.storage.maxTotalBytes).toBeGreaterThan(0);
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(`${tool.path}#key=${token}`);
  await expect(page.locator('#entryCount')).toHaveText(current.storage.entries.toLocaleString('en-US'));
  await expect(page.locator('#health')).toHaveText('Connected');
  await expect(page.locator('#coverage')).toContainText('samples');
  expect(new URL(page.url()).hash).toBe('');
  expect(errors).toEqual([]);
});
