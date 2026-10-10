# Docusaurus Markdown output and serving

Read this guide for Docusaurus upgrades, the local LLM plugin, generated Markdown, or docs Nginx changes.
Commands run from the Secutils root.

## Rendered HTML is the source

`components/secutils-docs/plugins/llms/index.ts` runs in `postBuild` and converts rendered `.theme-doc-markdown` content to Markdown.
Raw MDX does not contain the rendered text of React components such as `<Steps>` or `<SampleFields>`.
Keep extraction after rendering so those components survive in the output.

Read the plugin's exported `THEME_MARKUP` for its theme selectors.
The transforms cover steps, tables, Prism lines, admonitions, and hash links.
After Docusaurus/theme changes, run `check-llms` against the new build.
It checks selectors and compares construct counts between HTML and Markdown.
Fixture tests alone cannot establish compatibility with changed theme markup.

- Derive companion `.md` paths from routes, rather than source paths.
  `docs/project/intro.md` has `slug: /` and produces `build/docs.md`.
- Resolve relative URLs against the route without a trailing slash.
  `../img/x.png` from `/docs/guides/webhooks` resolves to `/docs/img/x.png`.
- Preserve `.ts` in the plugin path in `docusaurus.config.js`.
  Docusaurus uses plain require resolution before jiti transpilation.
- Load ESM-only unified/rehype/remark packages through dynamic imports inside `postBuild` for the CommonJS configuration.
- For Markdown assertions, accept remark's alternate list markers and admonition `> ` prefixes.

## MIME and routing

Nginx server-level `types {}` replaces the inherited MIME map.
Include `/etc/nginx/mime.types` in that block before the `text/markdown md;` entry.
Serve companion `.md`, `llms.txt`, and `llms-index.txt` as `text/markdown`.
The `.txt` files contain llmstxt-format Markdown and are content-negotiation redirect targets.

The production Traefik route adds `/docs` to public `/llms.txt` and `/llms-index.txt` requests.
Match the prefixed paths in the docs Nginx override:

```nginx
location ~ ^/docs/llms(-index)?\.txt$ {
    types { }
    default_type text/markdown;
}
```

The empty location MIME map lets `default_type` override `.txt` handling for these paths.
For in-container smoke checks, request `/docs/llms.txt`, rather than `/llms.txt`.

## Completion gate

```bash
npm --prefix components/secutils-docs run typecheck
npm --prefix components/secutils-docs test
npm --prefix components/secutils-docs run build
npm --prefix components/secutils-docs run check-llms
```

Inspect generated `llms.txt`, `llms-index.txt`, and affected page companions.
For serving changes, require successful HTTP responses and `Content-Type: text/markdown` at the actual routed paths.
