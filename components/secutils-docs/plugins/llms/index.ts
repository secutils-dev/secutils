import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

import type { LoadContext, Plugin, PluginRouteConfig, RouteConfig } from '@docusaurus/types';
import type { Element, ElementContent, Root, RootContent, Text } from 'hast';

export interface LlmsPluginOptions {
  /** Heading used at the top of both generated `.txt` files. */
  title: string;
  /** Blockquote line placed under the heading of both generated `.txt` files. */
  description: string;
  /** Paragraph placed before the table of contents in the index file. */
  rootContent: string;
  /** Paragraph placed before the concatenated pages in the full file. */
  fullRootContent: string;
  /** Name of the compact table-of-contents file. */
  indexFilename: string;
  /** Name of the file holding every page's full content. */
  fullFilename: string;
}

const DOCS_PLUGIN_NAME = 'docusaurus-plugin-content-docs';

/** Page whose rendered HTML is converted into a markdown companion. */
interface DocPage {
  /** Route the page is served at, including `baseUrl` (e.g. `/docs/guides/webhooks`). */
  routePath: string;
}

interface RenderedPage extends RenderedHtml {
  /** Path of the markdown companion relative to `outDir` (e.g. `guides/webhooks.md`). */
  outputPath: string;
}

export interface RenderedHtml {
  title: string;
  description: string;
  markdown: string;
}

export interface RenderContext {
  /** Absolute URL the page is served at, used to resolve relative image and link targets. */
  pageUrl: string;
  /** Suffix Docusaurus appends to every `<title>`, stripped to recover the page's own title. */
  siteTitle: string;
}

/**
 * Theme class names the transforms below key off. They are Docusaurus/Prism implementation details rather than a public
 * API, so `scripts/check-llms-output.mts` asserts they still appear in the built HTML: a rename would otherwise make
 * every rule silently no-op and quietly degrade the markdown instead of failing.
 */
export const THEME_MARKUP = [
  'theme-doc-markdown',
  'theme-code-block',
  'token-line',
  'theme-admonition',
  'admonitionHeading',
  'admonitionContent',
  'hash-link',
  'su-steps__step',
  'su-steps__caption',
  'su-table',
];

/**
 * Builds the HTML-to-markdown converter. Exposed separately from the plugin so `index.test.mts` can exercise the
 * transforms against small HTML fixtures without running a full site build.
 */
export async function createRenderer(): Promise<(html: string, context: RenderContext) => RenderedHtml> {
  // These packages are ESM-only while this plugin is loaded through jiti as CommonJS.
  const { unified } = await import('unified');
  const rehypeParse = (await import('rehype-parse')).default;
  const rehypeRemark = (await import('rehype-remark')).default;
  const remarkGfm = (await import('remark-gfm')).default;
  const remarkStringify = (await import('remark-stringify')).default;

  const parser = unified().use(rehypeParse);
  const toMarkdown = unified().use(rehypeRemark).use(remarkGfm).use(remarkStringify, { bullet: '-', fences: true });

  return (html, { pageUrl, siteTitle }) => {
    const tree = parser.parse(html);

    // The `<article>` element wraps breadcrumbs and pagination, so narrow to the doc body itself.
    const container = findElement(tree, (element) => hasClass(element, 'theme-doc-markdown'));
    if (!container) {
      throw new Error('[secutils-llms] No ".theme-doc-markdown" container in the rendered HTML.');
    }

    rewrite(container, [dropHashLink, unwrapCodeBlock, unwrapLabelledTable, unwrapSteps, unwrapAdmonition]);
    absolutizeUrls(container, pageUrl);

    const mdast = toMarkdown.runSync({ type: 'root', children: container.children });
    return {
      title: extractTitle(tree, siteTitle),
      description: extractDescription(tree),
      markdown: toMarkdown.stringify(mdast).trim(),
    };
  };
}

/**
 * Generates `llms.txt`, a compact index, and a per-page markdown companion from the *rendered* HTML.
 *
 * Reading the built HTML rather than the MDX sources is what makes `<Steps>` and `<SampleFields>` show up at all:
 * they are React components, so their content only exists after rendering.
 */
