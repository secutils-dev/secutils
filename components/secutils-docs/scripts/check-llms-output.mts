import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import docusaurusConfig from '../docusaurus.config.js';
import { THEME_MARKUP } from '../plugins/llms/index.ts';

const BUILD_DIR = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'build');

/**
 * The site origin varies by `SECUTILS_ENV` (the e2e docs image builds against `http://localhost:7171`) and the llms
 * plugin absolutizes every URL against it, so the expected prefix has to come from the same config the build read
 * rather than be pinned to production. Run `build` and this check with the same `SECUTILS_ENV`.
 */
const DOCS_URL = new URL(docusaurusConfig.baseUrl!, docusaurusConfig.url).href;

const FULL_FILE = 'llms.txt';
const INDEX_FILE = 'llms-index.txt';

/**
 * Markdown companions linked from `llms-index.txt`. Pinning the list catches a route or `slug` change silently moving a
 * companion (e.g. the doc root belongs at `docs.md`, not `project/intro.md`).
 */
const COMPANIONS = [
  'docs.md',
  'guides/digital_certificates/certificate_templates.md',
  'guides/digital_certificates/private_keys.md',
  'guides/platform/api_keys.md',
  'guides/platform/deno_runtime.md',
  'guides/platform/export_import.md',
  'guides/platform/notification_email.md',
  'guides/platform/secrets.md',
  'guides/platform/tags.md',
  'guides/platform/user_scripts.md',
  'guides/web_scraping/api.md',
  'guides/web_scraping/page.md',
  'guides/web_security/csp.md',
  'guides/webhooks.md',
  'project/api.md',
  'project/changelog.md',
  'project/changelog/2023.md',
  'project/changelog/2024.md',
  'project/changelog/2025.md',
  'project/roadmap.md',
];

/**
 * Fragments that only appear when the markdown is scraped from MDX sources instead of rendered HTML: the JSX of
 * `<Steps steps={[...]}/>` leaking as literal text, or un-normalized Docusaurus/Prism markup.
 */
const RESIDUE = [
  'caption: <>',
  ']} />',
  "img: '",
  "alt: '",
  '<Steps',
  '<SampleFields',
  '<CodeBlock',
  'su-steps__',
  'su-sample-fields',
  'theme-admonition',
  'token-line',
  'hash-link',
];

const read = (relativePath: string) => readFile(path.join(BUILD_DIR, relativePath), 'utf8');

const generated = new Map<string, string>();
for (const relativePath of [FULL_FILE, INDEX_FILE, ...COMPANIONS]) {
  generated.set(relativePath, await read(relativePath));
}

for (const [relativePath, content] of generated) {
  for (const fragment of RESIDUE) {
    assert.ok(!content.includes(fragment), `${relativePath} contains un-rendered markup: ${JSON.stringify(fragment)}`);
  }
  assert.ok(content.startsWith('# '), `${relativePath} does not start with a markdown heading.`);
}

// `<SampleFields>` renders a table of configuration values from a sample JSON file. It is deleted outright when the
// markdown is scraped from source, which silently drops the values a reader actually needs.
const webhooks = generated.get('guides/webhooks.md')!;
for (const value of ['**Name**:', 'HTML Responder', '/html-responder', 'Content-Type: text/html; charset=utf-8']) {
  assert.ok(webhooks.includes(value), `guides/webhooks.md is missing the sample value ${JSON.stringify(value)}.`);
}

