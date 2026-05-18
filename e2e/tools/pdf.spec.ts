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

    // Share / Copy / Export are gated on a result, so they all start
    // disabled and the empty-state hint sits in the right pane.
    await expect(page.locator('#shareBtn')).toBeDisabled();
    await expect(page.locator('#copyBtn')).toBeDisabled();
    await expect(page.locator('#exportBtn')).toBeDisabled();
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

  test('inlined liteparse bundle exports getPdfLinks + getPdfOutline (Markdown/Outline passes)', async ({ page }) => {
    // The Markdown tab calls `getPdfLinks(bytes)` to extract hyperlink
    // annotations and `getPdfOutline(bytes)` to drive the heading
    // hierarchy from PDF bookmarks. The Outline tab also calls
    // `getPdfOutline(bytes)`. Verify the deploy pipeline shipped a
    // bundle that actually exposes both symbols -- a stale bundle
    // (built before either export was added) would silently downgrade
    // the affected tabs in production: Markdown would lose the
    // bookmark-driven heading hierarchy and the Outline tab would
    // 500-equivalent (we surface the error inline).
    await page.goto(tool.path);
    const exported = await page.evaluate(async () => {
      const el = document.getElementById('su-bundle-liteparse');
      const raw = el?.textContent?.trim() ?? '';
      if (!raw) return { found: [] as string[] };
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
        return { found: [] as string[] };
      }
      const found: string[] = [];
      for (const name of ['getPdfLinks', 'getPdfOutline']) {
        if (src.includes(name)) found.push(name);
      }
      return { found };
    });
    expect(exported.found, 'liteparse bundle must export getPdfLinks').toContain('getPdfLinks');
    expect(exported.found, 'liteparse bundle must export getPdfOutline').toContain('getPdfOutline');
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
    await expect(page.locator('#exportBtn')).toBeEnabled();
    // Stats line is now displayed inline with the file info under the
    // dropzone (instead of crowding the result toolbar). For shared
    // links the dropzone shows a "Loaded from shared link" pseudo-file.
    await expect(page.locator('#parseStats')).toContainText('shared link');
    await expect(page.locator('#fileName')).toContainText('Loaded from shared link');
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
    await expect(page.locator('#exportBtn')).toBeEnabled();

    // The Export button now opens a contextual menu (replaces the old
    // dedicated Download + Open-in-md-to-html buttons). Verify both
    // entries are present and that the "Open in Markdown to HTML" item
    // is enabled on the Markdown tab (it's disabled on JSON/Outline).
    await page.locator('#exportBtn').click();
    await expect(page.locator('#exportMenu')).toBeVisible();
    await expect(page.locator('#exportDownload')).toBeEnabled();
    await expect(page.locator('#exportDownloadDesc')).toContainText('.md');
    await expect(page.locator('#exportOpenMd')).toBeEnabled();
    // Escape closes the menu (mirrors the OCR options popover pattern).
    await page.keyboard.press('Escape');
    await expect(page.locator('#exportMenu')).toBeHidden();
  });

  test('hydrates the Outline tab from a shared v3 URL fragment', async ({ page }) => {
    // v3 share URL carries the resolved outline tree in `o` (destinations
    // pre-resolved to 1-indexed page numbers so the recipient doesn't need
    // the PDF bytes). The page should land on the Outline tab with the
    // tree populated and clickable jump-buttons for entries with a known
    // page.
    const outline = [
      {
        title: 'Introduction',
        level: 0,
        page: 1,
        children: [
          { title: 'Background', level: 1, page: 2, children: [] },
          { title: 'Goals', level: 1, page: 3, children: [] },
        ],
      },
      { title: 'Method', level: 0, page: 5, children: [] },
      { title: 'Unresolved entry', level: 0, page: null, children: [] },
    ];
    const fragment = buildPdfFragment({ v: 3, f: 'outline', s: 'shared-doc', o: outline });

    await page.goto(`${tool.path}#${fragment}`);

    await expect(page.locator('#tabOutline')).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('#resultOutline')).toBeVisible();

    // Top-level rows: Introduction, Method, Unresolved entry.
    const rows = page.locator('#resultOutline .outline-item-row');
    await expect(rows.filter({ hasText: 'Introduction' })).toBeVisible();
    await expect(rows.filter({ hasText: 'Method' })).toBeVisible();

    // Nested children render under their parent.
    await expect(rows.filter({ hasText: 'Background' })).toBeVisible();
    await expect(rows.filter({ hasText: 'Goals' })).toBeVisible();

    // Entries with a `page: null` destination render as disabled rows
    // (the title stays visible, but they can't be clicked through).
    const unresolved = rows.filter({ hasText: 'Unresolved entry' });
    await expect(unresolved).toBeDisabled();

    // The Outline tab is share-able too.
    await expect(page.locator('#shareBtn')).toBeEnabled();
  });

  test('v3 outline piggy-backs on a Text share URL', async ({ page }) => {
    // The wire format opportunistically piggy-backs the outline tree on
    // any non-outline share URL when it's small enough (the 4 KB JSON
    // cap in buildShareUrl). A recipient who lands on the Text tab via
    // such a URL should be able to click over to the Outline tab and
    // see the tree without re-deriving it from PDF bytes they don't
    // have. This is the path that makes "share a single URL with the
    // user, they get the navigable TOC for free" work.
    const outline = [
      { title: 'Section One', level: 0, page: 1, children: [] },
      { title: 'Section Two', level: 0, page: 4, children: [] },
    ];
    const fragment = buildPdfFragment({
      v: 3,
      f: 'text',
      s: 'shared-doc',
      t: 'Section One\nfoo bar\nSection Two\nbaz',
      o: outline,
    });

    await page.goto(`${tool.path}#${fragment}`);

    // Lands on the Text tab as requested.
    await expect(page.locator('#tabText')).toHaveAttribute('aria-selected', 'true');

    // Clicking the Outline tab renders the piggy-backed tree
    // immediately (no PDF bytes, no fetch).
    await page.locator('#tabOutline').click();
    await expect(page.locator('#resultOutline')).toBeVisible();
    await expect(page.locator('#resultOutline .outline-item-row').filter({ hasText: 'Section One' })).toBeVisible();
    await expect(page.locator('#resultOutline .outline-item-row').filter({ hasText: 'Section Two' })).toBeVisible();
  });

  test('Screenshots toolbar exposes search input and bbox toggle', async ({ page }) => {
    // Even with no PDF parsed (fresh load), the wiring for the Screenshots
    // tab's toolbar must be present in the DOM -- the toolbar lives inside
    // `#resultShotsWrap`, which is hidden until the user lands on the
    // Screenshots tab with a real PDF in scope, but the elements exist
    // and are discoverable for any future regression that drops one of
    // them. (Functional behavior is covered separately, since exercising
    // it end-to-end requires actually rendering a PDF.)
    await page.goto(tool.path);
    await expect(page.locator('#shotsSearchInput')).toHaveCount(1);
    await expect(page.locator('#shotsBoxesToggle')).toHaveCount(1);
    await expect(page.locator('#resultShotsToolbar')).toHaveCount(1);
  });

  test('OCR options popover exposes the language chip-picker', async ({ page }) => {
    // The previous free-text `#ocrLang` input has been replaced with a
    // chip-row + searchable combobox sourced from the Tesseract 4 LSTM
    // catalog. Verify the picker renders the default English chip, that
    // a search filters the suggestion list, and that clicking a result
    // adds a second chip (codes are joined with `+` before being handed
    // to tesseract.js, but that's an implementation detail of the
    // getOcrOptions() helper).
    await page.goto(tool.path);
    await page.locator('#ocrBtn').click();
    await expect(page.locator('#ocrPopover')).toBeVisible();

    // Old free-text input is gone, the new picker is in its place.
    await expect(page.locator('#ocrLang')).toHaveCount(0);
    await expect(page.locator('#langChips')).toBeVisible();
    await expect(page.locator('#langChips .lang-chip')).toHaveCount(1);
    await expect(page.locator('#langChips .lang-chip').first()).toContainText('English');

    // Focusing the search opens the suggestions list and shows every
    // catalog entry; the default `eng` selection has the checkmark.
    await page.locator('#langSearch').focus();
    await expect(page.locator('#langSuggestions')).toBeVisible();
    const totalSuggestions = await page.locator('#langSuggestions .lang-suggestion').count();
    expect(totalSuggestions, 'catalog should contain >= 100 languages').toBeGreaterThanOrEqual(100);
    await expect(page.locator('#langSuggestions .lang-suggestion[data-code="eng"]')).toHaveClass(/is-selected/);

    // Typing filters the list. The catalog has three German variants
    // (German, German Fraktur (Latin), German Fraktur (legacy)), so use
    // a more specific query that uniquely matches the modern entry --
    // "(modern)" appears nowhere else; assert at least one match and
    // that the most-relevant entry is the modern German one.
    await page.locator('#langSearch').fill('german');
    const filteredCount = await page.locator('#langSuggestions .lang-suggestion').count();
    expect(filteredCount).toBeGreaterThanOrEqual(1);
    expect(filteredCount).toBeLessThan(totalSuggestions);
    await expect(page.locator('#langSuggestions .lang-suggestion[data-code="deu"]')).toHaveCount(1);

    // Clicking a suggestion adds a chip; the chip-row is now [English, German].
    // (Mousedown is the real trigger so blur doesn't race the click.)
    await page.locator('#langSuggestions .lang-suggestion[data-code="deu"]').dispatchEvent('mousedown');
    await expect(page.locator('#langChips .lang-chip')).toHaveCount(2);
    await expect(page.locator('#langChips .lang-chip').nth(1)).toContainText('German');

    // The chip's × button removes it.
    await page.locator('#langChips .lang-chip[data-code="deu"] .lang-chip-remove').click();
    await expect(page.locator('#langChips .lang-chip')).toHaveCount(1);
  });
});
