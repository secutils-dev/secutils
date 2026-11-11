import { execFileSync } from 'node:child_process';
import { join } from 'node:path';

import { expect, test } from '@playwright/test';

const root = join(__dirname, '../..');
const origin = 'https://tools.secutils.dev';
let html: string;

test.beforeAll(() => {
  html = execFileSync(process.execPath, ['--input-type=module'], {
    cwd: root,
    encoding: 'utf8',
    input: `
      import { readFileSync } from 'node:fs';
      import { minify } from 'html-minifier-terser';
      import { MINIFY_OPTIONS } from './dev/tools/shared/deploy.ts';
      import { assertInlineScriptSafety } from './dev/tools/shared/html.ts';
      const source = readFileSync('dev/tools/md/index.html', 'utf8').replaceAll('{{TOOLS_HOST}}', 'tools.secutils.dev');
      const html = await minify(source, MINIFY_OPTIONS);
      assertInlineScriptSafety(html);
      process.stdout.write(html);
    `,
  });
});

test('minified Markdown Preview closes its inline script and bootstraps without network dependencies', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  // Only startup is under test. CodeMirror must use its normal textarea fallback.
  await page.addInitScript(() => {
    Object.assign(window, { marked: { use() {}, Renderer: class {} } });
  });
  await page.route('**/*', (route) =>
    route.request().url() === `${origin}/md` ? route.fulfill({ contentType: 'text/html', body: html }) : route.abort(),
  );
  await page.goto(`${origin}/md`);
  await expect(page.getByRole('link', { name: /View AI agent skill/ })).toHaveAttribute('href', '/md.md');
  await expect
    .poll(() => page.evaluate(() => typeof (window as { __suOnEditorChange?: unknown }).__suOnEditorChange))
    .toBe('function');
  await expect(page.locator('#editorMount textarea')).toHaveCount(1);
  const theme = await page.locator('html').getAttribute('data-theme');
  await page.getByRole('button', { name: 'Toggle theme' }).click();
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', theme!);
  const lastScript = await page.locator('body > script').last().textContent();
  expect(lastScript).not.toMatch(/<\/script><\/body><\/html>$/);
  expect(errors).toEqual([]);
});
