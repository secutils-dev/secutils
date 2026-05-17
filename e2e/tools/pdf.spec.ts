import { deflateRawSync } from 'node:zlib';

import { expect, test } from '@playwright/test';

import { assertSeoBasics, assertSkillMd, getTool } from './_helpers';

const tool = getTool('pdf');

/**
 * Build a shareable PDF Extractor URL fragment from a pre-extracted state
 * payload. Mirrors the production wire format encoded in
 * `dev/tools/pdf-extractor.html#encodeState`:
 *
 *   [ 4 bytes uncompressed-length (LE u32) | N bytes raw DEFLATE of JSON ]
 *   -> base64url (`+` -> `-`, `/` -> `_`, strip `=`)
 *
 * The payload itself is the JSON envelope `{ v: 1, f, s, t?, j? }` defined
 * in the tool's `buildShareUrl()` / hydrate-from-hash blocks.
 */
function buildPdfFragment(state: object): string {
  const json = JSON.stringify(state);
  const utf8 = Buffer.from(json, 'utf8');
  const deflated = deflateRawSync(utf8);
  const out = Buffer.alloc(4 + deflated.length);
  out.writeUInt32LE(utf8.length, 0);
  deflated.copy(out, 4);
  return out.toString('base64').replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

test.describe(`${tool.name} (${tool.path})`, () => {
  test('SEO head block matches the AGENTS.md SEO budget', async ({ page }) => {
    const response = await page.goto(tool.path);
    expect(response?.ok()).toBeTruthy();
    await assertSeoBasics(page, tool);
  });

  test('skill .md is reachable with required frontmatter', async ({ request }) => {
    await assertSkillMd(request, tool);
  });

  test('initial UI shows the dropzone and a disabled Parse button', async ({ page }) => {
    await page.goto(tool.path);

    // Dropzone is the primary affordance; Parse is gated on a loaded file.
    await expect(page.locator('#dropzone')).toBeVisible();
    await expect(page.locator('#parseBtn')).toBeDisabled();

    // Share / Copy / Download are gated on a result, so they all start
    // disabled and the empty-state hint sits in the right pane.
    await expect(page.locator('#shareBtn')).toBeDisabled();
    await expect(page.locator('#copyBtn')).toBeDisabled();
    await expect(page.locator('#downloadBtn')).toBeDisabled();
    await expect(page.locator('#resultEmpty')).toBeVisible();
  });

  test('inlined liteparse bundle placeholder is present and non-empty', async ({ page }) => {
    // Smoke-check that the production deploy pipeline (deploy.ts inlining
    // via `data-su-bundle="liteparse"`) actually injected something into
    // the placeholder. We only assert it's non-trivial in size and parses
    // as text/plain (so the browser will not execute it eagerly) -- the
    // tool itself promotes it to an executable module on demand.
    await page.goto(tool.path);
    const bundle = page.locator('script[data-su-bundle="liteparse"]');
    await expect(bundle).toHaveAttribute('type', 'text/plain');
    const length = await bundle.evaluate((el) => el.textContent?.length ?? 0);
    expect(length, 'liteparse bundle must be inlined by deploy.ts').toBeGreaterThan(10_000);
  });

  test('inlined liteparse bundle exports getPdfLinks (Markdown tab link pass)', async ({ page }) => {
    // The Markdown tab calls `getPdfLinks(bytes)` to extract hyperlink
    // annotations before running the heuristic engine. Verify the deploy
    // pipeline shipped a bundle that actually exposes that symbol --
    // a stale bundle (built before the export was added) would silently
    // downgrade the Markdown tab to a link-free render in production.
    await page.goto(tool.path);
    const hasExport = await page.evaluate(async () => {
      const el = document.getElementById('su-bundle-liteparse');
      const raw = el?.textContent?.trim() ?? '';
      if (!raw) return false;
      const encoding = el?.getAttribute('data-su-bundle-encoding');
      let src: string;
      if (!encoding) {
        src = raw;
      } else if (encoding === 'gzip-base64') {
        const bin = atob(raw);
        const bytes = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
        const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip'));
        src = await new Response(stream).text();
      } else {
        return false;
      }
      return src.includes('getPdfLinks');
    });
    expect(hasExport, 'liteparse bundle must export getPdfLinks').toBe(true);
  });

  test('hydrates the Text tab from a shared URL fragment', async ({ page }) => {
    const text = 'Hello from a shared PDF link.\nLine 2.';
    const fragment = buildPdfFragment({ v: 1, f: 'text', s: 'shared-doc', t: text });

    await page.goto(`${tool.path}#${fragment}`);

    // Empty state is replaced by the text panel populated with the
    // round-tripped payload. The active tab is `text` (the share URL
    // explicitly carries `f: 'text'`).
    await expect(page.locator('#resultEmpty')).toBeHidden();
    await expect(page.locator('#resultText')).toBeVisible();
    await expect(page.locator('#resultText')).toHaveValue(text);
    await expect(page.locator('#tabText')).toHaveAttribute('aria-selected', 'true');

    // Export buttons unlock once the result is present (the payload arrived
    // from a URL fragment, so it is by definition Share-sized).
    await expect(page.locator('#shareBtn')).toBeEnabled();
    await expect(page.locator('#copyBtn')).toBeEnabled();
    await expect(page.locator('#downloadBtn')).toBeEnabled();
  });

  test('hydrates the JSON tab from a shared URL fragment with bounding boxes', async ({ page }) => {
    const json = {
      pages: [
        {
          page: 1,
          text: 'Hello world',
          boundingBoxes: [{ text: 'Hello world', x: 72, y: 720, width: 80, height: 14 }],
        },
      ],
    };
    const fragment = buildPdfFragment({ v: 1, f: 'json', s: 'shared-doc', j: json });

    await page.goto(`${tool.path}#${fragment}`);

    // Active tab flips to JSON because the share URL carried `f: 'json'`.
    await expect(page.locator('#tabJson')).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('#resultJson')).toBeVisible();

    const rendered = await page.locator('#resultJsonPre').textContent();
    expect(rendered, 'rendered JSON must contain the round-tripped page text').toContain('Hello world');
    expect(rendered, 'rendered JSON must preserve bounding-box geometry').toContain('"x": 72');
  });

  test('hydrates the Markdown tab from a shared v2 URL fragment', async ({ page }) => {
    // v2 share URL carries the rendered Markdown verbatim in `m`. The page
    // should land directly on the Markdown tab with the textarea populated
    // and the export buttons unlocked (the payload arrived from a URL
    // fragment, so it is by definition Share-sized).
    const md = '# Sample\n\nA paragraph.\n\n| Col A | Col B |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |\n';
    const fragment = buildPdfFragment({ v: 2, f: 'md', s: 'shared-doc', m: md });

    await page.goto(`${tool.path}#${fragment}`);

    await expect(page.locator('#tabMd')).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('#resultMd')).toBeVisible();
    await expect(page.locator('#resultMd')).toHaveValue(md);

    await expect(page.locator('#shareBtn')).toBeEnabled();
    await expect(page.locator('#copyBtn')).toBeEnabled();
    await expect(page.locator('#downloadBtn')).toBeEnabled();
    // Open-in-md-to-html applies to both Text and Markdown tabs.
    await expect(page.locator('#openMdBtn')).toBeEnabled();
  });
});
