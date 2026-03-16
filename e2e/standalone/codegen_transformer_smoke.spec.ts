/**
 * Structural smoke test for the Playwright script transformer.
 *
 * Spawns `npx playwright codegen` against `about:blank` to capture the
 * current boilerplate wrapper format, then verifies the transformer can
 * strip it. This detects breaking changes to the codegen output format
 * when the Playwright version is upgraded.
 *
 * Run:
 *   cd e2e && npx playwright test --config=playwright.standalone.config.ts
 *
 * Or via Make:
 *   make e2e-standalone-test
 */
import { execSync, spawn } from 'node:child_process';
import { existsSync, readFileSync, unlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { expect, test } from '@playwright/test';

import { transformPlaywrightScript } from '../../components/secutils-webui/src/pages/workspace/utils/web_scraping/playwright_script_transformer';

function isCodegenAvailable(): boolean {
  try {
    execSync('npx playwright codegen about:blank', { stdio: 'ignore', timeout: 5_000 });
    // Exited cleanly (unlikely for codegen, but means it works).
    return true;
  } catch (err: unknown) {
    if (err && typeof err === 'object') {
      const e = err as Record<string, unknown>;
      // Process timed out (ETIMEDOUT) or was killed -- codegen started successfully.
      if (e.killed || e.signal === 'SIGTERM' || e.code === 'ETIMEDOUT') {
        return true;
      }
    }
    return false;
  }
}

function runCodegenWithTimeout(target: string, outFile: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn('npx', ['playwright', 'codegen', `--target=${target}`, '-o', outFile, 'about:blank'], {
      stdio: 'ignore',
    });

    const timer = setTimeout(() => {
      child.kill('SIGTERM');
    }, 10_000);

    child.on('close', () => {
      clearTimeout(timer);
      if (existsSync(outFile)) {
        resolve(readFileSync(outFile, 'utf-8'));
      } else {
        reject(new Error(`Codegen did not produce output file: ${outFile}`));
      }
    });

    child.on('error', (err: Error) => {
      clearTimeout(timer);
      reject(err);
    });
  });
}

test.describe('Playwright codegen format smoke test', () => {
  test.skip(!isCodegenAvailable(), 'Playwright codegen or browsers not available');

  test('transformer handles current --target=javascript boilerplate', async () => {
    const outFile = join(tmpdir(), `codegen-smoke-js-${Date.now()}.js`);
    try {
      const raw = await runCodegenWithTimeout('javascript', outFile);
      expect(raw.length).toBeGreaterThan(0);

      const transformed = transformPlaywrightScript(raw);
      expect(transformed).toContain('export async function execute(page)');
      expect(transformed).not.toMatch(/require\s*\(/);
      expect(transformed).not.toMatch(/import\s+.*from\s+['"]playwright/);
      expect(transformed).not.toMatch(/browser\.launch/);
      expect(transformed).not.toMatch(/browser\.close/);
      expect(transformed).not.toMatch(/context\.close/);
    } finally {
      if (existsSync(outFile)) unlinkSync(outFile);
    }
  });

  test('transformer handles current --target=playwright-test boilerplate', async () => {
    const outFile = join(tmpdir(), `codegen-smoke-test-${Date.now()}.js`);
    try {
      const raw = await runCodegenWithTimeout('playwright-test', outFile);
      expect(raw.length).toBeGreaterThan(0);

      const transformed = transformPlaywrightScript(raw);
      expect(transformed).toContain('export async function execute(page)');
      expect(transformed).not.toMatch(/import\s+.*from\s+['"]@playwright\/test/);
      expect(transformed).not.toMatch(/\btest\s*\(/);
    } finally {
      if (existsSync(outFile)) unlinkSync(outFile);
    }
  });
});
