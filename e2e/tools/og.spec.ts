import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

import { test } from '@playwright/test';

import { goto } from '../helpers';
import { type Tool, TOOLS } from './registry';

// Output directory: Docusaurus serves `static/*` verbatim under `/docs/...`,
// so the final URL is the stable absolute `https://secutils.dev/docs/img/og/og-<slug>.png`
// referenced from each tool's `<head>`.
const OG_DIR = resolve(__dirname, '../../components/secutils-docs/static/img/og');
mkdirSync(OG_DIR, { recursive: true });

// Local-only template URL. The template is parameterless until the script
// reads `?name=...&path=...&desc=...&accent=...&icon=...&theme=...`. We use
// `file://` so this spec needs no server (no `make e2e-up`, no Docker).
const TEMPLATE_FILE_URL = pathToFileURL(resolve(__dirname, '../../dev/tools/og-template.html')).toString();

function buildTemplateUrl(tool: Tool, theme: 'dark' | 'light'): string {
  const params = new URLSearchParams({
    name: tool.name,
    path: tool.path,
    desc: tool.description,
    accent: tool.accent,
    icon: tool.icon,
    theme,
  });
  return `${TEMPLATE_FILE_URL}?${params.toString()}`;
}

// Capture at exactly the OG canvas size so the image is dimensioned correctly
// at the source (no Playwright clipping math, no scaling artefacts).
test.use({ viewport: { width: 1200, height: 630 } });

test.describe('OG image generation', () => {
  for (const tool of TOOLS) {
    test(`renders dark OG image for ${tool.slug}`, async ({ page }) => {
      await goto(page, buildTemplateUrl(tool, 'dark'));
      const path = resolve(OG_DIR, `og-${tool.slug}.png`);
      // Full-viewport screenshot at 1200x630. `goto()` patched
      // `page.screenshot` to run waitForStableUiBeforeScreenshot before and
      // stabilizeScreenshot after, so re-runs converge byte-for-byte.
      await page.screenshot({ path, fullPage: false });
    });

    test(`renders light OG image for ${tool.slug}`, async ({ page }) => {
      await goto(page, buildTemplateUrl(tool, 'light'));
      const path = resolve(OG_DIR, `og-${tool.slug}-light.png`);
      await page.screenshot({ path, fullPage: false });
    });
  }
});
