import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

import type { Locator, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

const MARS_BASE_URL = process.env.MARS_BASE_URL ?? 'https://mars.secutils.dev';
test.use({ baseURL: MARS_BASE_URL });
const fixture = (name: string) => resolve(__dirname, `fixtures/3mf/${name}.3mf`);
const dialog = (page: Page) => page.getByRole('dialog', { name: /^Export / });
const html = (page: Page) => page.frameLocator('#exportHtmlPreview iframe');
const pdf = (page: Page) => page.frameLocator('#exportPdfPreview iframe');
const TRIANGLE =
  'solid triangle\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid triangle';

async function boot(page: Page) {
  await page.goto('/');
  await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
}
async function setText(page: Page, text: string) {
  await page.evaluate(
    (text) => (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(text),
    text,
  );
}
async function openExport(page: Page) {
  await page.getByRole('button', { name: 'Export document', exact: true }).click();
  await expect(dialog(page).getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled({
    timeout: 20000,
  });
}
async function download(page: Page, format: 'HTML' | 'Markdown') {
  const pending = page.waitForEvent('download');
  await dialog(page)
    .getByRole('button', { name: `Download ${format}`, exact: true })
    .click();
  return readFile(await (await pending).path(), 'utf8');
}
async function colors(model: Locator) {
  return model.locator('canvas').evaluate((element) => {
    const canvas = element as HTMLCanvasElement;
    const gl = canvas.getContext('webgl')!;
    const pixels = new Uint8Array(canvas.width * canvas.height * 4);
    gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
    const counts = { red: 0, green: 0, blue: 0, mixed: 0 };
    for (let i = 0; i < pixels.length; i += 4) {
      const [r, g, b] = pixels.subarray(i, i + 3);
      if (r > 80 && r > g * 1.8 && r > b * 1.8) counts.red++;
      if (g > 80 && g > r * 1.8 && g > b * 1.8) counts.green++;
      if (b > 80 && b > r * 1.8 && b > g * 1.8) counts.blue++;
      if (r > 30 && g > 30 && b < 100 && Math.abs(r - g) < 30) counts.mixed++;
    }
    return counts;
  });
}
function binaryStl(count = 1) {
  const binary = Buffer.alloc(84 + count * 50);
  binary.write('solid binary header');
  binary.writeUInt32LE(count, 80);
  for (let i = 0; i < count; i++) {
    const x = (i % 100) / 100,
      y = Math.floor(i / 100) / 120;
    const values = [0, 0, 1, x, y, 0, x + 0.01, y, 0, x, y + 0.01, 0];
    for (const [index, value] of values.entries()) binary.writeFloatLE(value, 84 + i * 50 + index * 4);
  }
  return binary;
}

test.describe('model parity', () => {
  test.setTimeout(90000);

  test('binary and ASCII STL reopen as editable source and truncated imports retain the previous document', async ({
    page,
  }) => {
    await boot(page);
    const binary = binaryStl();
    await page.locator('#fileInput').setInputFiles({ name: 'binary.stl', mimeType: 'model/stl', buffer: binary });
    await expect(page.locator('#previewArticle figure[data-kind="stl"] polygon')).toHaveCount(1);
    await expect(page.locator('#saveState')).toHaveText('Saved');
    await openExport(page);
    expect(await download(page, 'Markdown')).toContain('```stl\nsolid model');
    await expect(html(page).locator('figure[data-kind="stl"] polygon')).toHaveCount(1);
    await dialog(page).getByRole('button', { name: 'Close export', exact: true }).click();
    await page
      .locator('#fileInput')
      .setInputFiles({ name: 'ascii.STL', mimeType: 'model/stl', buffer: Buffer.from(TRIANGLE) });
    await expect(page.locator('#docTitle')).toHaveText('ascii');
    await page
      .locator('#fileInput')
      .setInputFiles({ name: 'truncated.stl', mimeType: 'model/stl', buffer: binary.subarray(0, 120) });
    await expect(page.locator('#toast')).toContainText('Invalid or truncated STL file');
    await expect(page.locator('#docTitle')).toHaveText('ascii');
    await expect(page.locator('#previewArticle figure[data-kind="stl"] polygon')).toHaveCount(1);
  });

  test('large STL keeps a bounded DOM, saved views, interactive offline HTML, and print snapshots', async ({
    page,
    browserName,
  }) => {
    test.skip(browserName !== 'chromium', 'Large-mesh canvas checks require the Chromium software GPU.');
    await boot(page);
    await page
      .locator('#fileInput')
      .setInputFiles({ name: 'large.stl', mimeType: 'model/stl', buffer: binaryStl(12000) });
    const model = page.locator('#previewArticle figure[data-kind="stl"]');
    await expect(model.locator('canvas')).toBeVisible({ timeout: 15000 });
    expect(await model.locator('*').count()).toBeLessThan(100);
    await model.getByRole('button', { name: 'Rotate right', exact: true }).click();
    const view = await model.getAttribute('data-current-view');
    await model.getByRole('button', { name: 'Save current view to source', exact: true }).click();
    await expect(model).toHaveAttribute('data-view', view!);
    await openExport(page);
    await expect(html(page).locator('figure')).toHaveAttribute('data-view', view!);
    await expect(html(page).locator('figure canvas')).toBeVisible();
    const snapshot = await html(page).locator('.su-stl-snapshot').getAttribute('src');
    await dialog(page).getByRole('tab', { name: 'PDF', exact: true }).click();
    await expect(dialog(page).getByRole('button', { name: 'Print / Save PDF', exact: true })).toBeEnabled({
      timeout: 60000,
    });
    await expect(pdf(page).locator('.su-stl-snapshot')).toHaveAttribute('src', snapshot!);
    await dialog(page).getByRole('tab', { name: 'HTML', exact: true }).click();
    await expect(dialog(page).getByRole('button', { name: 'Download HTML', exact: true })).toBeEnabled();
    const output = await download(page, 'HTML');
    const path = test.info().outputPath('large-model.html');
    await writeFile(path, output);
    await page.context().setOffline(true);
    await page.goto(pathToFileURL(path).href);
    await expect(page.locator('figure canvas')).toBeVisible();
    await page.getByRole('button', { name: 'Surface angle', exact: true }).click();
    await page.emulateMedia({ media: 'print' });
    await expect(page.locator('.su-stl-snapshot')).toBeVisible();
    await expect(page.locator('figure canvas')).toBeHidden();
  });

  for (const name of ['vertex-colors', 'zip64-colors']) {
    test(`${name} retains pixel colors through actual offline HTML`, async ({ page, browserName }) => {
      test.skip(browserName !== 'chromium', 'Color-pixel assertions require the Chromium software GPU.');
      await boot(page);
      await page.locator('#fileInput').setInputFiles(fixture(name));
      const model = page.locator('#previewArticle figure[data-kind="su-mesh"]');
      await expect(model.locator('canvas')).toBeVisible({ timeout: 15000 });
      await expect.poll(async () => (await colors(model)).red).toBeGreaterThan(100);
      const pixels = await colors(model);
      expect(pixels.red).toBeGreaterThan(100);
      expect(pixels.blue).toBeGreaterThan(100);
      if (name === 'vertex-colors') {
        expect(pixels.green).toBeGreaterThan(100);
        expect(pixels.mixed).toBeGreaterThan(100);
      }
      await openExport(page);
      const path = test.info().outputPath(`${name}.html`);
      await writeFile(path, await download(page, 'HTML'));
      await page.context().setOffline(true);
      await page.goto(pathToFileURL(path).href);
      const offline = page.locator('figure[data-kind="su-mesh"]');
      await expect(offline.locator('canvas')).toBeVisible();
      await expect.poll(async () => (await colors(offline)).red).toBeGreaterThan(100);
      const exported = await colors(offline);
      expect(exported.red).toBeGreaterThan(100);
      expect(exported.blue).toBeGreaterThan(100);
      if (name === 'vertex-colors') expect(exported.mixed).toBeGreaterThan(100);
    });
  }

  test('uncolored, component, and slicer models import while broken archives leave the current model intact', async ({
    page,
    browserName,
  }) => {
    test.skip(browserName !== 'chromium', 'Canvas checks require the Chromium software GPU.');
    await boot(page);
    for (const name of ['box', 'multiple-cylinders', 'prusaslicer']) {
      await page.locator('#fileInput').setInputFiles(fixture(name));
      await expect(page.locator('#docTitle')).toHaveText(name);
      await expect(page.locator('#previewArticle figure[data-kind="su-mesh"] canvas')).toBeVisible();
    }
    await expect(page.locator('#toast')).toContainText('Slicer-specific paint metadata is unsupported');
    await page
      .locator('#fileInput')
      .setInputFiles({ name: 'broken.3mf', mimeType: 'model/3mf', buffer: Buffer.from('bad') });
    await expect(page.locator('#toast')).toContainText('Failed to open file');
    await expect(page.locator('#docTitle')).toHaveText('prusaslicer');
  });

  test('invalid geometry and view metadata retain the source and later readable blocks', async ({ page }) => {
    await boot(page);
    const text =
      '```geojson\n{"type":"Point","coordinates":["bad",0]}\n```\n\n```topojson\n{"type":"Topology","objects":{}}\n```\n\n```stl\nsolid empty\nendsolid\n```\n\n## Still readable\n\nBody.';
    await setText(page, text);
    await expect(page.locator('#previewArticle .su-mermaid-error')).toHaveCount(3);
    await expect(page.locator('#previewArticle h2')).toContainText('Still readable');
    await openExport(page);
    expect(await download(page, 'Markdown')).toBe(text);
    await dialog(page).getByRole('button', { name: 'Close export', exact: true }).click();
    for (const [index, view] of [
      '{"zoom":0}',
      '{"yaw":"bad"}',
      '{"mode":"unknown"}',
      '{"other":1}',
      'broken',
    ].entries()) {
      await setText(page, `\`\`\`stl view=${view}\n${TRIANGLE}\n\`\`\`\n\n## Later ${index}`);
      await expect(page.locator('#previewArticle h2')).toContainText(`Later ${index}`);
      await expect(page.locator('#previewArticle .su-mermaid-error')).toHaveCount(1);
      await expect(page.locator('#previewArticle .su-mermaid-error')).toBeVisible();
    }
  });

  test('model-worker cancellation, replacement, and editing never apply held results', async ({ page }) => {
    await page.addInitScript(() => {
      const OriginalWorker = window.Worker;
      const held = new Map<Worker, { message: unknown; transfer?: Transferable[] | StructuredSerializeOptions }>();
      window.Worker = class extends OriginalWorker {
        postMessage(message: unknown, transfer?: Transferable[] | StructuredSerializeOptions) {
          held.set(this, { message, transfer });
        }
        terminate() {
          held.delete(this);
          super.terminate();
        }
      };
      (window as unknown as { releaseModelWork(): number }).releaseModelWork = () => {
        for (const [worker, { message, transfer }] of held)
          OriginalWorker.prototype.postMessage.call(worker, message, transfer);
        const count = held.size;
        held.clear();
        return count;
      };
    });
    await boot(page);
    await setText(page, '# Keep this');
    await expect(page.locator('#previewArticle h1')).toContainText('Keep this');
    await page.locator('#fileInput').setInputFiles(fixture('face-colors'));
    await expect(page.getByRole('button', { name: 'Cancel import', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Export document', exact: true })).toBeDisabled();
    await page.getByRole('button', { name: 'Cancel import', exact: true }).click();
    await expect(page.locator('#previewArticle h1')).toContainText('Keep this');
    await page.locator('#fileInput').setInputFiles(fixture('face-colors'));
    await expect(page.getByRole('button', { name: 'Cancel import', exact: true })).toBeVisible();
    await page
      .locator('#fileInput')
      .setInputFiles({ name: 'New.md', mimeType: 'text/markdown', buffer: Buffer.from('# New file') });
    await expect(page.locator('#previewArticle h1')).toContainText('New file');
    await page.locator('#fileInput').setInputFiles(fixture('face-colors'));
    await expect(page.getByRole('button', { name: 'Cancel import', exact: true })).toBeVisible();
    await setText(page, '# Edited during import');
    await expect(page.getByRole('button', { name: 'Cancel import', exact: true })).toBeHidden();
    expect(await page.evaluate(() => (window as unknown as { releaseModelWork(): number }).releaseModelWork())).toBe(0);
    await expect(page.locator('#previewArticle h1')).toContainText('Edited during import');
    await expect(page.locator('figure[data-kind="su-mesh"]')).toHaveCount(0);
  });
});