// `<Steps>` must become an ordered list of screenshots, with image URLs absolute so they resolve from `llms.txt` too.
assert.match(webhooks, /^1\. !\[/m, 'guides/webhooks.md has no ordered step list.');
assert.ok(
  webhooks.includes(`${DOCS_URL}img/docs/guides/webhooks/`),
  `guides/webhooks.md step images are not absolute URLs under ${DOCS_URL}img/.`,
);

// Fenced code keeps its language, which is lost if Prism token markup is serialized verbatim.
assert.ok(webhooks.includes('```html'), 'guides/webhooks.md lost code block language hints.');

// Every construct the transforms handle must survive from the HTML into the markdown. Both sides are counted from the
// build rather than hard-coded, so adding or removing docs content never needs a test update, while a transform that
// stops matching - because Docusaurus renamed a class, say - drops the output count below the input count and fails.
const toHtmlPath = (companion: string) =>
  companion === 'docs.md' ? 'index.html' : companion.replace(/\.md$/, '/index.html');
const html = (await Promise.all(COMPANIONS.map((companion) => read(toHtmlPath(companion))))).join('\n');
const markdown = COMPANIONS.map((companion) => generated.get(companion)).join('\n');

const countOf = (haystack: string, pattern: RegExp) => haystack.match(pattern)?.length ?? 0;

// The transforms key off Docusaurus/Prism class names, which are implementation details rather than a public API.
// Asserting they are still in the built HTML turns a theme rename into a loud failure instead of silent degradation.
for (const marker of THEME_MARKUP) {
  assert.ok(html.includes(marker), `No "${marker}" in the built HTML; the llms plugin keys its transforms off it.`);
}

// Two details make these patterns look laxer than they are. Marker characters are alternatives because remark switches
// a list's marker (`1.` to `1)`, `-` to `*`) when it directly follows another list of the same kind, which would
// otherwise merge the two. And `[\s>]*` allows for blockquote nesting, since a construct inside an admonition comes out
// prefixed with `> ` - a code block in an admonition is the case that first broke this check.
const TRANSFORMS = [
  { name: 'admonition blockquotes', source: /class="theme-admonition /g, output: /^>[\s>]*\*\*/gm },
  { name: 'step list items', source: /class="su-steps__step"/g, output: /^[\s>]*\d+[.)] !\[/gm },
  { name: 'labelled table rows', source: /class="su-table"/g, output: /^[\s>]*[-*+] \*\*[^*\n]+\*\*:$/gm },
  // The lookahead keeps `theme-code-block-highlighted-line`, which marks a single line *inside* a block, from counting
  // as a block of its own. Fences come in pairs, so two markdown matches correspond to one HTML code block.
  { name: 'fenced code blocks', source: /class="[^"]*theme-code-block(?=[\s"])/g, output: /^[\s>]*```/gm, perBlock: 2 },
];

for (const { name, source, output, perBlock = 1 } of TRANSFORMS) {
  const expected = countOf(html, source);
  const produced = Math.floor(countOf(markdown, output) / perBlock);

  assert.ok(expected > 0, `No ${name} found in the built HTML; the check can no longer verify that transform.`);
  assert.ok(produced >= expected, `Only ${produced} of ${expected} ${name} survived into the markdown.`);
}

assert.ok(!/^[\s>]*#{1,6} .*\]\(#/m.test(markdown), 'A heading kept its hash-link anchor.');

// Relative targets are meaningless once a page is concatenated into `llms.txt`.
const relativeImage = markdown.match(/!\[[^\]]*\]\((?!https?:|data:)[^)]*\)/);
assert.equal(relativeImage, null, `Image target is not absolute: ${relativeImage?.[0]}`);

const index = generated.get(INDEX_FILE)!;
assert.ok(index.includes('## Table of Contents'), `${INDEX_FILE} has no table of contents.`);
for (const relativePath of COMPANIONS) {
  assert.ok(index.includes(`(${DOCS_URL}${relativePath})`), `${INDEX_FILE} does not link to ${relativePath}.`);
}

// The full file must hold every page's content, not just links. Each page contributes a `## <title>` section, matched
// against the companion's own `# <title>` heading rather than by counting - page bodies have their own `##` headings.
const full = generated.get(FULL_FILE)!;
assert.ok(full.length > 100_000, `${FULL_FILE} is only ${full.length} bytes; expected the full concatenated docs.`);
for (const relativePath of COMPANIONS) {
  const title = generated.get(relativePath)!.split('\n', 1)[0].replace(/^#\s+/, '');
  assert.ok(full.includes(`\n## ${title}\n`), `${FULL_FILE} has no section for ${relativePath} ("${title}").`);
}

console.log(`OK: ${FULL_FILE}, ${INDEX_FILE} and ${COMPANIONS.length} markdown companions are clean.`);
