/**
 * Wraps the default `DocItem/Content` theme component to inject a small "AI agent skill" badge above the doc title on
 * every page whose frontmatter declares `skill: true`. The badge links to the companion `SKILL.md` (Cloudflare Agent
 * Skills Discovery RFC v0.2.0) that we ship in `static/guides/<area>/<name>/SKILL.md`, which the docs nginx serves as
 * `text/markdown`. The static-tools site (`tools.secutils.dev/*.html`) ships an identically-styled link in its own page
 * headers, keeping the affordance consistent across both surfaces means an agent that has scraped one knows where to
 * look on the other.
 *
 * This is a swizzle "wrap" (https://docusaurus.io/docs/swizzling#wrapping): we delegate to `@theme-original/DocItem/Content`
 * for everything else, so theme upgrades only break here if the original component's prop contract changes.
 */
import { useDoc } from '@docusaurus/plugin-content-docs/client';
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore - virtual swizzle alias resolved by Docusaurus at build time.
import Content from '@theme-original/DocItem/Content';
import React from 'react';

import SkillBadge from '@site/src/components/SkillBadge';

interface ContentProps {
  children: React.ReactNode;
}

/**
 * Strict subset of the `frontMatter` shape we care about. Docusaurus types
 * `frontMatter` as `{ [key: string]: unknown }` so we cast a narrow view.
 */
interface SkillFrontMatter {
  skill?: boolean;
}

export default function DocItemContentWrapper(props: ContentProps): React.ReactElement {
  const { frontMatter, metadata } = useDoc();
  const fm = frontMatter as SkillFrontMatter;
  // The doc's `permalink` already includes the site's baseUrl (e.g. `/docs/guides/webhooks`), so appending `/SKILL.md`
  // lands on the exact static asset the docs build copies verbatim from `static/guides/<area>/<name>/SKILL.md`.
  const skillHref = fm.skill ? `${metadata.permalink.replace(/\/$/, '')}/SKILL.md` : null;
  return (
    <>
      {skillHref && <SkillBadge href={skillHref} />}
      <Content {...props} />
    </>
  );
}
