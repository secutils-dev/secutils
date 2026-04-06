import { EuiBadge, EuiLink, EuiText, EuiToolTip, useEuiTheme } from '@elastic/eui';
import type { ReactNode } from 'react';

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
    <div
      style={{
        display: 'flex',
        flexWrap: 'wrap',
        alignItems: 'center',
        gap: 4,
        color: disabled ? disabledColor : undefined,
      }}
    >
      <span>
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
      </span>
      {tags?.map((tag) => (
        <EuiToolTip key={tag.id} content={tag.name}>
          <EuiBadge
            color={tag.color}
            isDisabled={disabled}
            style={{
              maxWidth: 120,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
            }}
          >
            {tag.name}
          </EuiBadge>
        </EuiToolTip>
      ))}
    </div>
  );
}
