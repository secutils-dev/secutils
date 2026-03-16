// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';

import { transformPlaywrightScript } from './playwright_script_transformer';

const RETURN_HINT = [
  '',
  '  // TODO: Add a return statement with the content you want to track.',
  '  // Examples:',
  '  //   return await page.title();',
  '  //   return await page.content();',
  "  //   return await page.locator('.result').textContent();",
].join('\n');

describe('transformPlaywrightScript', () => {
  it('transforms test-framework format (default codegen output)', () => {
    const input = `import { test, expect } from '@playwright/test';

test('test', async ({ page }) => {
  await page.goto('https://example.com/');
  await page.getByRole('link', { name: 'Get started' }).click();
  await expect(page.getByRole('heading', { name: 'Installation' })).toBeVisible();
});`;

    const result = transformPlaywrightScript(input);
    expect(result).toBe(
      [
        'export async function execute(page) {',
        "  await page.goto('https://example.com/');",
        "  await page.getByRole('link', { name: 'Get started' }).click();",
        RETURN_HINT,
        '}',
      ].join('\n'),
    );
  });

  it('transforms library format (CommonJS require)', () => {
    const input = `const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.goto('https://example.com/');
  await page.getByRole('button', { name: 'Sign up' }).click();
  await page.getByPlaceholder('Email').fill('test@example.com');
  await context.close();
  await browser.close();
})();`;

    const result = transformPlaywrightScript(input);
    expect(result).toBe(
      [
        'export async function execute(page) {',
        "  await page.goto('https://example.com/');",
        "  await page.getByRole('button', { name: 'Sign up' }).click();",
        "  await page.getByPlaceholder('Email').fill('test@example.com');",
        RETURN_HINT,
        '}',
      ].join('\n'),
    );
  });

  it('transforms ESM import format', () => {
    const input = `import { chromium } from 'playwright';

const browser = await chromium.launch();
const context = await browser.newContext();
const page = await context.newPage();
await page.goto('https://example.com/');
await page.locator('#search').fill('hello');
await context.close();
await browser.close();`;

    const result = transformPlaywrightScript(input);
    expect(result).toBe(
      [
        'export async function execute(page) {',
        "  await page.goto('https://example.com/');",
        "  await page.locator('#search').fill('hello');",
        RETURN_HINT,
        '}',
      ].join('\n'),
    );
  });

  it('transforms playwright-core import', () => {
    const input = `import { chromium } from 'playwright-core';

const browser = await chromium.launch();
const context = await browser.newContext();
const page = await context.newPage();
await page.goto('https://example.com/');
await context.close();
await browser.close();`;

    const result = transformPlaywrightScript(input);
    expect(result).toBe(
      ['export async function execute(page) {', "  await page.goto('https://example.com/');", RETURN_HINT, '}'].join(
        '\n',
      ),
    );
  });

  it('strips expect() assertions', () => {
    const input = `import { test, expect } from '@playwright/test';

test('test', async ({ page }) => {
  await page.goto('https://example.com/');
  await expect(page).toHaveTitle(/Example/);
  await page.getByRole('link', { name: 'More info' }).click();
  await expect(page.getByRole('heading', { name: 'Info' })).toBeVisible();
});`;

    const result = transformPlaywrightScript(input);
    expect(result).toBe(
      [
        'export async function execute(page) {',
        "  await page.goto('https://example.com/');",
        "  await page.getByRole('link', { name: 'More info' }).click();",
        RETURN_HINT,
        '}',
      ].join('\n'),
    );
  });

  it('handles page.* calls only (no wrapper at all)', () => {
    const input = `await page.goto('https://example.com/');
await page.locator('.title').click();`;

    const result = transformPlaywrightScript(input);
    expect(result).toBe(
      [
        'export async function execute(page) {',
        "  await page.goto('https://example.com/');",
        "  await page.locator('.title').click();",
        RETURN_HINT,
        '}',
      ].join('\n'),
    );
  });

  it('preserves blank lines between actions', () => {
    const input = `import { test } from '@playwright/test';

test('test', async ({ page }) => {
  await page.goto('https://example.com/');

  await page.getByRole('link', { name: 'About' }).click();
});`;

    const result = transformPlaywrightScript(input);
    expect(result).toBe(
      [
        'export async function execute(page) {',
        "  await page.goto('https://example.com/');",
        '',
        "  await page.getByRole('link', { name: 'About' }).click();",
        RETURN_HINT,
        '}',
      ].join('\n'),
    );
  });

  it('handles firefox.launch and webkit.launch', () => {
    const input = `const { firefox } = require('playwright');
(async () => {
  const browser = await firefox.launch();
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.goto('https://example.com/');
  await context.close();
  await browser.close();
})();`;

    const result = transformPlaywrightScript(input);
    expect(result).toBe(
      ['export async function execute(page) {', "  await page.goto('https://example.com/');", RETURN_HINT, '}'].join(
        '\n',
      ),
    );
  });

  it('handles launch with options', () => {
    const input = `const { chromium } = require('playwright');
(async () => {
  const browser = await chromium.launch({ headless: false });
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.goto('https://example.com/');
  await context.close();
  await browser.close();
})();`;

    const result = transformPlaywrightScript(input);
    expect(result).toBe(
      ['export async function execute(page) {', "  await page.goto('https://example.com/');", RETURN_HINT, '}'].join(
        '\n',
      ),
    );
  });

  it('preserves variable declarations used with page', () => {
    const input = `import { test } from '@playwright/test';

test('test', async ({ page }) => {
  await page.goto('https://example.com/');
  const title = await page.title();
  console.log(title);
});`;

    const result = transformPlaywrightScript(input);
    expect(result).toBe(
      [
        'export async function execute(page) {',
        "  await page.goto('https://example.com/');",
        '  const title = await page.title();',
        '  console.log(title);',
        RETURN_HINT,
        '}',
      ].join('\n'),
    );
  });

  it('handles type imports gracefully', () => {
    const input = `import type { Page } from '@playwright/test';
import { test } from '@playwright/test';

test('test', async ({ page }) => {
  await page.goto('https://example.com/');
});`;

    const result = transformPlaywrightScript(input);
    expect(result).toBe(
      ['export async function execute(page) {', "  await page.goto('https://example.com/');", RETURN_HINT, '}'].join(
        '\n',
      ),
    );
  });

  it('transforms exact user input with headless:false option', () => {
    const input = `const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({
    headless: false
  });
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.goto('https://secutils.dev');
  await page.getByText('Welcome').click();

  // ---------------------
  await context.close();
  await browser.close();
})();`;

    const result = transformPlaywrightScript(input);
    // Should NOT contain any browser setup/teardown
    expect(result).not.toMatch(/require\s*\(/);
    expect(result).not.toMatch(/browser\.launch/);
    expect(result).not.toMatch(/browser\.close/);
    expect(result).not.toMatch(/context\.close/);
    // Should contain the wrapped execute function with page actions
    expect(result).toContain('export async function execute(page)');
    expect(result).toContain("await page.goto('https://secutils.dev');");
  });
});
