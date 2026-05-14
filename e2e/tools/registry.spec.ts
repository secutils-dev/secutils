import { expect, test } from '@playwright/test';

import { PROMOTED_TOOLS, TOOLS, TOOLS_HOST } from './registry';

// Cross-cutting checks for the agent-discovery surface: llms.txt aggregate
// index, every per-tool .md, and the bare TOOLS_HOST root. These are
// intentionally redundant with the per-tool specs (which also poll the .md
// reachability) so a misconfigured responder shows up here even before any
// per-tool spec runs and times out.
test.describe('Tools registry - agent-discovery surface', () => {
  test('the tools host root returns 200', async ({ request }) => {
    const r = await request.get('/');
    expect(r.ok(), `GET https://${TOOLS_HOST}/ should succeed`).toBeTruthy();
  });

  test('llms.txt is reachable as text/markdown', async ({ request }) => {
    const r = await request.get('/llms.txt');
    expect(r.ok()).toBeTruthy();
    const ct = r.headers()['content-type'] ?? '';
    // Served as text/markdown (not text/plain) so the homepage's
    // `Accept: text/markdown` 302 redirect chain ends with the right MIME
    // and Cloudflare's markdown-for-agents contract is satisfied.
    expect(ct).toMatch(/text\/markdown/);
    const body = await r.text();
    expect(body).toMatch(/^#\s+Secutils/m);
  });

  for (const tool of TOOLS) {
    if (tool.slug === 'index') continue;
    test(`/${tool.slug}.md is reachable as text/markdown and lists in llms.txt`, async ({ request }) => {
      const r = await request.get(`${tool.path}.md`);
      expect(r.ok(), `GET ${tool.path}.md should succeed`).toBeTruthy();
      const ct = r.headers()['content-type'] ?? '';
      expect(ct).toMatch(/text\/markdown/);

      const body = await r.text();

      // SKILL.md shape: YAML frontmatter with at least `name` and
      // `description`, then a Markdown body. We deliberately do not
      // assert `name` matches the slug -- skill names follow installable
      // skill conventions (e.g. `mock-response` rather than `echo`).
      expect(body.startsWith('---\n'), 'must be YAML frontmatter').toBeTruthy();
      const fm = body.match(/^---\n([\s\S]*?)\n---/);
      expect(fm, 'frontmatter block must close with ---').not.toBeNull();
      expect(fm![1]).toMatch(/^name:\s*\S+/m);
      expect(fm![1]).toMatch(/^description:\s*\S/m);

      const llms = await (await request.get('/llms.txt')).text();
      if (tool.promote) {
        expect(llms, 'llms.txt should reference this tool').toContain(tool.path);
      } else {
        // Non-promoted tools must be reachable via direct link, but must
        // not be advertised in any aggregate discovery surface (see
        // `dev/tools/AGENTS.md` -> "Promotion (su-tool-promote)").
        expect(llms, 'llms.txt must not advertise non-promoted tools').not.toContain(tool.path);
      }
    });
  }

  // Per-tool agent-readiness: every HTML responder honours
  // `Accept: text/markdown` content negotiation by 302-redirecting to the
  // `<slug>.md` sibling (or `/llms.txt` for the index). Browsers and
  // `Accept: */*` clients are unaffected.
  for (const tool of TOOLS) {
    test(`${tool.path} negotiates Accept: text/markdown to its .md sibling`, async ({ request }) => {
      const r = await request.get(tool.path, {
        headers: { Accept: 'text/markdown' },
        maxRedirects: 0,
      });
      expect(r.status(), `GET ${tool.path} with Accept: text/markdown`).toBe(302);
      const expected = tool.slug === 'index' ? '/llms.txt' : `${tool.path}.md`;
      expect(r.headers().location, 'Location must point at the .md sibling').toBe(expected);
      expect(r.headers().vary ?? '', 'Vary: Accept must be set so caches keep variants distinct').toMatch(/Accept/i);
    });
  }
});

test.describe('Tools registry - cross-cutting agent-discovery artefacts', () => {
  test('/robots.txt allows AI crawlers and references the sitemap', async ({ request }) => {
    const r = await request.get('/robots.txt');
    expect(r.ok()).toBeTruthy();
    expect(r.headers()['content-type'] ?? '').toMatch(/text\/plain/);
    const body = await r.text();
    expect(body).toMatch(/^User-agent:\s*\*\s*$\s*Allow:\s*\/\s*$/m);
    expect(body, 'must explicitly allow GPTBot').toMatch(/^User-agent:\s*GPTBot\s*$\s*Allow:\s*\/\s*$/m);
    expect(body, 'must explicitly allow ClaudeBot').toMatch(/^User-agent:\s*ClaudeBot\s*$\s*Allow:\s*\/\s*$/m);
    expect(body, 'must declare Content Signals').toMatch(/Content-Signal:\s*ai-train=yes/);
    expect(body, 'must point at the sitemap').toMatch(new RegExp(`Sitemap:\\s*https://${TOOLS_HOST}/sitemap\\.xml`));
  });

  test('/sitemap.xml is well-formed and lists every public surface', async ({ request }) => {
    const r = await request.get('/sitemap.xml');
    expect(r.ok()).toBeTruthy();
    expect(r.headers()['content-type'] ?? '').toMatch(/application\/xml/);
    const body = await r.text();
    expect(body).toMatch(/^<\?xml version="1\.0" encoding="UTF-8"\?>/);
    expect(body).toContain(`<loc>https://${TOOLS_HOST}/</loc>`);
    expect(body).toContain(`<loc>https://${TOOLS_HOST}/llms.txt</loc>`);
    expect(body).toContain(`<loc>https://${TOOLS_HOST}/.well-known/agent-skills/index.json</loc>`);
    for (const tool of PROMOTED_TOOLS) {
      expect(body, `sitemap must list ${tool.path}`).toContain(`<loc>https://${TOOLS_HOST}${tool.path}</loc>`);
      expect(body, `sitemap must list ${tool.path}.md`).toContain(`<loc>https://${TOOLS_HOST}${tool.path}.md</loc>`);
    }
    for (const tool of TOOLS.filter((t) => !t.promote && t.slug !== 'index')) {
      expect(body, `sitemap must NOT list non-promoted ${tool.path}`).not.toContain(
        `<loc>https://${TOOLS_HOST}${tool.path}</loc>`,
      );
    }
  });

  test('/.well-known/agent-skills/index.json conforms to the v0.2.0 shape', async ({ request }) => {
    const r = await request.get('/.well-known/agent-skills/index.json');
    expect(r.ok()).toBeTruthy();
    expect(r.headers()['content-type'] ?? '').toMatch(/application\/json/);
    const doc = await r.json();
    expect(doc).toHaveProperty('$schema');
    expect(doc.$schema).toMatch(/agentskills/);
    expect(Array.isArray(doc.skills)).toBe(true);
    expect(doc.skills.length, 'should list at least one skill').toBeGreaterThan(0);
    for (const skill of doc.skills) {
      expect(skill, 'every entry must be type=skill').toMatchObject({ type: 'skill' });
      expect(skill.name, 'name must be non-empty').toMatch(/\S/);
      expect(skill.description, 'description must be non-empty').toMatch(/\S/);
      expect(skill.url, 'url must point at our tools host .md').toMatch(new RegExp(`^https://${TOOLS_HOST}/.+\\.md$`));
      expect(skill.sha256, 'sha256 must be 64 hex chars').toMatch(/^[0-9a-f]{64}$/);
    }
    // Non-promoted tools must not advertise their skill in the discovery
    // index (their `<path>.md` is still served for direct fetching).
    const urls = doc.skills.map((s: { url: string }) => s.url);
    for (const tool of TOOLS.filter((t) => !t.promote && t.slug !== 'index')) {
      const expected = `https://${TOOLS_HOST}${tool.path}.md`;
      expect(urls, `agent-skills index must NOT advertise non-promoted ${tool.path}`).not.toContain(expected);
    }
  });

  test('the index page advertises discovery surfaces via Link headers', async ({ request }) => {
    const r = await request.get('/');
    expect(r.ok()).toBeTruthy();
    // Multiple `Link` headers may be merged by the HTTP stack into a single
    // comma-separated string -- match either way.
    const link = r.headers().link ?? '';
    expect(link, 'must advertise llms.txt').toMatch(/<\/llms\.txt>;\s*rel="describedby";\s*type="text\/markdown"/);
    expect(link, 'must advertise agent-skills index').toMatch(
      /<\/\.well-known\/agent-skills\/index\.json>;\s*rel="describedby"/,
    );
    expect(link, 'must advertise sitemap').toMatch(/<\/sitemap\.xml>;\s*rel="sitemap"/);
  });
});
