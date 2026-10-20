import { EuiLink, EuiText, EuiToolTip, useEuiFontSize, useEuiTheme } from '@elastic/eui';
import { css } from '@emotion/react';
import type { ReactNode } from 'react';
import { useLayoutEffect, useRef, useState } from 'react';

import { TagDot } from './tag_dot';
import type { EntityTag } from '../../../model';

interface EntityNameProps {
  name: string;
  href?: string;
  disabled?: boolean;
  icons?: ReactNode[];
  tags?: EntityTag[];
}

export function EntityName({ name, href, disabled, icons, tags }: EntityNameProps) {
  const { euiTheme } = useEuiTheme();
  const disabledColor = euiTheme.colors.textDisabled;

  return (
    // EUI cell content is a flex row: grow so the tags line gets the whole cell, and keep a minimum width so short
    // names stay on one line when long URLs compete for the table width.
    <div style={{ flexGrow: 1, minWidth: 'min(200px, 15vw)', color: disabled ? disabledColor : undefined }}>
      {href ? (
        <EuiLink href={href} style={{ color: disabled ? disabledColor : euiTheme.colors.text }}>
          {name}
        </EuiLink>
      ) : (
        <EuiText size="s">
          <strong>{name}</strong>
        </EuiText>
      )}
      {icons && icons.length > 0 ? (
        <span
          style={{
            display: 'inline-flex',
            gap: 4,
            marginLeft: 4,
            verticalAlign: 'middle',
            whiteSpace: 'nowrap',
            opacity: disabled ? 0.5 : undefined,
          }}
        >
          {icons}
        </span>
      ) : null}
      {tags && tags.length > 0 ? <EntityTags tags={tags} disabled={disabled} /> : null}
    </div>
  );
}

const TAG_GAP = 10;

/**
 * Returns how many tags fit into `available` pixels, reserving room for the "+N" chip whenever some tags are
 * left out. At least one tag is always shown, truncated if necessary.
 */
export function fitTagCount(widths: number[], available: number, gap: number, overflowWidth: number) {
  let used = 0;
  for (let i = 0; i < widths.length; i++) {
    const next = used + (i > 0 ? gap : 0) + widths[i];
    const reserve = i < widths.length - 1 ? gap + overflowWidth : 0;
    if (next + reserve > available) {
      return Math.max(i, 1);
    }
    used = next;
  }
  return widths.length;
}

function TagLabel({ tag }: { tag: EntityTag }) {
  return (
    <span
      style={{
        display: 'inline-block',
        minWidth: 0,
        maxWidth: 160,
        overflow: 'hidden',
        textOverflow: 'ellipsis',
        whiteSpace: 'nowrap',
      }}
    >
      <TagDot color={tag.color} />
      <span style={{ marginInlineStart: 5 }}>{tag.name}</span>
    </span>
  );
}

export function EntityTags({ tags, disabled }: { tags: EntityTag[]; disabled?: boolean }) {
  const { euiTheme } = useEuiTheme();
  const fontSize = useEuiFontSize('xs');
  const containerRef = useRef<HTMLDivElement>(null);
  const measureRef = useRef<HTMLDivElement>(null);
  const [visibleCount, setVisibleCount] = useState(tags.length);

  useLayoutEffect(() => {
    const container = containerRef.current;
    const measure = measureRef.current;
    if (!container || !measure) {
      return;
    }

    const update = () => {
      const [overflow, ...items] = Array.from(measure.children).map((child) => child.getBoundingClientRect().width);
      setVisibleCount(fitTagCount(items, container.getBoundingClientRect().width, TAG_GAP, overflow));
    };
    update();

    // The measure layer resizes when web fonts load, the container when the column resizes.
    const observer = new ResizeObserver(update);
    observer.observe(container);
    observer.observe(measure);
    return () => observer.disconnect();
  }, [tags]);

  const hiddenTags = tags.slice(visibleCount);
  const overflowChipStyle = {
    flexShrink: 0,
    whiteSpace: 'nowrap' as const,
    padding: '0 6px',
    borderRadius: euiTheme.border.radius.small,
    backgroundColor: euiTheme.colors.backgroundBaseSubdued,
    color: euiTheme.colors.text,
  };

  return (
    <div
      ref={containerRef}
      style={{
        position: 'relative',
        // Zero intrinsic width so tags never widen the column, then fill whatever width the name leaves.
        width: 0,
        minWidth: '100%',
        display: 'flex',
        alignItems: 'center',
        gap: TAG_GAP,
        overflow: 'hidden',
        marginTop: euiTheme.size.xs,
        ...fontSize,
        color: euiTheme.colors.textSubdued,
        opacity: disabled ? 0.6 : undefined,
      }}
    >
      {tags.slice(0, visibleCount).map((tag) => (
        <EuiToolTip key={tag.id} content={tag.name} anchorProps={{ style: { minWidth: 0, display: 'flex' } }}>
          <TagLabel tag={tag} />
        </EuiToolTip>
      ))}
      {hiddenTags.length > 0 ? (
        <EuiToolTip
          content={
            <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
              {hiddenTags.map((tag) => (
                <TagLabel key={tag.id} tag={tag} />
              ))}
            </div>
          }
          anchorProps={{ style: { flexShrink: 0 } }}
        >
          <span
            tabIndex={0}
            aria-label={`${hiddenTags.length} more tags: ${hiddenTags.map((tag) => tag.name).join(', ')}`}
            style={overflowChipStyle}
          >
            +{hiddenTags.length}
          </span>
        </EuiToolTip>
      ) : null}
      <div
        ref={measureRef}
        aria-hidden={true}
        css={css`
          position: absolute;
          inset-block-start: 0;
          inset-inline-start: 0;
          display: flex;
          gap: ${TAG_GAP}px;
          width: max-content;
          visibility: hidden;
          pointer-events: none;
          & > * {
            flex-shrink: 0;
          }
        `}
      >
        <span style={overflowChipStyle}>+{tags.length}</span>
        {tags.map((tag) => (
          <TagLabel key={tag.id} tag={tag} />
        ))}
      </div>
    </div>
  );
}
