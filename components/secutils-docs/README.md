The documentation website for Secutils.dev.

## Getting started

Install all the required dependencies with `npm install` and run the UI in watch mode with `npm run start`.

### Usage

The docs website should be accessible at http://localhost:7373.

## Agent skill files (`static/guides/.../SKILL.md`)

Alongside each `.mdx` guide we publish a hand-authored `SKILL.md` file at `static/guides/<area>/<name>/SKILL.md` that 
conforms to the [Cloudflare Agent Skills Discovery RFC v0.2.0](https://github.com/cloudflare/agent-skills-discovery-rfc).
Docusaurus copies the `static/` tree verbatim into `build/`, so the file is served at 
`https://secutils.dev/docs/guides/<area>/<name>/SKILL.md` with `Content-Type: text/markdown`.

The promo site at https://secutils.dev aggregates these URLs into its `/.well-known/agent-skills/index.json` discovery 
document. The aggregator lives in a separate repository under `components/secutils-dev-webui/src/skills.source.json` and
keys off each SKILL.md's frontmatter `name:` field. When adding or editing a SKILL.md here:

1. Use a unique `name:` slug (lowercased, hyphen-separated). It must match the corresponding entry in the promo repo's 
   `skills.source.json`.
2. Keep the YAML frontmatter minimal: `name:` and `description:` only. Use the folded scalar form (`description: >-`) 
   for descriptions longer than one line.
3. Author the body in clean Markdown. Avoid em-dashes, use commas instead. Do not wrap lines aggressively, aim for
   roughly 120 columns at sentence boundaries.
4. After editing, ask the promo repo's maintainer to rerun `npm run regenerate-skills` so the agent-skills index picks 
   up the new sha256 digest. The promo script defaults to reading the bytes from this repo through a sibling 
   working-tree `localPath`, so the digest matches the file the docs build will serve.

When adding an entirely new SKILL.md, also add a corresponding entry (`name`, `description`, `url`, `localPath`) to
`components/secutils-dev-webui/src/skills.source.json` in the promo repo and regenerate.

### "Skill" badge on the rendered guide

Every guide whose frontmatter declares `skill: true` automatically renders a small "Skill" pill in the top-right of the
page that links to the companion SKILL.md (computed as`<page-permalink>/SKILL.md`). The badge mirrors the styling of the
header pill used by the static tools at `tools.secutils.dev/*.html`, so the affordance is recognisable across both surfaces.

The implementation is a Docusaurus swizzle:

- `src/components/SkillBadge.tsx` / `.scss` - the standalone link/pill component.
- `src/theme/DocItem/Content/index.tsx` - wraps the default `@theme-original/DocItem/Content` component, reads the 
  current doc's `frontMatter` via `useDoc()`, and prepends the badge when `frontMatter.skill === true`.

When adding a new SKILL.md, remember to add `skill: true` to the sibling `.mdx` so the badge appears.
