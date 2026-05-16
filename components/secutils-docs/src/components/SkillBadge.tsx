import React from 'react';

import './SkillBadge.scss';

interface SkillBadgeProps {
  href: string;
}

/**
 * Small floating "AI agent skill" link rendered in the top-right of doc pages that ship a companion `SKILL.md` as
 * defined in the Cloudflare Agent Skills Discovery RFC v0.2.0. Mirrors the header pill used by the static tools at
 * `tools.secutils.dev` so agents can grab the same artefact from either surface.
 */
export default function SkillBadge({ href }: SkillBadgeProps): React.ReactElement {
  return (
    <a
      className="su-skill-badge"
      href={href}
      target="_blank"
      rel="noopener"
      title="View AI agent skill (SKILL.md, opens in new tab)"
      aria-label="View AI agent skill (opens in new tab)"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.582a.5.5 0 0 1 0 .962L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z" />
        <path d="M20 3v4" />
        <path d="M22 5h-4" />
        <path d="M4 17v2" />
        <path d="M5 18H3" />
      </svg>
      <span>Skill</span>
    </a>
  );
}
