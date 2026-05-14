import { type APIRequestContext, expect, type Page } from '@playwright/test';

import { type Tool, TOOLS, TOOLS_HOST } from './registry';

/**
 * Resolves the canonical Tool definition for a slug. Throws if the slug is not
 * in the registry so a typo in a spec fails loudly instead of silently
 * skipping assertions.
 */
export function getTool(slug: string): Tool {
  const tool = TOOLS.find((t) => t.slug === slug);
  if (!tool) {
    throw new Error(`Unknown tool slug "${slug}"; update e2e/tools/registry.ts`);
  }
  return tool;
}

/**
 * Asserts the SEO basics common to every free tool page: title, meta
 * description, canonical, robots, OG tag set, twitter card, and a parseable
 * WebApplication or ItemList JSON-LD block. These mirror the head block
 * documented in dev/tools/AGENTS.md so a regression here means the SEO budget
 * has degraded.
 */
export async function assertSeoBasics(page: Page, tool: Tool): Promise<void> {
  const html = await page.content();

  await expect(page).toHaveTitle(/Secutils\.dev/);
  await expect(page).toHaveTitle(new RegExp(escapeRegex(tool.name).split(' ').join('.{0,4}'), 'i'));

  const canonical = page.locator('link[rel="canonical"]');
  await expect(canonical).toHaveAttribute('href', `https://${TOOLS_HOST}${tool.path}`);

  const robots = page.locator('meta[name="robots"]');
  await expect(robots).toHaveAttribute('content', /index/);

  const description = page.locator('meta[name="description"]');
  await expect(description).toHaveAttribute('content', /.{60,}/);

  for (const property of ['og:type', 'og:title', 'og:description', 'og:url', 'og:image']) {
    const el = page.locator(`meta[property="${property}"]`);
    await expect(el).toHaveAttribute('content', /\S/);
  }

  await expect(page.locator('meta[property="og:url"]')).toHaveAttribute('content', `https://${TOOLS_HOST}${tool.path}`);

  for (const name of ['twitter:card', 'twitter:title', 'twitter:image']) {
    const el = page.locator(`meta[name="${name}"]`);
    await expect(el).toHaveAttribute('content', /\S/);
  }

  for (const introspection of ['su-tool-name', 'su-tool-description', 'su-tool-promote', 'su-tool-path']) {
    await expect(page.locator(`meta[name="${introspection}"]`)).toHaveAttribute('content', /\S/);
  }
  await expect(page.locator('meta[name="su-tool-promote"]')).toHaveAttribute('content', String(tool.promote));

  // JSON-LD must parse. The index page emits an ItemList; the per-tool pages
  // emit a WebApplication. Both must be valid schema.org with a name + url.
  const jsonLd = await page.locator('script[type="application/ld+json"]').first().textContent();
  expect(jsonLd, 'JSON-LD script tag should be present').toBeTruthy();
  const ld = JSON.parse(jsonLd!);
  expect(ld['@context']).toBe('https://schema.org');
  expect(['WebApplication', 'ItemList']).toContain(ld['@type']);
  expect(typeof ld.name).toBe('string');
  expect(typeof ld.url).toBe('string');

  const noscript = await page.locator('noscript').first().textContent();
  expect(noscript ?? '', 'noscript fallback paragraph should mention JavaScript').toMatch(/JavaScript/i);

  expect(html.length, 'tool HTML should be substantial enough to be useful').toBeGreaterThan(2000);
}

/**
 * Asserts that the AI-agent skill markdown lives at `<tool.path>.md`, returns
 * `text/markdown`, and is a valid Claude Code / Cursor SKILL.md: minimal
 * frontmatter (`name` + `description`) followed by a Markdown body. For the
 * index tool there is no skill -- we instead verify `/llms.txt` is reachable
 * as `text/plain` and starts with the expected header.
 */
export async function assertSkillMd(request: APIRequestContext, tool: Tool): Promise<void> {
  if (tool.slug === 'index') {
    const r = await request.get('/llms.txt');
    expect(r.ok(), 'GET /llms.txt should succeed').toBeTruthy();
    const text = await r.text();
    expect(text).toMatch(/^# Secutils/m);
    return;
  }

  const url = `${tool.path}.md`;
  const r = await request.get(url);
  expect(r.ok(), `GET ${url} should succeed`).toBeTruthy();
  const ct = r.headers()['content-type'] ?? '';
  expect(ct, `Content-Type for ${url} must include text/markdown`).toMatch(/text\/markdown/);

  const body = await r.text();

  // Minimal SKILL.md shape: opening `---` frontmatter, a `name:` and
  // `description:` key, a closing `---`, and at least one `# ` heading in
  // the body. We deliberately do not assert on `name` matching the slug
  // (e.g. `echo.skill.md` declares `name: mock-response` to align with the
  // installed Anthropic skill) and we do not assert any other frontmatter
  // keys -- the rich detail lives in the body where a skill loader and a
  // human reader both consume it.
  expect(body.startsWith('---\n'), 'skill .md must begin with YAML frontmatter').toBeTruthy();
  const fm = body.match(/^---\n([\s\S]*?)\n---\n([\s\S]*)$/);
  expect(fm, 'skill .md must have a closed YAML frontmatter block').not.toBeNull();
  const [, frontmatter, mdBody] = fm!;
  expect(frontmatter).toMatch(/^name:\s*\S+/m);
  expect(frontmatter).toMatch(/^description:\s*\S/m);
  expect(mdBody).toMatch(/^# /m);
}

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