export default function llmsPlugin(_context: LoadContext, options: Partial<LlmsPluginOptions>): Plugin<void> {
  const {
    title = '',
    description = '',
    rootContent = '',
    fullRootContent = '',
    indexFilename = 'llms-index.txt',
    fullFilename = 'llms.txt',
  } = options;

  return {
    name: 'secutils-llms',

    async postBuild({ routes, outDir, baseUrl, siteConfig }) {
      const render = await createRenderer();

      const pages = collectDocPages(routes).sort((a, b) => a.routePath.localeCompare(b.routePath));
      if (pages.length === 0) {
        throw new Error(
          '[secutils-llms] No doc routes carried "metadata.sourceFilePath"; cannot map routes to markdown files.',
        );
      }

      const rendered: RenderedPage[] = [];
      for (const page of pages) {
        const htmlPath = path.join(outDir, routeToDir(page.routePath, baseUrl), 'index.html');
        const html = await readFile(htmlPath, 'utf8');

        // Relative links are authored against the route *without* a trailing slash: from `/docs/guides/webhooks`,
        // `../img/x.png` resolves to `/docs/img/x.png`. Keeping the trailing slash would resolve one level too deep.
        const pageUrl = new URL(page.routePath.replace(/\/$/, ''), siteConfig.url).href;

        try {
          rendered.push({
            ...render(html, { pageUrl, siteTitle: siteConfig.title }),
            outputPath: routeToOutputPath(page.routePath, baseUrl),
          });
        } catch (error) {
          throw new Error(`[secutils-llms] Failed to convert ${htmlPath}.`, { cause: error });
        }
      }

      await Promise.all(
        rendered.map(async (page) => {
          const filePath = path.join(outDir, page.outputPath);
          await mkdir(path.dirname(filePath), { recursive: true });
          await writeFile(filePath, `${heading(page.title, page.description)}\n\n${page.markdown}\n`);
        }),
      );

      const docsUrl = new URL(baseUrl, siteConfig.url).href;
      const toc = rendered.map((page) => {
        const url = new URL(page.outputPath, docsUrl).href;
        return `- [${page.title}](${url})${page.description ? `: ${page.description}` : ''}`;
      });
      const body = rendered.map((page) => `## ${page.title}\n\n${page.markdown}`);

      await writeFile(
        path.join(outDir, indexFilename),
        `${heading(title, description)}\n\n${rootContent}\n\n## Table of Contents\n\n${toc.join('\n')}\n`,
      );
      await writeFile(
        path.join(outDir, fullFilename),
        `${heading(title, description)}\n\n${fullRootContent}\n\n${body.join('\n\n')}\n`,
      );

      console.log(`[secutils-llms] Wrote ${fullFilename}, ${indexFilename} and ${rendered.length} companions.`);
    },
  };
}

function heading(title: string, description: string): string {
  return description ? `# ${title}\n\n> ${description}` : `# ${title}`;
}

function collectDocPages(routes: PluginRouteConfig[]): DocPage[] {
  const pages: DocPage[] = [];

  const walkRoute = (route: RouteConfig, pluginName: string | undefined): void => {
    // `sourceFilePath` is only set on routes backed by a real doc file, which conveniently excludes the docs plugin's
    // wrapper route and any generated category index pages.
    if (pluginName === DOCS_PLUGIN_NAME && route.metadata?.sourceFilePath) {
      pages.push({ routePath: route.path });
    }
    for (const child of route.routes ?? []) {
      walkRoute(child, pluginName);
    }
  };

  for (const route of routes) {
    walkRoute(route, route.plugin?.name);
  }

  return pages;
}

