import { EuiBadge, EuiButtonIcon, EuiNotificationBadge, EuiPopover, EuiSelectable, useEuiTheme } from '@elastic/eui';
import type { EuiSelectableOption } from '@elastic/eui';
import { css } from '@emotion/react';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { useUserTags } from '../../../hooks';

export interface TagScopeSelectorProps {
  selectedTagIds: string[];
  onChange: (tagIds: string[]) => void;
}

export function TagScopeSelector({ selectedTagIds, onChange }: TagScopeSelectorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const { allTags, refreshTags } = useUserTags();
  const euiTheme = useEuiTheme();

  useEffect(() => {
    if (isOpen) {
      refreshTags();
    }
  }, [isOpen, refreshTags]);

  const options: EuiSelectableOption[] = useMemo(
    () =>
      allTags.map((tag) => ({
        label: tag.name,
        key: tag.id,
        checked: selectedTagIds.includes(tag.id) ? 'on' : undefined,
        prepend: <EuiBadge color={tag.color}>{'\u00A0'}</EuiBadge>,
      })),
    [allTags, selectedTagIds],
  );

  const handleChange = useCallback(
    (newOptions: EuiSelectableOption[]) => {
      onChange(newOptions.filter((opt) => opt.checked === 'on').map((opt) => opt.key!));
    },
    [onChange],
  );

  // Count only IDs that match existing tags - stale IDs (e.g., after import
  // with remapped tags) should not inflate the badge.
  const numActive = useMemo(() => {
    const knownIds = new Set(allTags.map((tag) => tag.id));
    return selectedTagIds.filter((id) => knownIds.has(id)).length;
  }, [allTags, selectedTagIds]);

  if (allTags.length === 0) {
    return null;
  }

  return (
    <EuiPopover
      button={
        <span
          css={css`
            position: relative;
            display: inline-flex;
            margin-right: ${euiTheme.euiTheme.size.xxs};
          `}
        >
          <EuiButtonIcon
            iconType="tag"
            iconSize="m"
            size="m"
            aria-label="Filter all lists by tags"
            onClick={() => setIsOpen(!isOpen)}
          />
          {numActive > 0 && (
            <EuiNotificationBadge
              css={css`
                position: absolute;
                top: 2px;
                right: 0;
                pointer-events: none;
              `}
            >
              {numActive}
            </EuiNotificationBadge>
          )}
        </span>
      }
      isOpen={isOpen}
      closePopover={() => setIsOpen(false)}
      panelPaddingSize="none"
      anchorPosition="downRight"
    >
      <EuiSelectable
        searchable
        searchProps={{ placeholder: 'Filter tags…', compressed: true }}
        options={options}
        onChange={handleChange}
      >
        {(list, search) => (
          <div style={{ width: 240 }}>
            {search}
            {list}
          </div>
        )}
      </EuiSelectable>
    </EuiPopover>
  );
}