/** `/docs/guides/webhooks` + `/docs/` -> `guides/webhooks` (the directory holding the route's `index.html`). */
function routeToDir(routePath: string, baseUrl: string): string {
  const relative = routePath.startsWith(baseUrl) ? routePath.slice(baseUrl.length) : routePath.replace(/^\//, '');
  return relative.replace(/\/$/, '');
}

/**
 * Companion paths follow the llmstxt convention of appending `.md` to the page URL, so they are derived from the route
 * rather than the source file - `docs/project/intro.md` has `slug: /` and belongs at `docs.md`, not `project/intro.md`.
 * The doc root has no path left once `baseUrl` is stripped, so it falls back to the last `baseUrl` segment (`docs.md`).
 */
function routeToOutputPath(routePath: string, baseUrl: string): string {
  const relative = routeToDir(routePath, baseUrl);
  return `${relative || baseUrl.replace(/\/$/, '').split('/').pop()}.md`;
}

function extractTitle(tree: Root, siteTitle: string): string {
  const title = findElement(tree, (element) => element.tagName === 'title');
  return textOf(title).replace(new RegExp(`\\s*\\|\\s*${escapeRegExp(siteTitle)}$`), '').trim();
}

function extractDescription(tree: Root): string {
  const meta = findElement(tree, (element) => element.tagName === 'meta' && element.properties?.name === 'description');
  return String(meta?.properties?.content ?? '').trim();
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/**
 * Replaces elements bottom-up so nested constructs are normalized before their container sees them - a code block
 * inside a `.su-table` cell is already a plain `<pre>` by the time the table rule runs.
 */
type Rule = (node: Element) => ElementContent[] | undefined;

function rewrite(parent: Element, rules: Rule[]): void {
  const children: ElementContent[] = [];

  for (const child of parent.children) {
    if (child.type !== 'element') {
      children.push(child);
      continue;
    }

    rewrite(child, rules);

    let replacement: ElementContent[] | undefined;
    for (const rule of rules) {
      replacement = rule(child);
      if (replacement) {
        break;
      }
    }

    children.push(...(replacement ?? [child]));
  }

  parent.children = children;
}

function walk(node: Element, visitor: (element: Element) => void): void {
  for (const child of node.children) {
    if (child.type === 'element') {
      visitor(child);
      walk(child, visitor);
    }
  }
}

/** Heading anchors would otherwise serialize as trailing link junk on every heading. */
const dropHashLink: Rule = (node) => (node.tagName === 'a' && hasClass(node, 'hash-link') ? [] : undefined);

/**
 * Prism renders each line as a `<div class="token-line">` of `<span>`s. Serializing that verbatim produces garbage,
 * so rebuild a plain `<pre><code>` that `hast-util-to-mdast` turns into a fenced block.
 */
const unwrapCodeBlock: Rule = (node) => {
  if (!hasClass(node, 'theme-code-block')) {
    return undefined;
  }

  const language = classesOf(node)
    .find((name) => name.startsWith('language-'))
    ?.slice('language-'.length);
  const lines = findElements(node, (element) => hasClass(element, 'token-line')).map(textOf);
  // Fall back to the `<pre>` only, never the whole block: the copy button lives outside it.
  const code = lines.length > 0 ? lines.join('\n') : textOf(findElement(node, (element) => element.tagName === 'pre'));

  const properties = language && language !== 'text' ? { className: [`language-${language}`] } : {};
  return [element('pre', [element('code', [text(code.replace(/\s+$/, ''))], properties)])];
};

/**
 * `.su-table` is a two-column label/value grid whose values are code blocks. GFM table cells cannot hold fenced code
 * and the values are often multi-line, so emit a list of labels each followed by its block instead.
 */
const unwrapLabelledTable: Rule = (node) => {
  if (node.tagName !== 'table' || !hasClass(node, 'su-table')) {
    return undefined;
  }

  const rows = findElements(node, (element) => element.tagName === 'tr').map((row) =>
    findElements(row, (element) => element.tagName === 'td' || element.tagName === 'th'),
  );
  if (rows.length === 0 || rows.some((cells) => cells.length !== 2)) {
    return undefined;
  }

  const items = rows.map(([label, value]) =>
    element('li', [element('p', [element('strong', [text(textOf(label).trim())]), text(':')]), ...value.children]),
  );
  return [element('ul', items)];
};

/** `.su-steps` is a numbered screenshot walkthrough; an ordered list carries the same meaning in markdown. */
const unwrapSteps: Rule = (node) => {
  if (!hasClass(node, 'su-steps')) {
    return undefined;
  }

  // Taking only `.su-steps__step` drops both the `.su-steps__indicator` counters (the list numbers them) and the
  // trailing `<dialog>` lightbox, whose empty `<img>` would otherwise emit a stray image.
  const items = childElements(node)
    .filter((step) => hasClass(step, 'su-steps__step'))
    .map((step) => {
      const content = findElement(step, (element) => hasClass(element, 'su-steps__content')) ?? step;
      const image = findElement(content, (element) => element.tagName === 'img');
      const caption = findElement(content, (element) => hasClass(element, 'su-steps__caption'));
      return element('li', [...(image ? [element('p', [image])] : []), ...(caption?.children ?? [])]);
    });

  return items.length > 0 ? [element('ol', items)] : undefined;
};

/** Admonitions carry their kind in a class and their label in a heading whose icon must not leak into the text. */
const unwrapAdmonition: Rule = (node) => {
  if (!hasClass(node, 'theme-admonition')) {
    return undefined;
  }

  const kind = classesOf(node)
    .find((name) => name.startsWith('theme-admonition-'))
    ?.slice('theme-admonition-'.length);
  const label = findElement(node, (element) => startsWithClass(element, 'admonitionHeading'));
  const content = findElement(node, (element) => startsWithClass(element, 'admonitionContent'));
  const title = (textOf(label) || kind || '').trim();

  return [
    element('blockquote', [
      ...(title ? [element('p', [element('strong', [text(title.charAt(0).toUpperCase() + title.slice(1))])])] : []),
      ...(content?.children ?? node.children),
    ]),
  ];
};

function absolutizeUrls(container: Element, pageUrl: string): void {
  walk(container, (node) => {
    const attribute = node.tagName === 'img' ? 'src' : node.tagName === 'a' ? 'href' : undefined;
    if (!attribute) {
      return;
    }

    const value = node.properties?.[attribute];
    if (typeof value !== 'string' || value === '' || /^[a-z][a-z0-9+.-]*:/i.test(value) || value.startsWith('#')) {
      return;
    }

    node.properties[attribute] = new URL(value, pageUrl).href;
  });
}

function element(tagName: string, children: ElementContent[], properties: Element['properties'] = {}): Element {
  return { type: 'element', tagName, properties, children };
}

function text(value: string): Text {
  return { type: 'text', value };
}

function classesOf(node: Element): string[] {
  const className = node.properties?.className;
  return Array.isArray(className) ? className.map(String) : [];
}

function hasClass(node: Element, name: string): boolean {
  return classesOf(node).includes(name);
}

/** Docusaurus suffixes its own class names with a build hash (e.g. `admonitionHeading_Gvgb`). */
function startsWithClass(node: Element, prefix: string): boolean {
  return classesOf(node).some((name) => name.startsWith(prefix));
}

function childElements(node: Element): Element[] {
  return node.children.filter((child): child is Element => child.type === 'element');
}

function findElement(tree: Root | Element, predicate: (element: Element) => boolean): Element | undefined {
  for (const child of tree.children) {
    if (child.type !== 'element') {
      continue;
    }
    if (predicate(child)) {
      return child;
    }
    const nested = findElement(child, predicate);
    if (nested) {
      return nested;
    }
  }
  return undefined;
}

function findElements(tree: Root | Element, predicate: (element: Element) => boolean): Element[] {
  const found: Element[] = [];
  for (const child of tree.children) {
    if (child.type === 'element') {
      if (predicate(child)) {
        found.push(child);
      }
      found.push(...findElements(child, predicate));
    }
  }
  return found;
}

function textOf(node: Root | RootContent | undefined): string {
  if (!node) {
    return '';
  }
  if (node.type === 'text') {
    return node.value;
  }
  return 'children' in node ? node.children.map(textOf).join('') : '';
}
